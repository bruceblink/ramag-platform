//! HTTP API 请求驱动。
//!
//! 该驱动只负责把领域请求转换为一次 HTTP 调用，并把响应转换成有界快照。
//! 它不记录请求历史，也不把凭据写入错误文本；请求变量由应用层传入并在发送前展开。

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytes::Bytes;
use http::HeaderMap;
use ramag_domain::entities::{
    ApiAuth, ApiCancellation, ApiKeyLocation, ApiMultipartValue, ApiParameter, ApiProtocol,
    ApiProxyConfig, ApiRequestSpec, ApiResponseSnapshot, ApiResponseSnapshotParts,
    ApiResponseStatus, ApiTlsConfig, MAX_API_MULTIPART_FILE_BYTES,
    MAX_API_MULTIPART_FILE_NAME_BYTES, MAX_API_MULTIPART_PATH_BYTES, MAX_API_MULTIPART_TOTAL_BYTES,
    MAX_API_PARAMETER_COUNT, MAX_API_PARAMETER_NAME_BYTES, MAX_API_PARAMETER_VALUE_BYTES,
    MAX_API_RESPONSE_BODY_BYTES,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::ApiDriver;
use reqwest::{Client, RequestBuilder, Response};
use rustls::crypto::CryptoProvider;
use tokio::io::AsyncReadExt as _;

#[path = "http_client.rs"]
mod client;
#[path = "http_proxy.rs"]
mod proxy;
#[path = "http_request.rs"]
mod request;

use crate::oauth2::{OAuth2AccessToken, OAuth2TokenProvider, resolve_oauth2_config};
pub(crate) use client::build_client;
pub(crate) use proxy::resolve_proxy_config;

pub(crate) const MAX_TLS_FILE_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const CANCELLATION_POLL: Duration = Duration::from_millis(10);

#[derive(Clone)]
pub struct HttpApiDriver {
    client: Client,
    oauth2: OAuth2TokenProvider,
}

impl HttpApiDriver {
    /// 创建不带工作区状态的 HTTP 客户端；TLS Provider 在基础设施边界确保可用。
    pub fn new() -> Result<Self> {
        ensure_tls_provider()?;
        Ok(Self {
            client: build_client(&ApiTlsConfig::default(), &ApiProxyConfig::default())?,
            oauth2: OAuth2TokenProvider::default(),
        })
    }
}

#[async_trait]
impl ApiDriver for HttpApiDriver {
    fn protocol(&self) -> ApiProtocol {
        ApiProtocol::Http
    }

    /// 校验并发送 HTTP 请求，随后以固定大小读取响应；取消标记会中断发送和响应流读取。
    async fn execute(
        &self,
        request: &ApiRequestSpec,
        variables: &BTreeMap<String, String>,
        cancelled: ApiCancellation,
    ) -> Result<ApiResponseSnapshot> {
        let spec = match request {
            ApiRequestSpec::Http(spec) => spec,
            ApiRequestSpec::Grpc(_) => {
                return Err(DomainError::InvalidConfig(
                    "HTTP 驱动不能执行 gRPC 请求".into(),
                ));
            }
        };
        spec.validate().map_err(DomainError::InvalidConfig)?;
        let (response, started) = self
            .execute_once(spec, variables, cancelled.clone(), false)
            .await?;
        if response.status().as_u16() == 401
            && let ApiAuth::OAuth2 { config } = &spec.auth
        {
            let proxy = resolve_proxy_config(&spec.proxy, variables)?;
            let config = resolve_oauth2_config(config, variables)?;
            self.oauth2.invalidate(&config, &spec.tls, &proxy).await;
            drop(response);
            let (response, started) = self
                .execute_once(spec, variables, cancelled.clone(), true)
                .await?;
            return read_response(response, started, cancelled).await;
        }
        read_response(response, started, cancelled).await
    }
}

async fn multipart_form(
    body: &ramag_domain::entities::ApiBody,
    variables: &BTreeMap<String, String>,
    cancelled: ApiCancellation,
) -> Result<reqwest::multipart::Form> {
    let mut form = reqwest::multipart::Form::new();
    let mut total_bytes = 0usize;

    for part in &body.multipart {
        ensure_not_cancelled(&cancelled)?;
        let name = expand_template(
            &part.name,
            variables,
            "Multipart 字段名称",
            MAX_API_PARAMETER_NAME_BYTES,
        )?;
        let content_type = part
            .content_type
            .as_ref()
            .map(|content_type| {
                expand_template(
                    content_type,
                    variables,
                    "Multipart Content-Type",
                    MAX_API_PARAMETER_VALUE_BYTES,
                )
            })
            .transpose()?;

        let mut request_part = match &part.value {
            ApiMultipartValue::Text { value } => {
                let value = expand_template(
                    value,
                    variables,
                    "Multipart 文本字段值",
                    MAX_API_PARAMETER_VALUE_BYTES,
                )?;
                total_bytes = add_multipart_bytes(total_bytes, value.len())?;
                reqwest::multipart::Part::text(value)
            }
            ApiMultipartValue::File { path, file_name } => {
                let path = expand_template(
                    path,
                    variables,
                    "Multipart 文件路径",
                    MAX_API_MULTIPART_PATH_BYTES,
                )?;
                let bytes = read_multipart_file(&path, cancelled.clone()).await?;
                total_bytes = add_multipart_bytes(total_bytes, bytes.len())?;
                let file_name = file_name
                    .as_ref()
                    .map(|file_name| {
                        expand_template(
                            file_name,
                            variables,
                            "Multipart 文件名",
                            MAX_API_MULTIPART_FILE_NAME_BYTES,
                        )
                    })
                    .transpose()?
                    .or_else(|| {
                        Path::new(&path)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| "file".into());
                reqwest::multipart::Part::bytes(bytes).file_name(file_name)
            }
        };
        if let Some(content_type) = content_type {
            request_part = request_part
                .mime_str(&content_type)
                .map_err(|_| DomainError::InvalidConfig("Multipart Content-Type 无效".into()))?;
        }
        form = form.part(name, request_part);
    }
    Ok(form)
}

fn add_multipart_bytes(current: usize, added: usize) -> Result<usize> {
    let total = current
        .checked_add(added)
        .ok_or_else(|| DomainError::InvalidConfig("Multipart 正文大小计算溢出".into()))?;
    if total > MAX_API_MULTIPART_TOTAL_BYTES {
        return Err(DomainError::InvalidConfig(format!(
            "Multipart 正文数据超过 {MAX_API_MULTIPART_TOTAL_BYTES} bytes 上限"
        )));
    }
    Ok(total)
}

async fn read_multipart_file(path: &str, cancelled: ApiCancellation) -> Result<Vec<u8>> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|_| DomainError::InvalidConfig("Multipart 文件无法读取".into()))?;
    let length = file
        .metadata()
        .await
        .map_err(|_| DomainError::InvalidConfig("Multipart 文件大小无法读取".into()))?
        .len();
    if length > MAX_API_MULTIPART_FILE_BYTES as u64 {
        return Err(DomainError::InvalidConfig(format!(
            "Multipart 单个文件超过 {MAX_API_MULTIPART_FILE_BYTES} bytes 上限"
        )));
    }

    let mut bytes = Vec::with_capacity(length as usize);
    let mut limited_file = file.take(MAX_API_MULTIPART_FILE_BYTES as u64 + 1);
    let mut read = Box::pin(limited_file.read_to_end(&mut bytes));
    tokio::select! {
        result = &mut read => {
            result.map_err(|_| DomainError::InvalidConfig("Multipart 文件无法读取".into()))?;
        }
        _ = wait_until_cancelled(cancelled) => {
            return Err(DomainError::Cancelled("HTTP 请求已取消".into()));
        }
    }
    if bytes.len() > MAX_API_MULTIPART_FILE_BYTES {
        return Err(DomainError::InvalidConfig(format!(
            "Multipart 单个文件超过 {MAX_API_MULTIPART_FILE_BYTES} bytes 上限"
        )));
    }
    Ok(bytes)
}

pub(crate) type ExpandedParameter = (String, String, bool);

// 把认证配置展开成请求参数；敏感标记只影响响应/日志摘要，不改变实际发送值。
fn apply_auth(
    auth: &ApiAuth,
    variables: &BTreeMap<String, String>,
    query: &mut Vec<ExpandedParameter>,
    headers: &mut Vec<ExpandedParameter>,
) -> Result<()> {
    match auth {
        ApiAuth::None => {}
        ApiAuth::Basic { username, password } => {
            let username = expand_template(
                username,
                variables,
                "Basic 用户名",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?;
            let password = expand_template(
                password,
                variables,
                "Basic 密码",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?;
            let encoded = base64_basic(&username, &password);
            headers.push(("authorization".into(), format!("Basic {encoded}"), true));
        }
        ApiAuth::Bearer { token } => {
            let token = expand_template(
                token,
                variables,
                "Bearer Token",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?;
            headers.push(("authorization".into(), format!("Bearer {token}"), true));
        }
        ApiAuth::OAuth2 { .. } => {
            return Err(DomainError::InvalidConfig(
                "OAuth2 令牌必须由驱动在发送前生成".into(),
            ));
        }
        ApiAuth::ApiKey {
            name,
            value,
            location,
        } => {
            let name = expand_template(
                name,
                variables,
                "API Key 名称",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?;
            let value = expand_template(
                value,
                variables,
                "API Key 值",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?;
            match location {
                ApiKeyLocation::Header => headers.push((name, value, true)),
                ApiKeyLocation::Query => query.push((name, value, true)),
            }
        }
    }
    Ok(())
}

pub(crate) fn expand_parameter(
    parameter: &ApiParameter,
    variables: &BTreeMap<String, String>,
    label: &str,
) -> Result<ExpandedParameter> {
    Ok((
        expand_template(
            &parameter.name,
            variables,
            label,
            MAX_API_PARAMETER_VALUE_BYTES,
        )?,
        expand_template(
            &parameter.value,
            variables,
            label,
            MAX_API_PARAMETER_VALUE_BYTES,
        )?,
        parameter.sensitive,
    ))
}

pub(crate) fn expand_template(
    template: &str,
    variables: &BTreeMap<String, String>,
    label: &str,
    max_bytes: usize,
) -> Result<String> {
    // 逐个替换 {{name}}，并在生成结果上限处停止，避免变量把请求体或 URL 放大到无界。
    let mut output = String::with_capacity(template.len());
    let mut cursor = 0usize;
    while let Some(relative_start) = template[cursor..].find("{{") {
        let start = cursor + relative_start;
        output.push_str(&template[cursor..start]);
        let value_start = start + 2;
        let relative_end = template[value_start..]
            .find("}}")
            .ok_or_else(|| DomainError::InvalidConfig(format!("{label}变量占位符未闭合")))?;
        let end = value_start + relative_end;
        let name = template[value_start..end].trim();
        if name.is_empty() {
            return Err(DomainError::InvalidConfig(format!("{label}变量名不能为空")));
        }
        let value = variables
            .get(name)
            .ok_or_else(|| DomainError::InvalidConfig(format!("缺少 API 环境变量：{name}")))?;
        output.push_str(value);
        cursor = end + 2;
    }
    output.push_str(&template[cursor..]);
    if output.len() > max_bytes {
        return Err(DomainError::InvalidConfig(format!(
            "{label}展开后超过 {max_bytes} bytes 上限"
        )));
    }
    Ok(output)
}

pub(crate) async fn send_request(
    builder: RequestBuilder,
    cancelled: ApiCancellation,
) -> Result<Response> {
    // reqwest 的发送 Future 被 pin 后与取消轮询并行；取消时丢弃 Future 会关闭底层请求。
    let mut send = Box::pin(builder.send());
    tokio::select! {
        result = &mut send => result.map_err(map_request_error),
        _ = wait_until_cancelled(cancelled) => Err(DomainError::Cancelled("HTTP 请求已取消".into())),
    }
}

async fn read_response(
    mut response: Response,
    started: Instant,
    cancelled: ApiCancellation,
) -> Result<ApiResponseSnapshot> {
    // 只保留领域层允许的正文前缀，同时用 Content-Length 或已读取字节数记录原始大小。
    let headers = response_headers(response.headers())?;
    let status = ApiResponseStatus::Http {
        code: response.status().as_u16(),
    };
    let declared_size = response.content_length();
    let mut body = Vec::new();
    let mut observed_size = 0u64;
    let mut truncated = declared_size.is_some_and(|size| size > MAX_API_RESPONSE_BODY_BYTES as u64);

    while let Some(chunk) = next_chunk(&mut response, cancelled.clone()).await? {
        observed_size = observed_size.saturating_add(chunk.len() as u64);
        if body.len() < MAX_API_RESPONSE_BODY_BYTES {
            let remaining = MAX_API_RESPONSE_BODY_BYTES - body.len();
            body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        }
        if observed_size > MAX_API_RESPONSE_BODY_BYTES as u64 {
            truncated = true;
            break;
        }
    }

    let size_bytes = declared_size.unwrap_or(observed_size);
    ApiResponseSnapshot::new(ApiResponseSnapshotParts {
        protocol: ApiProtocol::Http,
        status,
        headers,
        metadata: Vec::new(),
        body,
        elapsed_millis: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        size_bytes,
        truncated,
        error: None,
    })
    .map_err(DomainError::InvalidConfig)
}

async fn next_chunk(response: &mut Response, cancelled: ApiCancellation) -> Result<Option<Bytes>> {
    // 每个响应块都检查取消标记，避免慢响应占用请求任务直到网络自然结束。
    tokio::select! {
        result = response.chunk() => result.map_err(map_request_error),
        _ = wait_until_cancelled(cancelled) => Err(DomainError::Cancelled("HTTP 请求已取消".into())),
    }
}

pub(crate) async fn wait_until_cancelled(cancelled: ApiCancellation) {
    while !cancelled.load(Ordering::Relaxed) {
        tokio::time::sleep(CANCELLATION_POLL).await;
    }
}

fn response_headers(headers: &HeaderMap) -> Result<Vec<ApiParameter>> {
    if headers.len() > MAX_API_PARAMETER_COUNT {
        return Err(DomainError::InvalidConfig(format!(
            "HTTP 响应 Headers 数量超过 {MAX_API_PARAMETER_COUNT} 个上限"
        )));
    }
    headers
        .iter()
        .map(|(name, value)| {
            let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
            if value.len() > MAX_API_PARAMETER_VALUE_BYTES {
                return Err(DomainError::InvalidConfig(
                    "HTTP 响应 Header 值超过长度上限".into(),
                ));
            }
            Ok(ApiParameter::new(
                name.as_str(),
                value,
                is_sensitive_header(name.as_str()),
            ))
        })
        .collect()
}

fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "proxy-authorization" | "cookie" | "set-cookie" | "www-authenticate"
    )
}

pub(crate) fn read_tls_file(path: &str, label: &str) -> Result<Vec<u8>> {
    let mut file =
        File::open(path).map_err(|_| DomainError::InvalidConfig(format!("{label}文件无法读取")))?;
    let length = file
        .seek(SeekFrom::End(0))
        .map_err(|_| DomainError::InvalidConfig(format!("{label}文件大小无法读取")))?;
    if length > MAX_TLS_FILE_BYTES as u64 {
        return Err(DomainError::InvalidConfig(format!(
            "{label}文件超过 {MAX_TLS_FILE_BYTES} bytes 上限"
        )));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| DomainError::InvalidConfig(format!("{label}文件无法读取")))?;
    let mut bytes = Vec::with_capacity(length as usize);
    file.read_to_end(&mut bytes)
        .map_err(|_| DomainError::InvalidConfig(format!("{label}文件无法读取")))?;
    Ok(bytes)
}

fn ensure_tls_provider() -> Result<()> {
    if CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    if CryptoProvider::get_default().is_none() {
        return Err(DomainError::InvalidConfig(
            "无法安装 TLS 加密 Provider".into(),
        ));
    }
    Ok(())
}

pub(crate) fn ensure_not_cancelled(cancelled: &ApiCancellation) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        Err(DomainError::Cancelled("HTTP 请求已取消".into()))
    } else {
        Ok(())
    }
}

fn map_request_error(error: reqwest::Error) -> DomainError {
    if error.is_timeout() {
        DomainError::ConnectionFailed("HTTP 请求超时".into())
    } else if error.is_builder() {
        DomainError::InvalidConfig("HTTP 请求构造失败".into())
    } else {
        DomainError::ConnectionFailed("HTTP 请求失败".into())
    }
}

fn base64_basic(username: &str, password: &str) -> String {
    use base64::Engine as _;

    base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"))
}

fn oauth2_bearer_auth(token: OAuth2AccessToken) -> ApiAuth {
    ApiAuth::Bearer { token: token.value }
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
