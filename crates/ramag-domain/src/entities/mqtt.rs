//! MQTT 连接配置、协议边界和 Mosquitto 管理对象。
//!
//! MQTT 的连接数据面、Mosquitto Dynamic Security 管理面和静态文件管理面
//! 使用同一份配置保存，但能力由后续适配器单独声明，不把 Broker 管理接口
//! 混入通用 MQTT 协议。

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{TlsVerify, ssh::SshProfileId};

pub const DEFAULT_MQTT_PORT: u16 = 1883;
pub const DEFAULT_MQTT_TLS_PORT: u16 = 8883;
pub const DEFAULT_MQTT_KEEP_ALIVE_SECONDS: u16 = 60;
pub const MAX_MQTT_PROFILES: usize = 512;
pub const MAX_MQTT_PROFILE_NAME_BYTES: usize = 256;
pub const MAX_MQTT_HOST_BYTES: usize = 1024;
pub const MAX_MQTT_CLIENT_ID_BYTES: usize = 256;
pub const MAX_MQTT_USERNAME_BYTES: usize = 1024;
pub const MAX_MQTT_PASSWORD_BYTES: usize = 64 * 1024;
pub const MAX_MQTT_REMARK_BYTES: usize = 16 * 1024;
pub const MAX_MQTT_TLS_PATH_BYTES: usize = 32 * 1024;
pub const MAX_MQTT_TOPIC_BYTES: usize = u16::MAX as usize;
pub const MAX_MOSQUITTO_NAME_BYTES: usize = 256;
pub const MAX_MOSQUITTO_DESCRIPTION_BYTES: usize = 16 * 1024;
pub const MAX_MOSQUITTO_ACLS: usize = 4096;
pub const MAX_MQTT_PROFILE_RECORD_BYTES: usize = 1024 * 1024;
pub const MAX_MQTT_PROFILE_LIST_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MqttProfileId(pub Uuid);

impl MqttProfileId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MqttProfileId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MqttProfileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MqttProtocolVersion {
    #[default]
    V5,
    V311,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MqttTransport {
    #[default]
    Tcp,
    Tls,
}

impl MqttTransport {
    pub const fn default_port(self) -> u16 {
        match self {
            Self::Tcp => DEFAULT_MQTT_PORT,
            Self::Tls => DEFAULT_MQTT_TLS_PORT,
        }
    }

    pub const fn uses_tls(self) -> bool {
        matches!(self, Self::Tls)
    }
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttTlsConfig {
    #[serde(default)]
    pub verify: TlsVerify,
    #[serde(default)]
    pub ca_cert_path: Option<String>,
    #[serde(default)]
    pub client_cert_path: Option<String>,
    #[serde(default)]
    pub client_key_path: Option<String>,
}

impl fmt::Debug for MqttTlsConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MqttTlsConfig")
            .field("verify", &self.verify)
            .field(
                "ca_cert_path",
                &self.ca_cert_path.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "client_cert_path",
                &self.client_cert_path.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "client_key_path",
                &self.client_key_path.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

impl MqttTlsConfig {
    fn validate(&self) -> Result<(), String> {
        validate_optional_path("CA 证书路径", self.ca_cert_path.as_deref())?;
        validate_optional_path("客户端证书路径", self.client_cert_path.as_deref())?;
        validate_optional_path("客户端密钥路径", self.client_key_path.as_deref())?;
        if self.client_cert_path.is_some() != self.client_key_path.is_some() {
            return Err("客户端证书和客户端密钥必须同时配置".into());
        }
        Ok(())
    }

    fn has_certificate_paths(&self) -> bool {
        self.ca_cert_path.is_some()
            || self.client_cert_path.is_some()
            || self.client_key_path.is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MosquittoConfigTarget {
    #[default]
    Local,
    Ssh {
        profile_id: SshProfileId,
    },
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MosquittoStaticConfig {
    #[serde(default)]
    pub target: MosquittoConfigTarget,
    pub password_file: Option<String>,
    pub acl_file: Option<String>,
}

impl fmt::Debug for MosquittoStaticConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MosquittoStaticConfig")
            .field("target", &self.target)
            .field(
                "password_file",
                &self.password_file.as_ref().map(|_| "[CONFIGURED]"),
            )
            .field("acl_file", &self.acl_file.as_ref().map(|_| "[CONFIGURED]"))
            .finish()
    }
}

impl MosquittoStaticConfig {
    fn validate(&self) -> Result<(), String> {
        validate_optional_path("password_file 路径", self.password_file.as_deref())?;
        validate_optional_path("acl_file 路径", self.acl_file.as_deref())?;
        if self.password_file.is_none() && self.acl_file.is_none() {
            return Err("Mosquitto 静态配置至少需要 password_file 或 acl_file".into());
        }
        Ok(())
    }
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MosquittoManagementConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub admin_username: Option<String>,
    #[serde(default)]
    pub admin_password: Option<String>,
    #[serde(default)]
    pub static_config: Option<MosquittoStaticConfig>,
}

impl fmt::Debug for MosquittoManagementConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MosquittoManagementConfig")
            .field("enabled", &self.enabled)
            .field(
                "admin_username",
                &self.admin_username.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "admin_password",
                &self.admin_password.as_ref().map(|_| "[REDACTED]"),
            )
            .field("static_config", &self.static_config)
            .finish()
    }
}

impl MosquittoManagementConfig {
    fn validate(&self) -> Result<(), String> {
        validate_optional_protocol_text(
            "管理用户名",
            self.admin_username.as_deref(),
            MAX_MQTT_USERNAME_BYTES,
        )?;
        validate_optional_protocol_text(
            "管理密码",
            self.admin_password.as_deref(),
            MAX_MQTT_PASSWORD_BYTES,
        )?;
        if self.admin_password.is_some() && self.admin_username.as_deref().is_none_or(str::is_empty)
        {
            return Err("设置 Mosquitto 管理密码时必须同时设置管理用户名".into());
        }
        if let Some(static_config) = &self.static_config {
            static_config.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttProfile {
    pub id: MqttProfileId,
    pub name: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub transport: MqttTransport,
    #[serde(default)]
    pub protocol_version: MqttProtocolVersion,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub tls: MqttTlsConfig,
    #[serde(default = "default_keep_alive")]
    pub keep_alive_seconds: u16,
    #[serde(default)]
    pub clean_start: bool,
    #[serde(default)]
    pub session_expiry_seconds: Option<u32>,
    #[serde(default)]
    pub management: MosquittoManagementConfig,
    #[serde(default)]
    pub remark: Option<String>,
}

impl fmt::Debug for MqttProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MqttProfile")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("transport", &self.transport)
            .field("protocol_version", &self.protocol_version)
            .field("client_id", &self.client_id)
            .field("username", &self.username.as_ref().map(|_| "[REDACTED]"))
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .field("tls", &self.tls)
            .field("keep_alive_seconds", &self.keep_alive_seconds)
            .field("clean_start", &self.clean_start)
            .field("session_expiry_seconds", &self.session_expiry_seconds)
            .field("management", &self.management)
            .field("remark", &self.remark)
            .finish()
    }
}

impl MqttProfile {
    pub fn new(name: impl Into<String>, host: impl Into<String>, port: u16) -> Self {
        Self {
            id: MqttProfileId::new(),
            name: name.into(),
            host: host.into(),
            port,
            transport: MqttTransport::default(),
            protocol_version: MqttProtocolVersion::default(),
            client_id: None,
            username: None,
            password: None,
            tls: MqttTlsConfig::default(),
            keep_alive_seconds: DEFAULT_MQTT_KEEP_ALIVE_SECONDS,
            clean_start: true,
            session_expiry_seconds: None,
            management: MosquittoManagementConfig::default(),
            remark: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_required_text("MQTT 配置名称", &self.name, MAX_MQTT_PROFILE_NAME_BYTES)?;
        validate_required_host(&self.host)?;
        if self.port == 0 {
            return Err("MQTT 端口必须是 1 - 65535".into());
        }
        if self.keep_alive_seconds != 0 && self.keep_alive_seconds < 5 {
            return Err("MQTT Keep Alive 必须为 0 或至少 5 秒".into());
        }
        if self.protocol_version == MqttProtocolVersion::V311
            && self.session_expiry_seconds.is_some()
        {
            return Err("MQTT 3.1.1 不能设置 MQTT 5 会话过期时间".into());
        }
        validate_optional_single_line(
            "Client ID",
            self.client_id.as_deref(),
            MAX_MQTT_CLIENT_ID_BYTES,
        )?;
        validate_optional_protocol_text(
            "用户名",
            self.username.as_deref(),
            MAX_MQTT_USERNAME_BYTES,
        )?;
        validate_optional_protocol_text("密码", self.password.as_deref(), MAX_MQTT_PASSWORD_BYTES)?;
        if self.password.is_some() && self.username.as_deref().is_none_or(str::is_empty) {
            return Err("设置 MQTT 密码时必须同时设置用户名".into());
        }
        validate_optional_single_line("备注", self.remark.as_deref(), MAX_MQTT_REMARK_BYTES)?;
        self.tls.validate()?;
        if !self.transport.uses_tls() && self.tls.has_certificate_paths() {
            return Err("非 TLS MQTT 连接不能设置证书路径".into());
        }
        if self.management.enabled {
            self.management.validate()?;
        } else if self.management.admin_username.is_some()
            || self.management.admin_password.is_some()
            || self.management.static_config.is_some()
        {
            return Err("未启用 Mosquitto 管理时不能设置管理参数".into());
        }
        Ok(())
    }
}

fn default_keep_alive() -> u16 {
    DEFAULT_MQTT_KEEP_ALIVE_SECONDS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MqttTransportBackend {
    Native,
    TestDouble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttTransportCapabilities {
    pub backend: MqttTransportBackend,
    pub build_available: bool,
    pub mqtt311: bool,
    pub mqtt5: bool,
    pub tcp: bool,
    pub tls: bool,
    pub subscribe: bool,
    pub publish: bool,
    pub dynamic_security: bool,
    pub static_config: bool,
    pub metrics: bool,
    pub retained_topics: bool,
    pub online_clients: bool,
}

impl MqttTransportCapabilities {
    pub const fn unknown() -> Self {
        Self {
            backend: MqttTransportBackend::TestDouble,
            build_available: false,
            mqtt311: false,
            mqtt5: false,
            tcp: false,
            tls: false,
            subscribe: false,
            publish: false,
            dynamic_security: false,
            static_config: false,
            metrics: false,
            retained_topics: false,
            online_clients: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MqttTransportCapability {
    Mqtt311,
    Mqtt5,
    Tcp,
    Tls,
    Subscribe,
    Publish,
    DynamicSecurity,
    StaticConfig,
    Metrics,
    RetainedTopics,
    OnlineClients,
}

impl MqttTransportCapabilities {
    pub const fn supports(self, capability: MqttTransportCapability) -> bool {
        match capability {
            MqttTransportCapability::Mqtt311 => self.mqtt311,
            MqttTransportCapability::Mqtt5 => self.mqtt5,
            MqttTransportCapability::Tcp => self.tcp,
            MqttTransportCapability::Tls => self.tls,
            MqttTransportCapability::Subscribe => self.subscribe,
            MqttTransportCapability::Publish => self.publish,
            MqttTransportCapability::DynamicSecurity => self.dynamic_security,
            MqttTransportCapability::StaticConfig => self.static_config,
            MqttTransportCapability::Metrics => self.metrics,
            MqttTransportCapability::RetainedTopics => self.retained_topics,
            MqttTransportCapability::OnlineClients => self.online_clients,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MosquittoAclType {
    PublishClientSend,
    PublishClientReceive,
    Subscribe,
    Unsubscribe,
    SubscribeLiteral,
    SubscribePattern,
    UnsubscribeLiteral,
    UnsubscribePattern,
}

impl MosquittoAclType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PublishClientSend => "publishClientSend",
            Self::PublishClientReceive => "publishClientReceive",
            Self::Subscribe => "subscribe",
            Self::Unsubscribe => "unsubscribe",
            Self::SubscribeLiteral => "subscribeLiteral",
            Self::SubscribePattern => "subscribePattern",
            Self::UnsubscribeLiteral => "unsubscribeLiteral",
            Self::UnsubscribePattern => "unsubscribePattern",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MosquittoAclDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MosquittoAcl {
    pub acl_type: MosquittoAclType,
    pub topic: String,
    pub decision: MosquittoAclDecision,
    #[serde(default = "default_acl_priority")]
    pub priority: i32,
}

impl MosquittoAcl {
    pub fn validate(&self) -> Result<(), String> {
        validate_mqtt_topic_filter(&self.topic)?;
        if self.priority < -1 || self.priority > 100_000 {
            return Err("Mosquitto ACL 优先级必须是 -1 到 100000".into());
        }
        Ok(())
    }
}

fn default_acl_priority() -> i32 {
    -1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MosquittoRoleBinding {
    pub role_name: String,
    #[serde(default = "default_acl_priority")]
    pub priority: i32,
}

impl MosquittoRoleBinding {
    fn validate(&self) -> Result<(), String> {
        validate_required_text("Role 名称", &self.role_name, MAX_MOSQUITTO_NAME_BYTES)?;
        if self.priority < -1 || self.priority > 100_000 {
            return Err("Role 优先级必须是 -1 到 100000".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MosquittoGroupBinding {
    pub group_name: String,
    #[serde(default = "default_acl_priority")]
    pub priority: i32,
}

impl MosquittoGroupBinding {
    fn validate(&self) -> Result<(), String> {
        validate_required_text("Group 名称", &self.group_name, MAX_MOSQUITTO_NAME_BYTES)?;
        if self.priority < -1 || self.priority > 100_000 {
            return Err("Group 优先级必须是 -1 到 100000".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MosquittoClient {
    pub username: String,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub password_configured: bool,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub text_name: Option<String>,
    #[serde(default)]
    pub text_description: Option<String>,
    #[serde(default)]
    pub groups: Vec<MosquittoGroupBinding>,
    #[serde(default)]
    pub roles: Vec<MosquittoRoleBinding>,
}

impl MosquittoClient {
    pub fn validate(&self) -> Result<(), String> {
        validate_required_text("客户端用户名", &self.username, MAX_MOSQUITTO_NAME_BYTES)?;
        validate_optional_single_line(
            "客户端 Client ID",
            self.client_id.as_deref(),
            MAX_MQTT_CLIENT_ID_BYTES,
        )?;
        validate_optional_single_line(
            "客户端显示名称",
            self.text_name.as_deref(),
            MAX_MOSQUITTO_NAME_BYTES,
        )?;
        validate_optional_text(
            "客户端描述",
            self.text_description.as_deref(),
            MAX_MOSQUITTO_DESCRIPTION_BYTES,
        )?;
        for group in &self.groups {
            group.validate()?;
        }
        for role in &self.roles {
            role.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MosquittoGroup {
    pub group_name: String,
    #[serde(default)]
    pub text_name: Option<String>,
    #[serde(default)]
    pub text_description: Option<String>,
    #[serde(default)]
    pub roles: Vec<MosquittoRoleBinding>,
}

impl MosquittoGroup {
    pub fn validate(&self) -> Result<(), String> {
        validate_required_text("Group 名称", &self.group_name, MAX_MOSQUITTO_NAME_BYTES)?;
        validate_optional_single_line(
            "Group 显示名称",
            self.text_name.as_deref(),
            MAX_MOSQUITTO_NAME_BYTES,
        )?;
        validate_optional_text(
            "Group 描述",
            self.text_description.as_deref(),
            MAX_MOSQUITTO_DESCRIPTION_BYTES,
        )?;
        for role in &self.roles {
            role.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MosquittoRole {
    pub role_name: String,
    #[serde(default)]
    pub text_name: Option<String>,
    #[serde(default)]
    pub text_description: Option<String>,
    #[serde(default)]
    pub acls: Vec<MosquittoAcl>,
}

impl MosquittoRole {
    pub fn validate(&self) -> Result<(), String> {
        validate_required_text("Role 名称", &self.role_name, MAX_MOSQUITTO_NAME_BYTES)?;
        validate_optional_single_line(
            "Role 显示名称",
            self.text_name.as_deref(),
            MAX_MOSQUITTO_NAME_BYTES,
        )?;
        validate_optional_text(
            "Role 描述",
            self.text_description.as_deref(),
            MAX_MOSQUITTO_DESCRIPTION_BYTES,
        )?;
        if self.acls.len() > MAX_MOSQUITTO_ACLS {
            return Err(format!("Role ACL 数量不能超过 {MAX_MOSQUITTO_ACLS}"));
        }
        for acl in &self.acls {
            acl.validate()?;
        }
        Ok(())
    }
}

pub fn validate_mqtt_topic_name(topic: &str) -> Result<(), String> {
    validate_protocol_text("MQTT Topic", topic, MAX_MQTT_TOPIC_BYTES)?;
    if topic.is_empty() {
        return Err("MQTT Topic 不能为空".into());
    }
    if topic.contains('+') || topic.contains('#') {
        return Err("MQTT 发布 Topic 不能包含 + 或 # 通配符".into());
    }
    Ok(())
}

pub fn validate_mqtt_topic_filter(filter: &str) -> Result<(), String> {
    validate_protocol_text("MQTT Topic Filter", filter, MAX_MQTT_TOPIC_BYTES)?;
    if filter.is_empty() {
        return Err("MQTT Topic Filter 不能为空".into());
    }

    let filter = if let Some(shared) = filter.strip_prefix("$share/") {
        let mut parts = shared.splitn(2, '/');
        let group = parts.next().unwrap_or_default();
        let filter = parts.next().unwrap_or_default();
        if group.is_empty() || group.contains('+') || group.contains('#') {
            return Err("共享订阅的 Group 名称无效".into());
        }
        if filter.is_empty() {
            return Err("共享订阅缺少 Topic Filter".into());
        }
        filter
    } else {
        filter
    };

    let levels = filter.split('/').collect::<Vec<_>>();
    for (index, level) in levels.iter().enumerate() {
        if level.contains('#') && (*level != "#" || index + 1 != levels.len()) {
            return Err("MQTT # 必须独占最后一个 Topic 层级".into());
        }
        if level.contains('+') && *level != "+" {
            return Err("MQTT + 必须独占一个 Topic 层级".into());
        }
    }
    Ok(())
}

fn validate_required_host(host: &str) -> Result<(), String> {
    validate_protocol_text("MQTT Broker 地址", host, MAX_MQTT_HOST_BYTES)?;
    if host.trim().is_empty() {
        return Err("MQTT Broker 地址不能为空".into());
    }
    if host.contains("://") || host.chars().any(char::is_whitespace) {
        return Err("MQTT Broker 地址不能包含协议前缀或空白字符".into());
    }
    Ok(())
}

fn validate_required_text(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label}不能为空"));
    }
    validate_single_line(label, value, max_bytes)
}

fn validate_optional_single_line(
    label: &str,
    value: Option<&str>,
    max_bytes: usize,
) -> Result<(), String> {
    if let Some(value) = value {
        validate_single_line(label, value, max_bytes)?;
    }
    Ok(())
}

fn validate_optional_protocol_text(
    label: &str,
    value: Option<&str>,
    max_bytes: usize,
) -> Result<(), String> {
    if let Some(value) = value {
        validate_protocol_text(label, value, max_bytes)?;
    }
    Ok(())
}

fn validate_optional_text(
    label: &str,
    value: Option<&str>,
    max_bytes: usize,
) -> Result<(), String> {
    if let Some(value) = value {
        validate_protocol_text(label, value, max_bytes)?;
    }
    Ok(())
}

fn validate_optional_path(label: &str, value: Option<&str>) -> Result<(), String> {
    if let Some(value) = value {
        validate_required_text(label, value, MAX_MQTT_TLS_PATH_BYTES)?;
    }
    Ok(())
}

fn validate_single_line(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    validate_protocol_text(label, value, max_bytes)?;
    if value.chars().any(char::is_control) {
        return Err(format!("{label}不能包含控制字符"));
    }
    Ok(())
}

fn validate_protocol_text(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.len() > max_bytes {
        return Err(format!(
            "{label}过长：{} bytes，最多 {max_bytes} bytes",
            value.len()
        ));
    }
    if value.contains('\0') {
        return Err(format!("{label}不能包含 NUL 字符"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mqtt_profile_defaults_are_valid_and_debug_redacts_secrets() {
        let mut profile = MqttProfile::new("local", "127.0.0.1", DEFAULT_MQTT_PORT);
        profile.username = Some("operator".into());
        profile.password = Some("secret-password".into());

        assert!(profile.validate().is_ok());
        let debug = format!("{profile:?}");
        assert!(!debug.contains("secret-password"));
        assert!(!debug.contains("operator"));
    }

    #[test]
    fn mqtt_profile_rejects_incomplete_auth_and_plaintext_tls_paths() {
        let mut profile = MqttProfile::new("local", "localhost", DEFAULT_MQTT_PORT);
        profile.password = Some("secret".into());
        assert!(profile.validate().is_err());

        profile.password = None;
        profile.tls.ca_cert_path = Some("ca.pem".into());
        assert!(profile.validate().is_err());

        profile.transport = MqttTransport::Tls;
        profile.tls.ca_cert_path = None;
        profile.tls.client_cert_path = Some("client.pem".into());
        assert!(profile.validate().is_err());

        profile.tls.client_key_path = Some("client.key".into());
        profile.keep_alive_seconds = 1;
        assert!(profile.validate().is_err());
    }

    #[test]
    fn topic_name_and_filter_rules_match_mqtt_wildcard_boundaries() {
        assert!(validate_mqtt_topic_name("devices/one/state").is_ok());
        assert!(validate_mqtt_topic_name("devices/+/state").is_err());
        assert!(validate_mqtt_topic_filter("devices/+/state").is_ok());
        assert!(validate_mqtt_topic_filter("devices/#").is_ok());
        assert!(validate_mqtt_topic_filter("devices/#/state").is_err());
        assert!(validate_mqtt_topic_filter("$share/workers/devices/#").is_ok());
        assert!(validate_mqtt_topic_filter("$share//devices/#").is_err());
    }

    #[test]
    fn mosquitto_acl_and_role_validation_bounds_priority_and_acl_count() {
        let acl = MosquittoAcl {
            acl_type: MosquittoAclType::SubscribePattern,
            topic: "devices/%u/#".into(),
            decision: MosquittoAclDecision::Allow,
            priority: 10,
        };
        let role = MosquittoRole {
            role_name: "operator".into(),
            text_name: None,
            text_description: None,
            acls: vec![acl],
        };
        assert!(role.validate().is_ok());

        let invalid = MosquittoAcl {
            priority: 100_001,
            ..role.acls[0].clone()
        };
        assert!(invalid.validate().is_err());
    }
}
