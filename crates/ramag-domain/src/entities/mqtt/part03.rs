
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
        if matches!(
            self.acl_type,
            MosquittoAclType::Subscribe | MosquittoAclType::Unsubscribe
        ) {
            return Err(
                "Mosquitto Dynamic Security 不支持未区分类型的 subscribe 或 unsubscribe ACL".into(),
            );
        }
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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MosquittoClient {
    pub username: String,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub password_configured: bool,
    /// 新建或修改客户端时可选的新密码；读取 Dynamic Security 返回值时始终为空。
    #[serde(default, skip_serializing)]
    pub password: Option<String>,
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

impl fmt::Debug for MosquittoClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MosquittoClient")
            .field("username", &self.username)
            .field("client_id", &self.client_id)
            .field("password_configured", &self.password_configured)
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .field("disabled", &self.disabled)
            .field("text_name", &self.text_name)
            .field("text_description", &self.text_description)
            .field("groups", &self.groups)
            .field("roles", &self.roles)
            .finish()
    }
}

impl MosquittoClient {
    pub fn validate(&self) -> Result<(), String> {
        validate_required_text("客户端用户名", &self.username, MAX_MOSQUITTO_NAME_BYTES)?;
        validate_optional_protocol_text(
            "客户端密码",
            self.password.as_deref(),
            MAX_MQTT_PASSWORD_BYTES,
        )?;
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
    /// 是否允许此 Role 使用包含通配符的订阅 ACL；该值必须与 Broker 返回值保持一致。
    #[serde(default)]
    pub allow_wildcards_subscriptions: bool,
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

