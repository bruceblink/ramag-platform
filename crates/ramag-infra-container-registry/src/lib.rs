#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

//! Docker Registry HTTP v2 的有界只读查询适配器。

use std::io::Read;
use std::time::Duration;

use async_trait::async_trait;
use ramag_domain::{
    ContainerRegistryCredential, ContainerRegistryDriver, ContainerRegistryInfo,
    ContainerRegistryManifest, ContainerRegistryProfile, ContainerRegistryRepository,
    ContainerRegistryTag, MAX_CONTAINER_REGISTRY_DIGEST_BYTES, MAX_CONTAINER_REGISTRY_REPOSITORIES,
    MAX_CONTAINER_REGISTRY_REPOSITORY_BYTES, MAX_CONTAINER_REGISTRY_TAG_BYTES,
    MAX_CONTAINER_REGISTRY_TAGS,
    error::{ContainerError, ContainerErrorCategory, DomainError, Result},
};
use reqwest::{
    Method,
    blocking::{Client, Response},
};
use serde::Deserialize;
use tracing::debug;
use url::Url;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const CATALOG_PAGE_SIZE: usize = 1_000;

#[derive(Clone)]
pub struct RegistryHttpDriver {
    client: Client,
}

impl RegistryHttpDriver {
    pub fn new() -> Result<Self> {
        install_tls_crypto_provider()?;
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .user_agent(concat!("Ramag/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| invalid_config("创建 Docker Registry HTTP 客户端失败"))?;
        Ok(Self { client })
    }

    fn request(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
        operation: &'static str,
        url: Url,
    ) -> Result<Response> {
        self.request_with_method(Method::GET, profile, credential, operation, url, None)
    }

    fn request_with_method(
        &self,
        method: Method,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
        operation: &'static str,
        url: Url,
        accept: Option<&'static str>,
    ) -> Result<Response> {
        profile.validate().map_err(DomainError::InvalidConfig)?;
        if let Some(credential) = credential {
            credential.validate().map_err(DomainError::InvalidConfig)?;
        }
        let mut request = self.client.request(method, url);
        if let Some(accept) = accept {
            request = request.header(reqwest::header::ACCEPT, accept);
        }
        if let Some(credential) = credential {
            request = request.basic_auth(&credential.username, Some(&credential.password));
        }
        let response = request
            .send()
            .map_err(|error| map_request_error(operation, error))?;
        if !response.status().is_success() {
            return Err(map_status(operation, response));
        }
        Ok(response)
    }

    fn registry_url(
        profile: &ContainerRegistryProfile,
        suffix: &str,
        operation: &'static str,
    ) -> Result<Url> {
        profile.validate().map_err(DomainError::InvalidConfig)?;
        let endpoint =
            Url::parse(&profile.endpoint).map_err(|_| invalid_config("Registry 端点格式无效"))?;
        endpoint.join(suffix).map_err(|_| invalid_config(operation))
    }

    fn list_repositories_blocking(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
    ) -> Result<Vec<ContainerRegistryRepository>> {
        let mut url = Self::registry_url(profile, "v2/_catalog", "读取 Registry 仓库")?;
        url.query_pairs_mut()
            .append_pair("n", &CATALOG_PAGE_SIZE.to_string());
        let response = self.request(profile, credential, "读取 Registry 仓库", url)?;
        let payload: CatalogResponse = read_json(response, "读取 Registry 仓库")?;
        if payload.repositories.len() > MAX_CONTAINER_REGISTRY_REPOSITORIES {
            return Err(protocol_error("Registry 仓库数量超过限制"));
        }
        payload
            .repositories
            .into_iter()
            .map(|name| {
                validate_repository(&name)?;
                Ok(ContainerRegistryRepository { name })
            })
            .collect()
    }

    fn list_tags_blocking(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
        repository: &str,
    ) -> Result<Vec<ContainerRegistryTag>> {
        validate_repository(repository)?;
        let suffix = format!("v2/{repository}/tags/list");
        let url = Self::registry_url(profile, &suffix, "读取 Registry Tag")?;
        let response = self.request(profile, credential, "读取 Registry Tag", url)?;
        let payload: TagsResponse = read_json(response, "读取 Registry Tag")?;
        let tags = payload.tags.unwrap_or_default();
        if tags.len() > MAX_CONTAINER_REGISTRY_TAGS {
            return Err(protocol_error("Registry Tag 数量超过限制"));
        }
        tags.into_iter()
            .map(|name| {
                validate_tag(&name)?;
                Ok(ContainerRegistryTag {
                    repository: payload
                        .name
                        .clone()
                        .unwrap_or_else(|| repository.to_owned()),
                    name,
                })
            })
            .collect()
    }

    fn test_connection_blocking(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
    ) -> Result<ContainerRegistryInfo> {
        let url = Self::registry_url(profile, "v2/", "测试 Registry 连接")?;
        let response = self.request(profile, credential, "测试 Registry 连接", url)?;
        let server_version = response
            .headers()
            .get("docker-distribution-api-version")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        Ok(ContainerRegistryInfo {
            registry_id: profile.id.clone(),
            api_version: Some("v2".into()),
            server_version,
            authenticated: credential.is_some(),
        })
    }

    fn get_manifest_blocking(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
        repository: &str,
        reference: &str,
    ) -> Result<ContainerRegistryManifest> {
        validate_repository(repository)?;
        validate_reference(reference)?;
        let suffix = format!("v2/{repository}/manifests/{reference}");
        let url = Self::registry_url(profile, &suffix, "读取 Registry 镜像清单")?;
        let response = self.request_with_method(
            Method::HEAD,
            profile,
            credential,
            "读取 Registry 镜像清单",
            url,
            Some(concat!(
                "application/vnd.oci.image.manifest.v1+json, ",
                "application/vnd.docker.distribution.manifest.v2+json"
            )),
        )?;
        let digest = response
            .headers()
            .get("docker-content-digest")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| protocol_error("Registry 镜像清单缺少 digest"))?;
        validate_digest(digest)?;
        let media_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        Ok(ContainerRegistryManifest {
            repository: repository.to_owned(),
            reference: reference.to_owned(),
            digest: digest.to_owned(),
            media_type,
            size_bytes: response.content_length(),
        })
    }
}

fn install_tls_crypto_provider() -> Result<()> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        return Err(invalid_config("无法安装 TLS 加密 Provider"));
    }
    Ok(())
}

#[async_trait]
impl ContainerRegistryDriver for RegistryHttpDriver {
    async fn test_connection(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
    ) -> Result<ContainerRegistryInfo> {
        let driver = self.clone();
        let profile = profile.clone();
        let credential = credential.cloned();
        smol::unblock(move || driver.test_connection_blocking(&profile, credential.as_ref())).await
    }

    async fn list_repositories(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
    ) -> Result<Vec<ContainerRegistryRepository>> {
        let driver = self.clone();
        let profile = profile.clone();
        let credential = credential.cloned();
        smol::unblock(move || driver.list_repositories_blocking(&profile, credential.as_ref()))
            .await
    }

    async fn list_tags(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
        repository: &str,
    ) -> Result<Vec<ContainerRegistryTag>> {
        let driver = self.clone();
        let profile = profile.clone();
        let credential = credential.cloned();
        let repository = repository.to_owned();
        smol::unblock(move || driver.list_tags_blocking(&profile, credential.as_ref(), &repository))
            .await
    }

    async fn get_manifest(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
        repository: &str,
        reference: &str,
    ) -> Result<ContainerRegistryManifest> {
        let driver = self.clone();
        let profile = profile.clone();
        let credential = credential.cloned();
        let repository = repository.to_owned();
        let reference = reference.to_owned();
        smol::unblock(move || {
            driver.get_manifest_blocking(&profile, credential.as_ref(), &repository, &reference)
        })
        .await
    }
}

#[derive(Debug, Deserialize)]
struct CatalogResponse {
    #[serde(default)]
    repositories: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    name: Option<String>,
    tags: Option<Vec<String>>,
}

fn read_json<T: for<'de> Deserialize<'de>>(
    mut response: Response,
    operation: &'static str,
) -> Result<T> {
    let Some(length) = response.content_length() else {
        return parse_json_response(&mut response, operation);
    };
    if length as usize > MAX_RESPONSE_BYTES {
        return Err(protocol_error("Registry 响应超过大小限制"));
    }
    parse_json_response(&mut response, operation)
}

fn parse_json_response<T: for<'de> Deserialize<'de>>(
    response: &mut Response,
    operation: &'static str,
) -> Result<T> {
    let mut body = Vec::new();
    response
        .take((MAX_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|_| protocol_error("读取 Registry 响应失败"))?;
    if body.len() > MAX_RESPONSE_BYTES {
        return Err(protocol_error("Registry 响应超过大小限制"));
    }
    serde_json::from_slice(&body).map_err(|_| protocol_error(operation))
}

fn validate_repository(repository: &str) -> Result<()> {
    if repository.is_empty() || repository.len() > MAX_CONTAINER_REGISTRY_REPOSITORY_BYTES {
        return Err(DomainError::InvalidConfig(
            "Registry 仓库名称无效或超过长度限制".into(),
        ));
    }
    if repository.split('/').any(|part| {
        part.is_empty()
            || part == "."
            || part == ".."
            || !part.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
            })
    }) {
        return Err(DomainError::InvalidConfig(
            "Registry 仓库名称包含不支持的字符".into(),
        ));
    }
    Ok(())
}

fn validate_tag(tag: &str) -> Result<()> {
    if tag.is_empty()
        || tag.len() > MAX_CONTAINER_REGISTRY_TAG_BYTES
        || tag.chars().any(char::is_control)
        || !tag
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
    {
        return Err(protocol_error("Registry 返回的 Tag 无效"));
    }
    Ok(())
}

fn validate_reference(reference: &str) -> Result<()> {
    if reference.starts_with("sha256:") {
        validate_digest(reference)
    } else {
        validate_tag(reference)
    }
}

fn validate_digest(digest: &str) -> Result<()> {
    if digest.is_empty() || digest.len() > MAX_CONTAINER_REGISTRY_DIGEST_BYTES {
        return Err(protocol_error("Registry digest 无效或超过长度限制"));
    }
    let Some((algorithm, encoded)) = digest.split_once(':') else {
        return Err(protocol_error("Registry digest 格式无效"));
    };
    if algorithm.is_empty()
        || !algorithm.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"+._-".contains(&byte)
        })
        || encoded.len() < 32
        || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(protocol_error("Registry digest 格式无效"));
    }
    Ok(())
}

fn map_request_error(operation: &'static str, error: reqwest::Error) -> DomainError {
    let (category, message, retryable) = if error.is_timeout() {
        (ContainerErrorCategory::Timeout, "Registry 请求超时", true)
    } else if error.is_builder() {
        (
            ContainerErrorCategory::InvalidConfig,
            "Registry 请求配置无效",
            false,
        )
    } else {
        (
            ContainerErrorCategory::Network,
            "Registry 网络请求失败",
            true,
        )
    };
    debug!(operation, category = ?category, retryable, "registry request failed");
    DomainError::Container(ContainerError::new(category, operation, message).retryable(retryable))
}

fn map_status(operation: &'static str, response: Response) -> DomainError {
    let status = response.status().as_u16();
    let (category, message, retryable) = match status {
        401 => (
            ContainerErrorCategory::Authentication,
            "Registry 认证失败",
            false,
        ),
        403 => (
            ContainerErrorCategory::PermissionDenied,
            "Registry 权限不足",
            false,
        ),
        404 => (
            ContainerErrorCategory::NotFound,
            "Registry 资源不存在",
            false,
        ),
        408 | 504 => (ContainerErrorCategory::Timeout, "Registry 请求超时", true),
        429 | 500..=599 => (
            ContainerErrorCategory::Network,
            "Registry 服务暂时不可用",
            true,
        ),
        _ => (
            ContainerErrorCategory::Protocol,
            "Registry 返回错误响应",
            false,
        ),
    };
    debug!(operation, status, category = ?category, retryable, "registry server rejected request");
    DomainError::Container(ContainerError::new(category, operation, message).retryable(retryable))
}

fn invalid_config(message: &str) -> DomainError {
    DomainError::Container(ContainerError::new(
        ContainerErrorCategory::InvalidConfig,
        "创建 Registry 客户端",
        message,
    ))
}

fn protocol_error(message: &str) -> DomainError {
    DomainError::Container(ContainerError::new(
        ContainerErrorCategory::Protocol,
        "解析 Registry 响应",
        message,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_tls_provider_before_building_client() {
        assert!(install_tls_crypto_provider().is_ok());
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
        assert!(RegistryHttpDriver::new().is_ok());
    }

    #[test]
    fn rejects_unsafe_repository_path_before_http_request() {
        assert!(validate_repository("library/nginx").is_ok());
        assert!(validate_repository("library/../secret").is_err());
        assert!(validate_repository("Library/nginx").is_err());
    }

    #[test]
    fn accepts_manifest_tags_and_digests_but_not_paths() {
        assert!(validate_reference("stable-2026.09").is_ok());
        assert!(validate_reference("sha256:0123456789abcdef0123456789abcdef").is_ok());
        assert!(validate_reference("stable/latest").is_err());
        assert!(validate_reference("sha256:not-a-digest").is_err());
    }

    #[test]
    #[ignore = "需要本机 Docker Registry v2 中的临时清单，使用环境变量和 cargo test -- --ignored 执行"]
    fn reads_manifest_digest_from_local_registry() {
        let endpoint = std::env::var("RAMAG_TEST_REGISTRY_ENDPOINT")
            .unwrap_or_else(|_| "http://127.0.0.1:15000".into());
        let repository = std::env::var("RAMAG_TEST_REGISTRY_REPOSITORY")
            .expect("必须设置 RAMAG_TEST_REGISTRY_REPOSITORY");
        let reference =
            std::env::var("RAMAG_TEST_REGISTRY_REFERENCE").unwrap_or_else(|_| "stable".into());
        let expected_digest = std::env::var("RAMAG_TEST_REGISTRY_DIGEST")
            .expect("必须设置 RAMAG_TEST_REGISTRY_DIGEST");
        let profile =
            ContainerRegistryProfile::new("本机 Registry", endpoint).with_insecure_http(true);
        let driver = RegistryHttpDriver::new().expect("Registry HTTP 客户端应创建成功");
        let manifest = smol::block_on(driver.get_manifest(&profile, None, &repository, &reference))
            .expect("Registry 镜像清单应成功读取");
        assert_eq!(manifest.repository, repository);
        assert_eq!(manifest.reference, reference);
        assert_eq!(manifest.digest, expected_digest);
        assert!(manifest.size_bytes.is_some());
    }

    #[test]
    #[ignore = "需要本机 Docker Registry v2，使用 cargo test -- --ignored 执行"]
    fn reads_local_registry_v2_without_writes() {
        let endpoint = std::env::var("RAMAG_TEST_REGISTRY_ENDPOINT")
            .unwrap_or_else(|_| "http://127.0.0.1:15000".into());
        let profile =
            ContainerRegistryProfile::new("本机 Registry", endpoint).with_insecure_http(true);
        let driver = RegistryHttpDriver::new().expect("Registry HTTP 客户端应创建成功");
        let info = smol::block_on(driver.test_connection(&profile, None))
            .expect("Registry v2 连接测试应成功");
        assert_eq!(info.registry_id, profile.id);
        assert_eq!(info.api_version.as_deref(), Some("v2"));

        let repositories =
            smol::block_on(driver.list_repositories(&profile, None)).expect("仓库列表应成功");
        assert!(repositories.len() <= MAX_CONTAINER_REGISTRY_REPOSITORIES);

        if let Some(repository) = repositories.first() {
            let tags = smol::block_on(driver.list_tags(&profile, None, &repository.name))
                .expect("已有仓库的 Tag 列表应成功");
            assert!(tags.len() <= MAX_CONTAINER_REGISTRY_TAGS);
        }

        let missing = smol::block_on(driver.list_tags(&profile, None, "ramag-missing/repository"));
        assert!(matches!(
            missing,
            Err(DomainError::Container(error))
                if error.category == ContainerErrorCategory::NotFound
        ));
    }
}
