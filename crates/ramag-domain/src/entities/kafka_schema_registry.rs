//! Schema Registry 的连接配置和只读 Subject 快照。

use std::fmt;

use serde::{Deserialize, Serialize};

use super::kafka_validation::{
    validate_optional_protocol_text, validate_optional_single_line, validate_required_text,
};
use super::{
    MAX_KAFKA_SCHEMA_REGISTRY_ENDPOINT_BYTES, MAX_KAFKA_SCHEMA_REGISTRY_PASSWORD_BYTES,
    MAX_KAFKA_SCHEMA_REGISTRY_USERNAME_BYTES, MAX_KAFKA_SCHEMA_SUBJECT_BYTES,
};

/// 可选的外部 Schema Registry 连接配置；配置随 Kafka 集群配置一并加密保存。
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaSchemaRegistryConfig {
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}

impl fmt::Debug for KafkaSchemaRegistryConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KafkaSchemaRegistryConfig")
            .field("endpoint", &self.endpoint.as_ref().map(|_| "[CONFIGURED]"))
            .field("username", &self.username.as_ref().map(|_| "[REDACTED]"))
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl KafkaSchemaRegistryConfig {
    pub fn validate(&self) -> Result<(), String> {
        validate_optional_single_line(
            "Schema Registry 端点",
            self.endpoint.as_deref(),
            MAX_KAFKA_SCHEMA_REGISTRY_ENDPOINT_BYTES,
        )?;
        if let Some(endpoint) = self.endpoint.as_deref()
            && !endpoint.starts_with("http://")
            && !endpoint.starts_with("https://")
        {
            return Err("Schema Registry 端点必须使用 http:// 或 https://".into());
        }
        validate_optional_protocol_text(
            "Schema Registry 用户名",
            self.username.as_deref(),
            MAX_KAFKA_SCHEMA_REGISTRY_USERNAME_BYTES,
        )?;
        validate_optional_protocol_text(
            "Schema Registry 密码",
            self.password.as_deref(),
            MAX_KAFKA_SCHEMA_REGISTRY_PASSWORD_BYTES,
        )?;
        if self.endpoint.is_none() && (self.username.is_some() || self.password.is_some()) {
            return Err("未配置 Schema Registry 端点时不能保存认证参数".into());
        }
        if self.username.is_some() != self.password.is_some() {
            return Err("Schema Registry Basic Auth 必须同时设置用户名和密码".into());
        }
        Ok(())
    }
}

/// Schema Registry `/subjects` 返回的一个 Subject。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaSchemaRegistrySubject {
    pub name: String,
}

impl KafkaSchemaRegistrySubject {
    pub fn validate(&self) -> Result<(), String> {
        validate_required_text("Schema Subject", &self.name, MAX_KAFKA_SCHEMA_SUBJECT_BYTES)
    }
}
