//! MQTT 消息、订阅和 Mosquitto 管理结果的有界数据结构。

use std::fmt;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    MAX_MOSQUITTO_NAME_BYTES, MAX_MQTT_TLS_PATH_BYTES, MosquittoClient, MosquittoGroup,
    MosquittoRole, MqttProfile, validate_mqtt_topic_filter, validate_mqtt_topic_name,
};

pub const MAX_MQTT_PUBLISH_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_MQTT_SUBSCRIPTIONS: usize = 128;
pub const MAX_MQTT_USER_PROPERTIES: usize = 64;
pub const MAX_MQTT_USER_PROPERTY_BYTES: usize = 8 * 1024;
pub const MAX_MQTT_TOPIC_OBSERVATIONS: usize = 4096;
pub const MAX_MQTT_ONLINE_CLIENTS: usize = 4096;
pub const MAX_MOSQUITTO_CLIENTS: usize = 4096;
pub const MAX_MOSQUITTO_GROUPS: usize = 4096;
pub const MAX_MOSQUITTO_ROLES: usize = 4096;
pub const MAX_MOSQUITTO_STATIC_FILE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MqttQos {
    #[default]
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

impl MqttQos {
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::AtMostOnce => 0,
            Self::AtLeastOnce => 1,
            Self::ExactlyOnce => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttSubscription {
    pub filter: String,
    #[serde(default)]
    pub qos: MqttQos,
}

impl MqttSubscription {
    pub fn validate(&self) -> Result<(), String> {
        validate_mqtt_topic_filter(&self.filter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttSubscribeRequest {
    pub subscriptions: Vec<MqttSubscription>,
}

impl MqttSubscribeRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.subscriptions.is_empty() {
            return Err("MQTT 订阅至少需要一个 Topic Filter".into());
        }
        if self.subscriptions.len() > MAX_MQTT_SUBSCRIPTIONS {
            return Err(format!("MQTT 订阅数量不能超过 {MAX_MQTT_SUBSCRIPTIONS}"));
        }
        for subscription in &self.subscriptions {
            subscription.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttUserProperty {
    pub name: String,
    pub value: String,
}

impl MqttUserProperty {
    pub fn validate(&self) -> Result<(), String> {
        validate_text(
            "MQTT 5 User Property 名称",
            &self.name,
            MAX_MQTT_USER_PROPERTY_BYTES,
        )?;
        validate_text(
            "MQTT 5 User Property 值",
            &self.value,
            MAX_MQTT_USER_PROPERTY_BYTES,
        )
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttPublishRequest {
    pub topic: String,
    pub payload: Vec<u8>,
    #[serde(default)]
    pub qos: MqttQos,
    #[serde(default)]
    pub retain: bool,
    #[serde(default)]
    pub user_properties: Vec<MqttUserProperty>,
}

impl fmt::Debug for MqttPublishRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MqttPublishRequest")
            .field("topic", &self.topic)
            .field("payload_bytes", &self.payload.len())
            .field("qos", &self.qos)
            .field("retain", &self.retain)
            .field("user_property_count", &self.user_properties.len())
            .finish()
    }
}

impl MqttPublishRequest {
    pub fn validate(&self) -> Result<(), String> {
        validate_mqtt_topic_name(&self.topic)?;
        validate_payload_len(self.payload.len())?;
        validate_user_properties(&self.user_properties)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttPublishResult {
    pub topic: String,
    #[serde(default)]
    pub packet_id: Option<u16>,
    pub qos: MqttQos,
}

impl MqttPublishResult {
    pub fn validate(&self) -> Result<(), String> {
        validate_mqtt_topic_name(&self.topic)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttMessage {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: MqttQos,
    #[serde(default)]
    pub retain: bool,
    #[serde(default)]
    pub duplicate: bool,
    pub received_at: DateTime<Utc>,
    #[serde(default)]
    pub user_properties: Vec<MqttUserProperty>,
}

impl fmt::Debug for MqttMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MqttMessage")
            .field("topic", &self.topic)
            .field("payload_bytes", &self.payload.len())
            .field("qos", &self.qos)
            .field("retain", &self.retain)
            .field("duplicate", &self.duplicate)
            .field("received_at", &self.received_at)
            .field("user_property_count", &self.user_properties.len())
            .finish()
    }
}

impl MqttMessage {
    pub fn validate(&self) -> Result<(), String> {
        validate_mqtt_topic_name(&self.topic)?;
        validate_payload_len(self.payload.len())?;
        validate_user_properties(&self.user_properties)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MqttMessageSinkResult {
    Accepted,
    Backpressured,
    Closed,
}

pub type MqttMessageSink = Arc<dyn Fn(MqttMessage) -> MqttMessageSinkResult + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MqttTopicSource {
    Observed,
    Retained,
    Acl,
    Sys,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttTopicObservation {
    pub name: String,
    pub source: MqttTopicSource,
    #[serde(default)]
    pub retained: bool,
    #[serde(default)]
    pub observed_at: Option<DateTime<Utc>>,
}

impl MqttTopicObservation {
    pub fn validate(&self) -> Result<(), String> {
        validate_mqtt_topic_name(&self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttOnlineClient {
    pub client_id: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub remote_address: Option<String>,
    #[serde(default)]
    pub connected_at: Option<DateTime<Utc>>,
}

impl MqttOnlineClient {
    pub fn validate(&self) -> Result<(), String> {
        validate_text(
            "在线客户端 Client ID",
            &self.client_id,
            MAX_MOSQUITTO_NAME_BYTES,
        )?;
        validate_optional_text(
            "在线客户端用户名",
            self.username.as_deref(),
            MAX_MOSQUITTO_NAME_BYTES,
        )?;
        validate_optional_text(
            "在线客户端地址",
            self.remote_address.as_deref(),
            MAX_MOSQUITTO_NAME_BYTES,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttBrokerSnapshot {
    #[serde(default)]
    pub topics: Vec<MqttTopicObservation>,
    #[serde(default)]
    pub online_clients: Vec<MqttOnlineClient>,
    #[serde(default)]
    pub topics_complete: bool,
    #[serde(default)]
    pub online_clients_complete: bool,
}

impl MqttBrokerSnapshot {
    pub fn validate(self) -> Result<Self, String> {
        if self.topics.len() > MAX_MQTT_TOPIC_OBSERVATIONS {
            return Err(format!(
                "MQTT Topic 观察记录不能超过 {MAX_MQTT_TOPIC_OBSERVATIONS}"
            ));
        }
        if self.online_clients.len() > MAX_MQTT_ONLINE_CLIENTS {
            return Err(format!("MQTT 在线客户端不能超过 {MAX_MQTT_ONLINE_CLIENTS}"));
        }
        for topic in &self.topics {
            topic.validate()?;
        }
        for client in &self.online_clients {
            client.validate()?;
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MosquittoDynamicSecuritySnapshot {
    #[serde(default)]
    pub clients: Vec<MosquittoClient>,
    #[serde(default)]
    pub groups: Vec<MosquittoGroup>,
    #[serde(default)]
    pub roles: Vec<MosquittoRole>,
}

impl MosquittoDynamicSecuritySnapshot {
    pub fn validate(self) -> Result<Self, String> {
        if self.clients.len() > MAX_MOSQUITTO_CLIENTS {
            return Err(format!(
                "Mosquitto 客户端数量不能超过 {MAX_MOSQUITTO_CLIENTS}"
            ));
        }
        if self.groups.len() > MAX_MOSQUITTO_GROUPS {
            return Err(format!(
                "Mosquitto Group 数量不能超过 {MAX_MOSQUITTO_GROUPS}"
            ));
        }
        if self.roles.len() > MAX_MOSQUITTO_ROLES {
            return Err(format!("Mosquitto Role 数量不能超过 {MAX_MOSQUITTO_ROLES}"));
        }
        for client in &self.clients {
            client.validate()?;
        }
        for group in &self.groups {
            group.validate()?;
        }
        for role in &self.roles {
            role.validate()?;
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MosquittoStaticFileKind {
    Password,
    Acl,
}

impl MosquittoStaticFileKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Password => "password_file",
            Self::Acl => "acl_file",
        }
    }

    pub fn configured_path(self, profile: &MqttProfile) -> Option<&str> {
        profile
            .management
            .static_config
            .as_ref()
            .and_then(|config| match self {
                Self::Password => config.password_file.as_deref(),
                Self::Acl => config.acl_file.as_deref(),
            })
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MosquittoStaticFile {
    pub kind: MosquittoStaticFileKind,
    pub path: String,
    pub content: String,
}

impl fmt::Debug for MosquittoStaticFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MosquittoStaticFile")
            .field("kind", &self.kind)
            .field("path", &"[CONFIGURED]")
            .field("content_bytes", &self.content.len())
            .finish()
    }
}

impl MosquittoStaticFile {
    pub fn validate(&self) -> Result<(), String> {
        if self.path.trim().is_empty() {
            return Err(format!("Mosquitto {} 路径不能为空", self.kind.label()));
        }
        validate_text(
            "Mosquitto 静态配置路径",
            &self.path,
            MAX_MQTT_TLS_PATH_BYTES,
        )?;
        if self.content.len() > MAX_MOSQUITTO_STATIC_FILE_BYTES {
            return Err(format!(
                "Mosquitto 静态配置内容不能超过 {MAX_MOSQUITTO_STATIC_FILE_BYTES} bytes"
            ));
        }
        if self.content.contains('\0') {
            return Err("Mosquitto 静态配置不能包含 NUL 字符".into());
        }
        Ok(())
    }
}

fn validate_payload_len(length: usize) -> Result<(), String> {
    if length > MAX_MQTT_PUBLISH_PAYLOAD_BYTES {
        return Err(format!(
            "MQTT 消息负载不能超过 {MAX_MQTT_PUBLISH_PAYLOAD_BYTES} bytes"
        ));
    }
    Ok(())
}

fn validate_user_properties(properties: &[MqttUserProperty]) -> Result<(), String> {
    if properties.len() > MAX_MQTT_USER_PROPERTIES {
        return Err(format!(
            "MQTT 5 User Property 数量不能超过 {MAX_MQTT_USER_PROPERTIES}"
        ));
    }
    for property in properties {
        property.validate()?;
    }
    Ok(())
}

fn validate_optional_text(
    label: &str,
    value: Option<&str>,
    max_bytes: usize,
) -> Result<(), String> {
    if let Some(value) = value {
        validate_text(label, value, max_bytes)?;
    }
    Ok(())
}

fn validate_text(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.len() > max_bytes {
        return Err(format!(
            "{label}过长：{} bytes，最多 {max_bytes} bytes",
            value.len()
        ));
    }
    if value.contains('\0') || value.chars().any(char::is_control) {
        return Err(format!("{label}不能包含控制字符"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_debug_and_message_debug_do_not_include_payload() {
        let publish = MqttPublishRequest {
            topic: "devices/one/state".into(),
            payload: b"private-payload".to_vec(),
            qos: MqttQos::AtLeastOnce,
            retain: true,
            user_properties: vec![],
        };
        assert!(publish.validate().is_ok());
        assert!(!format!("{publish:?}").contains("private-payload"));

        let message = MqttMessage {
            topic: publish.topic.clone(),
            payload: publish.payload,
            qos: publish.qos,
            retain: publish.retain,
            duplicate: false,
            received_at: Utc::now(),
            user_properties: vec![],
        };
        assert!(message.validate().is_ok());
        assert!(!format!("{message:?}").contains("private-payload"));
    }

    #[test]
    fn broker_snapshot_marks_incomplete_sources_without_faking_data() {
        let snapshot = MqttBrokerSnapshot {
            topics: vec![],
            online_clients: vec![],
            topics_complete: false,
            online_clients_complete: false,
        };
        assert!(!snapshot.topics_complete);
        assert!(!snapshot.online_clients_complete);
        assert!(snapshot.validate().is_ok());
    }

    #[test]
    fn static_file_debug_redacts_content_and_dynamic_snapshot_is_bounded() {
        let file = MosquittoStaticFile {
            kind: MosquittoStaticFileKind::Password,
            path: "/etc/mosquitto/passwd".into(),
            content: "secret-hash".into(),
        };
        assert!(file.validate().is_ok());
        let debug = format!("{file:?}");
        assert!(!debug.contains("secret-hash"));

        let snapshot = MosquittoDynamicSecuritySnapshot {
            clients: vec![],
            groups: vec![],
            roles: vec![],
        };
        assert!(snapshot.validate().is_ok());
    }
}
