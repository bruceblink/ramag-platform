//! Kafka Connect REST 只读适配器；读取连接器和 Task 状态，不执行外部写操作。

use std::{
    io::Read,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use async_trait::async_trait;
use ramag_domain::{
    entities::{KafkaClusterConfig, KafkaConnectConnector, KafkaConnectTask},
    error::{DomainError, KafkaError, KafkaErrorCategory, Result},
    traits::KafkaConnectDriver,
};
use reqwest::{
    Url,
    blocking::{Client, Response},
};
use serde_json::Value;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone)]
pub struct KafkaConnectHttpDriver {
    client: Client,
}

impl KafkaConnectHttpDriver {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .user_agent(concat!("Ramag/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| DomainError::Other("创建 Kafka Connect HTTP 客户端失败".into()))?;
        Ok(Self { client })
    }

    fn list_connectors_blocking(
        &self,
        config: &KafkaClusterConfig,
        cancelled: &AtomicBool,
    ) -> Result<Vec<KafkaConnectConnector>> {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(cancelled_error());
        }
        let Some(endpoint) = config.connect.endpoint.as_deref() else {
            return Ok(Vec::new());
        };
        let url = connectors_url(endpoint)?;
        let mut request = self.client.get(url);
        if let (Some(username), Some(password)) = (
            config.connect.username.as_deref(),
            config.connect.password.as_deref(),
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
                    "读取 Kafka Connect 连接器",
                    format!("Kafka Connect 返回 HTTP {}", status.as_u16()),
                )
                .retryable(matches!(category, KafkaErrorCategory::Protocol)),
            ));
        }
        parse_connectors(&read_response_body(response)?)
    }
}

#[async_trait]
impl KafkaConnectDriver for KafkaConnectHttpDriver {
    async fn list_connectors(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<Vec<KafkaConnectConnector>> {
        self.list_connectors_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn list_connectors_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<KafkaConnectConnector>> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        let driver = self.clone();
        let config = config.clone();
        smol::unblock(move || driver.list_connectors_blocking(&config, &cancelled)).await
    }
}

fn connectors_url(endpoint: &str) -> Result<Url> {
    let mut url = Url::parse(endpoint)
        .map_err(|_| DomainError::InvalidConfig("Kafka Connect 端点地址无效".into()))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(DomainError::InvalidConfig(
            "Kafka Connect 端点只支持不带认证参数的 http/https 地址".into(),
        ));
    }
    let path = url.path().trim_end_matches('/');
    let path = if path.is_empty() {
        "/connectors".into()
    } else {
        format!("{path}/connectors")
    };
    url.set_path(&path);
    url.set_query(Some("expand=status"));
    Ok(url)
}

fn parse_connectors(body: &[u8]) -> Result<Vec<KafkaConnectConnector>> {
    let value =
        serde_json::from_slice::<Value>(body).map_err(|_| protocol_error("JSON 格式无效"))?;
    let mut connectors = match value {
        Value::Object(entries) => entries
            .into_iter()
            .map(|(name, value)| connector_from_status(name, value))
            .collect::<Result<Vec<_>>>()?,
        Value::Array(names) => names
            .into_iter()
            .filter_map(|name| name.as_str().map(|name| empty_connector(name.to_owned())))
            .collect(),
        _ => return Err(protocol_error("连接器列表格式无效")),
    };
    connectors.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(connectors)
}

fn connector_from_status(name: String, value: Value) -> Result<KafkaConnectConnector> {
    let object = value
        .as_object()
        .ok_or_else(|| protocol_error("连接器状态格式无效"))?;
    let connector = object.get("connector").and_then(Value::as_object);
    let state = connector
        .and_then(|connector| connector.get("state"))
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN")
        .to_owned();
    let worker_id = connector
        .and_then(|connector| connector.get("worker_id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let tasks = object
        .get("tasks")
        .and_then(Value::as_array)
        .map(|tasks| tasks.iter().map(parse_task).collect::<Result<Vec<_>>>())
        .transpose()?
        .unwrap_or_default();
    let error = connector
        .and_then(|connector| connector.get("trace"))
        .and_then(Value::as_str)
        .map(compact_error);
    let connector_type = object
        .get("type")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok(KafkaConnectConnector {
        name,
        connector_type,
        state,
        worker_id,
        tasks,
        error,
    })
}

fn parse_task(value: &Value) -> Result<KafkaConnectTask> {
    let object = value
        .as_object()
        .ok_or_else(|| protocol_error("Kafka Connect Task 状态格式无效"))?;
    let id = object
        .get("id")
        .and_then(Value::as_i64)
        .and_then(|id| i32::try_from(id).ok())
        .ok_or_else(|| protocol_error("Kafka Connect Task ID 无效"))?;
    let state = object
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN")
        .to_owned();
    let worker_id = object
        .get("worker_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let trace = object
        .get("trace")
        .and_then(Value::as_str)
        .map(compact_error);
    Ok(KafkaConnectTask {
        id,
        state,
        worker_id,
        trace,
    })
}

fn empty_connector(name: String) -> KafkaConnectConnector {
    KafkaConnectConnector {
        name,
        connector_type: None,
        state: "UNKNOWN".into(),
        worker_id: None,
        tasks: Vec::new(),
        error: None,
    }
}

fn compact_error(error: &str) -> String {
    error.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn read_response_body(mut response: Response) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(protocol_error("Kafka Connect 响应超过 4 MiB 限制"));
    }
    let mut body = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|_| network_error("读取 Kafka Connect 响应失败"))?;
        if read == 0 {
            break;
        }
        if body.len().saturating_add(read) > MAX_RESPONSE_BYTES {
            return Err(protocol_error("Kafka Connect 响应超过 4 MiB 限制"));
        }
        body.extend_from_slice(&buffer[..read]);
    }
    Ok(body)
}

fn request_error(error: reqwest::Error) -> DomainError {
    if error.is_timeout() {
        return timeout_error("Kafka Connect 请求超时");
    }
    network_error("无法连接 Kafka Connect")
}

fn protocol_error(message: &str) -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::Protocol,
        "读取 Kafka Connect 连接器",
        message,
    ))
}

fn network_error(message: &str) -> DomainError {
    DomainError::Kafka(
        KafkaError::new(
            KafkaErrorCategory::Network,
            "读取 Kafka Connect 连接器",
            message,
        )
        .retryable(true),
    )
}

fn timeout_error(message: &str) -> DomainError {
    DomainError::Kafka(
        KafkaError::new(
            KafkaErrorCategory::Timeout,
            "读取 Kafka Connect 连接器",
            message,
        )
        .retryable(true),
    )
}

fn cancelled_error() -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::Cancelled,
        "读取 Kafka Connect 连接器",
        "Kafka Connect 读取任务已取消",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_connectors_and_expand_status_to_endpoint() {
        let url = connectors_url("http://127.0.0.1:8083/").expect("valid endpoint");
        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:8083/connectors?expand=status"
        );
        let url = connectors_url("https://connect.example/api").expect("valid endpoint");
        assert_eq!(
            url.as_str(),
            "https://connect.example/api/connectors?expand=status"
        );
    }

    #[test]
    fn parses_expanded_connector_status_without_leaking_trace_shape() {
        let body = br#"{
            "orders": {
                "type": "sink",
                "connector": {"state": "FAILED", "worker_id": "worker-1", "trace": "line 1\nline 2"},
                "tasks": [{"id": 0, "state": "FAILED", "worker_id": "worker-1", "trace": "task\nerror"}]
            }
        }"#;
        let connectors = parse_connectors(body).expect("valid status");
        assert_eq!(connectors[0].name, "orders");
        assert_eq!(connectors[0].state, "FAILED");
        assert_eq!(connectors[0].error.as_deref(), Some("line 1 line 2"));
        assert_eq!(connectors[0].tasks[0].trace.as_deref(), Some("task error"));
    }

    #[test]
    fn rejects_endpoint_credentials_and_query_parameters() {
        assert!(connectors_url("http://user:pass@localhost:8083").is_err());
        assert!(connectors_url("http://localhost:8083?token=secret").is_err());
        assert!(connectors_url("ftp://localhost:8083").is_err());
    }
}
