//! Kafka Connect REST 配置和只读连接器状态。

use std::fmt;

use serde::{Deserialize, Serialize};

use super::kafka_validation::{
    validate_optional_protocol_text, validate_optional_single_line, validate_required_text,
};
use super::{
    MAX_KAFKA_CONNECT_ENDPOINT_BYTES, MAX_KAFKA_CONNECT_PASSWORD_BYTES,
    MAX_KAFKA_CONNECT_USERNAME_BYTES, MAX_KAFKA_CONNECTOR_NAME_BYTES,
    MAX_KAFKA_CONNECTOR_STATE_BYTES, MAX_KAFKA_CONNECTOR_TASKS, MAX_KAFKA_CONNECTOR_TRACE_BYTES,
    MAX_KAFKA_CONNECTOR_WORKER_BYTES,
};

/// 可选的 Kafka Connect REST 地址和 Basic Auth；配置随 Kafka 集群配置一并保存。
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaConnectConfig {
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}

impl fmt::Debug for KafkaConnectConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KafkaConnectConfig")
            .field("endpoint", &self.endpoint.as_ref().map(|_| "[CONFIGURED]"))
            .field("username", &self.username.as_ref().map(|_| "[REDACTED]"))
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl KafkaConnectConfig {
    pub fn validate(&self) -> Result<(), String> {
        validate_optional_single_line(
            "Kafka Connect 端点",
            self.endpoint.as_deref(),
            MAX_KAFKA_CONNECT_ENDPOINT_BYTES,
        )?;
        if let Some(endpoint) = self.endpoint.as_deref()
            && !endpoint.starts_with("http://")
            && !endpoint.starts_with("https://")
        {
            return Err("Kafka Connect 端点必须使用 http:// 或 https://".into());
        }
        validate_optional_protocol_text(
            "Kafka Connect 用户名",
            self.username.as_deref(),
            MAX_KAFKA_CONNECT_USERNAME_BYTES,
        )?;
        validate_optional_protocol_text(
            "Kafka Connect 密码",
            self.password.as_deref(),
            MAX_KAFKA_CONNECT_PASSWORD_BYTES,
        )?;
        if self.endpoint.is_none() && (self.username.is_some() || self.password.is_some()) {
            return Err("未配置 Kafka Connect 端点时不能保存认证参数".into());
        }
        if self.username.is_some() != self.password.is_some() {
            return Err("Kafka Connect Basic Auth 必须同时设置用户名和密码".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaConnectTask {
    pub id: i32,
    pub state: String,
    #[serde(default)]
    pub worker_id: Option<String>,
    #[serde(default)]
    pub trace: Option<String>,
}

impl KafkaConnectTask {
    pub fn validate(&self) -> Result<(), String> {
        if self.id < 0 {
            return Err("Kafka Connect Task ID 不能为负数".into());
        }
        validate_required_text(
            "Kafka Connect Task 状态",
            &self.state,
            MAX_KAFKA_CONNECTOR_STATE_BYTES,
        )?;
        if let Some(worker_id) = self.worker_id.as_deref() {
            validate_required_text(
                "Kafka Connect Task Worker",
                worker_id,
                MAX_KAFKA_CONNECTOR_WORKER_BYTES,
            )?;
        }
        if let Some(trace) = self.trace.as_deref() {
            validate_optional_single_line(
                "Kafka Connect Task 错误",
                Some(trace),
                MAX_KAFKA_CONNECTOR_TRACE_BYTES,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaConnectConnector {
    pub name: String,
    #[serde(default)]
    pub connector_type: Option<String>,
    pub state: String,
    #[serde(default)]
    pub worker_id: Option<String>,
    #[serde(default)]
    pub tasks: Vec<KafkaConnectTask>,
    #[serde(default)]
    pub error: Option<String>,
}

impl KafkaConnectConnector {
    pub fn validate(&self) -> Result<(), String> {
        validate_required_text(
            "Kafka Connect 连接器名称",
            &self.name,
            MAX_KAFKA_CONNECTOR_NAME_BYTES,
        )?;
        validate_required_text(
            "Kafka Connect 连接器状态",
            &self.state,
            MAX_KAFKA_CONNECTOR_STATE_BYTES,
        )?;
        if let Some(connector_type) = self.connector_type.as_deref() {
            validate_optional_single_line(
                "Kafka Connect 连接器类型",
                Some(connector_type),
                MAX_KAFKA_CONNECTOR_NAME_BYTES,
            )?;
        }
        if let Some(worker_id) = self.worker_id.as_deref() {
            validate_required_text(
                "Kafka Connect Worker",
                worker_id,
                MAX_KAFKA_CONNECTOR_WORKER_BYTES,
            )?;
        }
        if self.tasks.len() > MAX_KAFKA_CONNECTOR_TASKS {
            return Err(format!(
                "Kafka Connect Task 数量超过 {MAX_KAFKA_CONNECTOR_TASKS} 个上限"
            ));
        }
        for task in &self.tasks {
            task.validate()?;
        }
        if let Some(error) = self.error.as_deref() {
            validate_optional_single_line(
                "Kafka Connect 连接器错误",
                Some(error),
                MAX_KAFKA_CONNECTOR_TRACE_BYTES,
            )?;
        }
        Ok(())
    }
}
