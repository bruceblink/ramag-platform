
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
