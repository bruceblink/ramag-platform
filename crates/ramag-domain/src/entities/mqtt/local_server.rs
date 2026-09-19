#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttLocalServerConfig {
    #[serde(default = "default_local_server_host")]
    pub bind_host: String,
    #[serde(default = "default_local_server_port")]
    pub port: u16,
    #[serde(default = "default_local_server_max_connections")]
    pub max_connections: u32,
    #[serde(default)]
    pub allow_anonymous: bool,
    #[serde(default)]
    pub users: Vec<MqttLocalServerUser>,
}

impl Default for MqttLocalServerConfig {
    fn default() -> Self {
        Self {
            bind_host: default_local_server_host(),
            port: default_local_server_port(),
            max_connections: default_local_server_max_connections(),
            allow_anonymous: true,
            users: Vec::new(),
        }
    }
}

impl fmt::Debug for MqttLocalServerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MqttLocalServerConfig")
            .field("bind_host", &self.bind_host)
            .field("port", &self.port)
            .field("max_connections", &self.max_connections)
            .field("allow_anonymous", &self.allow_anonymous)
            .field("users", &self.users)
            .finish()
    }
}

impl MqttLocalServerConfig {
    /// 校验监听地址、连接上限和固定账号，拒绝 Broker 启动后才暴露的配置错误。
    pub fn validate(&self) -> Result<(), String> {
        let host = self.bind_host.trim();
        if host.is_empty() {
            return Err("本地 MQTT Broker 监听地址不能为空".into());
        }
        if host.len() > MAX_MQTT_LOCAL_SERVER_HOST_BYTES {
            return Err(format!(
                "本地 MQTT Broker 监听地址不能超过 {MAX_MQTT_LOCAL_SERVER_HOST_BYTES} 字节"
            ));
        }
        host.parse::<std::net::IpAddr>()
            .map_err(|_| "本地 MQTT Broker 监听地址必须是 IPv4 或 IPv6 地址".to_string())?;
        if self.port == 0 {
            return Err("本地 MQTT Broker 端口必须是 1 - 65535".into());
        }
        if self.max_connections == 0 {
            return Err("本地 MQTT Broker 连接上限必须大于 0".into());
        }
        if self.max_connections > MAX_MQTT_LOCAL_SERVER_MAX_CONNECTIONS {
            return Err(format!(
                "本地 MQTT Broker 连接上限不能超过 {MAX_MQTT_LOCAL_SERVER_MAX_CONNECTIONS}"
            ));
        }
        if self.users.len() > 128 {
            return Err("本地 MQTT Broker 用户数量不能超过 128".into());
        }
        if !self.allow_anonymous && self.users.is_empty() {
            return Err("关闭匿名连接时至少需要配置一个固定账号".into());
        }
        for (index, user) in self.users.iter().enumerate() {
            user.validate(index)?;
        }
        for (index, user) in self.users.iter().enumerate() {
            if self.users[index + 1..]
                .iter()
                .any(|other| other.username == user.username)
            {
                return Err(format!("本地 MQTT Broker 用户名重复：{}", user.username));
            }
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttLocalServerUser {
    pub username: String,
    pub password: String,
}

impl fmt::Debug for MqttLocalServerUser {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MqttLocalServerUser")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

impl MqttLocalServerUser {
    fn validate(&self, index: usize) -> Result<(), String> {
        if self.username.trim().is_empty() {
            return Err(format!("本地 MQTT Broker 第 {} 个用户名不能为空", index + 1));
        }
        if self.username.len() > MAX_MQTT_USERNAME_BYTES {
            return Err(format!(
                "本地 MQTT Broker 用户名不能超过 {MAX_MQTT_USERNAME_BYTES} 字节"
            ));
        }
        if self.username.chars().any(char::is_control) {
            return Err("本地 MQTT Broker 用户名不能包含控制字符".into());
        }
        if self.password.len() > MAX_MQTT_PASSWORD_BYTES {
            return Err(format!(
                "本地 MQTT Broker 用户密码不能超过 {MAX_MQTT_PASSWORD_BYTES} 字节"
            ));
        }
        if self.password.is_empty() {
            return Err(format!(
                "本地 MQTT Broker 第 {} 个用户密码不能为空",
                index + 1
            ));
        }
        if self.password.chars().any(char::is_control) {
            return Err("本地 MQTT Broker 用户密码不能包含控制字符".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttLocalServerStatus {
    pub running: bool,
    pub bind_host: String,
    pub port: u16,
    #[serde(default = "default_local_server_max_connections")]
    pub max_connections: u32,
    pub allow_anonymous: bool,
}

impl MqttLocalServerStatus {
    pub fn stopped(config: &MqttLocalServerConfig) -> Self {
        Self {
            running: false,
            bind_host: config.bind_host.clone(),
            port: config.port,
            max_connections: config.max_connections,
            allow_anonymous: config.allow_anonymous,
        }
    }

    pub fn running(config: &MqttLocalServerConfig) -> Self {
        Self {
            running: true,
            bind_host: config.bind_host.clone(),
            port: config.port,
            max_connections: config.max_connections,
            allow_anonymous: config.allow_anonymous,
        }
    }
}

fn default_local_server_host() -> String {
    DEFAULT_MQTT_LOCAL_SERVER_HOST.to_string()
}

const fn default_local_server_port() -> u16 {
    DEFAULT_MQTT_PORT
}

const fn default_local_server_max_connections() -> u32 {
    DEFAULT_MQTT_LOCAL_SERVER_MAX_CONNECTIONS
}
