//! Schema Registry HTTP 读取适配器；只读取 Subject、Version 和 Schema 详情并限制响应大小。

use std::{
    io::Read,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use async_trait::async_trait;
use ramag_domain::{
    entities::{
        KafkaClusterConfig, KafkaSchemaRegistrySubject, KafkaSchemaRegistryVersion,
        MAX_KAFKA_SCHEMA_VERSIONS,
    },
    error::{DomainError, KafkaError, KafkaErrorCategory, Result},
    traits::KafkaSchemaRegistryDriver,
};
use reqwest::{
    Url,
    blocking::{Client, Response},
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone)]
pub struct SchemaRegistryHttpDriver {
    client: Client,
}

impl SchemaRegistryHttpDriver {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .user_agent(concat!("Ramag/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| DomainError::Other("创建 Schema Registry HTTP 客户端失败".into()))?;
        Ok(Self { client })
    }

    fn list_subjects_blocking(
        &self,
        config: &KafkaClusterConfig,
        cancelled: &AtomicBool,
    ) -> Result<Vec<KafkaSchemaRegistrySubject>> {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(cancelled_error("读取 Schema Registry Subject"));
        }
        let Some(endpoint) = config.schema_registry.endpoint.as_deref() else {
            return Ok(Vec::new());
        };
        let url = subjects_url(endpoint)?;
        let response = self.get(config, url, "读取 Schema Registry Subject", cancelled)?;
        let body = read_response_body(response, "读取 Schema Registry Subject", cancelled)?;
        serde_json::from_slice::<Vec<String>>(&body)
            .map(|names| {
                names
                    .into_iter()
                    .map(|name| KafkaSchemaRegistrySubject { name })
                    .collect()
            })
            .map_err(|_| {
                DomainError::Kafka(KafkaError::new(
                    KafkaErrorCategory::Protocol,
                    "读取 Schema Registry Subject",
                    "Schema Registry 返回的 Subject 列表格式无效",
                ))
            })
    }

    fn list_versions_blocking(
        &self,
        config: &KafkaClusterConfig,
        subject: &str,
        cancelled: &AtomicBool,
    ) -> Result<Vec<i32>> {
        let endpoint = config
            .schema_registry
            .endpoint
            .as_deref()
            .ok_or_else(|| invalid_config_error("未配置 Schema Registry 端点"))?;
        let response = self.get(
            config,
            subject_versions_url(endpoint, subject)?,
            "读取 Schema Registry Version",
            cancelled,
        )?;
        let body = read_response_body(response, "读取 Schema Registry Version", cancelled)?;
        let versions = serde_json::from_slice::<Vec<i32>>(&body).map_err(|_| {
            protocol_error(
                "读取 Schema Registry Version",
                "Schema Registry 返回的 Version 列表格式无效",
            )
        })?;
        if versions.len() > MAX_KAFKA_SCHEMA_VERSIONS {
            return Err(protocol_error(
                "读取 Schema Registry Version",
                "Schema Registry 返回的 Version 数量超过限制",
            ));
        }
        Ok(versions)
    }

    fn get_version_blocking(
        &self,
        config: &KafkaClusterConfig,
        subject: &str,
        version: i32,
        cancelled: &AtomicBool,
    ) -> Result<KafkaSchemaRegistryVersion> {
        let endpoint = config
            .schema_registry
            .endpoint
            .as_deref()
            .ok_or_else(|| invalid_config_error("未配置 Schema Registry 端点"))?;
        let response = self.get(
            config,
            schema_version_url(endpoint, subject, version)?,
            "读取 Schema Registry Schema",
            cancelled,
        )?;
        let body = read_response_body(response, "读取 Schema Registry Schema", cancelled)?;
        serde_json::from_slice::<KafkaSchemaRegistryVersion>(&body).map_err(|_| {
            protocol_error(
                "读取 Schema Registry Schema",
                "Schema Registry 返回的 Schema 版本格式无效",
            )
        })
    }

    fn get(
        &self,
        config: &KafkaClusterConfig,
        url: Url,
        operation: &'static str,
        cancelled: &AtomicBool,
    ) -> Result<Response> {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(cancelled_error(operation));
        }
        let mut request = self.client.get(url);
        if let (Some(username), Some(password)) = (
            config.schema_registry.username.as_deref(),
            config.schema_registry.password.as_deref(),
        ) {
            request = request.basic_auth(username, Some(password));
        }
        let response = request
            .send()
            .map_err(|error| request_error(error, operation))?;
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(cancelled_error(operation));
        }
        if !response.status().is_success() {
            return Err(status_error(response.status(), operation));
        }
        Ok(response)
    }
}

#[async_trait]
impl KafkaSchemaRegistryDriver for SchemaRegistryHttpDriver {
    async fn list_subjects(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<Vec<KafkaSchemaRegistrySubject>> {
        self.list_subjects_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn list_subjects_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<KafkaSchemaRegistrySubject>> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        let driver = self.clone();
        let config = config.clone();
        smol::unblock(move || driver.list_subjects_blocking(&config, &cancelled)).await
    }

    async fn list_versions(&self, config: &KafkaClusterConfig, subject: &str) -> Result<Vec<i32>> {
        self.list_versions_with_cancel(config, subject, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn list_versions_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        subject: &str,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<i32>> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        let driver = self.clone();
        let config = config.clone();
        let subject = subject.to_owned();
        smol::unblock(move || driver.list_versions_blocking(&config, &subject, &cancelled)).await
    }

    async fn get_version(
        &self,
        config: &KafkaClusterConfig,
        subject: &str,
        version: i32,
    ) -> Result<KafkaSchemaRegistryVersion> {
        self.get_version_with_cancel(config, subject, version, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn get_version_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        subject: &str,
        version: i32,
        cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaSchemaRegistryVersion> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        if version < 0 {
            return Err(invalid_config_error("Schema Version 不能为负数"));
        }
        let driver = self.clone();
        let config = config.clone();
        let subject = subject.to_owned();
        smol::unblock(move || driver.get_version_blocking(&config, &subject, version, &cancelled))
            .await
    }
}

fn subjects_url(endpoint: &str) -> Result<Url> {
    let mut url = registry_url(endpoint)?;
    append_path_segments(&mut url, &["subjects"])?;
    Ok(url)
}

fn subject_versions_url(endpoint: &str, subject: &str) -> Result<Url> {
    let mut url = registry_url(endpoint)?;
    append_path_segments(&mut url, &["subjects", subject, "versions"])?;
    Ok(url)
}

fn schema_version_url(endpoint: &str, subject: &str, version: i32) -> Result<Url> {
    let mut url = registry_url(endpoint)?;
    append_path_segments(
        &mut url,
        &["subjects", subject, "versions", &version.to_string()],
    )?;
    Ok(url)
}

fn registry_url(endpoint: &str) -> Result<Url> {
    let url = Url::parse(endpoint)
        .map_err(|_| DomainError::InvalidConfig("Schema Registry 端点地址无效".into()))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(DomainError::InvalidConfig(
            "Schema Registry 端点只支持不带认证参数的 http/https 地址".into(),
        ));
    }
    Ok(url)
}

fn append_path_segments(url: &mut Url, segments: &[&str]) -> Result<()> {
    let mut path = url
        .path_segments_mut()
        .map_err(|_| DomainError::InvalidConfig("Schema Registry 端点路径无效".into()))?;
    for segment in segments {
        path.push(segment);
    }
    Ok(())
}

fn read_response_body(
    mut response: Response,
    operation: &'static str,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(DomainError::Kafka(KafkaError::new(
            KafkaErrorCategory::Protocol,
            operation,
            "Schema Registry 响应超过 4 MiB 限制",
        )));
    }
    let mut body = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(cancelled_error(operation));
        }
        let read = response.read(&mut buffer).map_err(|_| {
            DomainError::Kafka(KafkaError::new(
                KafkaErrorCategory::Network,
                operation,
                "读取 Schema Registry 响应失败",
            ))
        })?;
        if read == 0 {
            break;
        }
        if body.len().saturating_add(read) > MAX_RESPONSE_BYTES {
            return Err(DomainError::Kafka(KafkaError::new(
                KafkaErrorCategory::Protocol,
                operation,
                "Schema Registry 响应超过 4 MiB 限制",
            )));
        }
        body.extend_from_slice(&buffer[..read]);
    }
    Ok(body)
}

fn status_error(status: reqwest::StatusCode, operation: &'static str) -> DomainError {
    let category = if matches!(
        status,
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
    ) {
        KafkaErrorCategory::PermissionDenied
    } else if status == reqwest::StatusCode::NOT_FOUND {
        KafkaErrorCategory::NotFound
    } else {
        KafkaErrorCategory::Protocol
    };
    DomainError::Kafka(
        KafkaError::new(
            category,
            operation,
            format!("Schema Registry 返回 HTTP {}", status.as_u16()),
        )
        .retryable(matches!(category, KafkaErrorCategory::Protocol)),
    )
}

fn request_error(error: reqwest::Error, operation: &'static str) -> DomainError {
    let (category, message) = if error.is_timeout() {
        (KafkaErrorCategory::Timeout, "Schema Registry 请求超时")
    } else if error.is_connect() {
        (KafkaErrorCategory::Network, "无法连接 Schema Registry")
    } else {
        (KafkaErrorCategory::Network, "Schema Registry 请求失败")
    };
    DomainError::Kafka(KafkaError::new(category, operation, message).retryable(true))
}

fn invalid_config_error(message: &str) -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::InvalidConfig,
        "读取 Schema Registry",
        message,
    ))
}

fn protocol_error(operation: &'static str, message: &str) -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::Protocol,
        operation,
        message,
    ))
}

fn cancelled_error(operation: &'static str) -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::Cancelled,
        operation,
        "Schema Registry 读取任务已取消",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::KafkaSchemaRegistryConfig;

    #[test]
    fn appends_subjects_to_a_bounded_http_endpoint() -> Result<()> {
        let url = subjects_url("http://127.0.0.1:8081/")?;
        assert_eq!(url.as_str(), "http://127.0.0.1:8081/subjects");
        let url = subjects_url("https://registry.example/api")?;
        assert_eq!(url.as_str(), "https://registry.example/api/subjects");
        Ok(())
    }

    #[test]
    fn rejects_endpoint_credentials_and_query_parameters() {
        assert!(subjects_url("http://user:pass@localhost:8081").is_err());
        assert!(subjects_url("http://localhost:8081?token=secret").is_err());
        assert!(subjects_url("ftp://localhost:8081").is_err());
    }

    #[test]
    fn appends_encoded_subject_version_paths() -> Result<()> {
        let versions = subject_versions_url("https://registry.example/api", "orders/value")?;
        assert_eq!(versions.path(), "/api/subjects/orders%2Fvalue/versions");
        let detail = schema_version_url("https://registry.example/api", "orders-value", 3)?;
        assert_eq!(
            detail.as_str(),
            "https://registry.example/api/subjects/orders-value/versions/3"
        );
        Ok(())
    }

    #[test]
    fn parses_a_bounded_schema_version_response() -> Result<()> {
        let detail: KafkaSchemaRegistryVersion = serde_json::from_str(
            r#"{"subject":"orders-value","version":3,"id":42,"schemaType":"AVRO","schema":"{}"}"#,
        )
        .map_err(|error| {
            DomainError::InvalidConfig(format!("Schema version JSON should parse: {error}"))
        })?;
        assert!(detail.validate().is_ok());
        assert_eq!(detail.schema_type.as_deref(), Some("AVRO"));
        Ok(())
    }

    #[test]
    fn missing_registry_endpoint_is_a_valid_empty_read() {
        let config = KafkaClusterConfig::new("local", vec!["localhost:9092".into()]);
        assert!(config.schema_registry == KafkaSchemaRegistryConfig::default());
    }
}
