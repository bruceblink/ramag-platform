//! Schema Registry HTTP 读取适配器；只读取 Subject 名称并限制响应大小。

use std::{
    io::Read,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use async_trait::async_trait;
use ramag_domain::{
    entities::{KafkaClusterConfig, KafkaSchemaRegistrySubject},
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
            return Err(cancelled_error());
        }
        let Some(endpoint) = config.schema_registry.endpoint.as_deref() else {
            return Ok(Vec::new());
        };
        let url = subjects_url(endpoint)?;
        let mut request = self.client.get(url);
        if let (Some(username), Some(password)) = (
            config.schema_registry.username.as_deref(),
            config.schema_registry.password.as_deref(),
        ) {
            request = request.basic_auth(username, Some(password));
        }
        let response = request.send().map_err(request_error)?;
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(cancelled_error());
        }
        if !response.status().is_success() {
            let status = response.status();
            let category = if status == reqwest::StatusCode::UNAUTHORIZED
                || status == reqwest::StatusCode::FORBIDDEN
            {
                KafkaErrorCategory::PermissionDenied
            } else if status == reqwest::StatusCode::NOT_FOUND {
                KafkaErrorCategory::NotFound
            } else {
                KafkaErrorCategory::Protocol
            };
            return Err(DomainError::Kafka(
                KafkaError::new(
                    category,
                    "读取 Schema Registry Subject",
                    format!("Schema Registry 返回 HTTP {}", status.as_u16()),
                )
                .retryable(matches!(category, KafkaErrorCategory::Protocol)),
            ));
        }
        let body = read_response_body(response)?;
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
}

fn subjects_url(endpoint: &str) -> Result<Url> {
    let mut url = Url::parse(endpoint)
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
    let path = url.path().trim_end_matches('/');
    let path = if path.is_empty() {
        "/subjects".into()
    } else {
        format!("{path}/subjects")
    };
    url.set_path(&path);
    Ok(url)
}

fn read_response_body(mut response: Response) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(DomainError::Kafka(KafkaError::new(
            KafkaErrorCategory::Protocol,
            "读取 Schema Registry Subject",
            "Schema Registry 响应超过 4 MiB 限制",
        )));
    }
    let mut body = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = response.read(&mut buffer).map_err(|_| {
            DomainError::Kafka(KafkaError::new(
                KafkaErrorCategory::Network,
                "读取 Schema Registry Subject",
                "读取 Schema Registry 响应失败",
            ))
        })?;
        if read == 0 {
            break;
        }
        if body.len().saturating_add(read) > MAX_RESPONSE_BYTES {
            return Err(DomainError::Kafka(KafkaError::new(
                KafkaErrorCategory::Protocol,
                "读取 Schema Registry Subject",
                "Schema Registry 响应超过 4 MiB 限制",
            )));
        }
        body.extend_from_slice(&buffer[..read]);
    }
    Ok(body)
}

fn request_error(error: reqwest::Error) -> DomainError {
    let (category, message) = if error.is_timeout() {
        (KafkaErrorCategory::Timeout, "Schema Registry 请求超时")
    } else if error.is_connect() {
        (KafkaErrorCategory::Network, "无法连接 Schema Registry")
    } else {
        (KafkaErrorCategory::Network, "Schema Registry 请求失败")
    };
    DomainError::Kafka(
        KafkaError::new(category, "读取 Schema Registry Subject", message).retryable(true),
    )
}

fn cancelled_error() -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::Cancelled,
        "读取 Schema Registry Subject",
        "Schema Registry 读取任务已取消",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::KafkaSchemaRegistryConfig;

    #[test]
    fn appends_subjects_to_a_bounded_http_endpoint() {
        let url = subjects_url("http://127.0.0.1:8081/").expect("valid endpoint");
        assert_eq!(url.as_str(), "http://127.0.0.1:8081/subjects");
        let url = subjects_url("https://registry.example/api").expect("valid endpoint");
        assert_eq!(url.as_str(), "https://registry.example/api/subjects");
    }

    #[test]
    fn rejects_endpoint_credentials_and_query_parameters() {
        assert!(subjects_url("http://user:pass@localhost:8081").is_err());
        assert!(subjects_url("http://localhost:8081?token=secret").is_err());
        assert!(subjects_url("ftp://localhost:8081").is_err());
    }

    #[test]
    fn missing_registry_endpoint_is_a_valid_empty_read() {
        let config = KafkaClusterConfig::new("local", vec!["localhost:9092".into()]);
        assert!(config.schema_registry == KafkaSchemaRegistryConfig::default());
    }
}
