//! Docker Registry v2 的连接和只读查询模型。

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_CONTAINER_REGISTRY_NAME_BYTES: usize = 256;
pub const MAX_CONTAINER_REGISTRY_ENDPOINT_BYTES: usize = 2 * 1024;
pub const MAX_CONTAINER_REGISTRY_REPOSITORY_BYTES: usize = 512;
pub const MAX_CONTAINER_REGISTRY_TAG_BYTES: usize = 512;
pub const MAX_CONTAINER_REGISTRY_CREDENTIAL_BYTES: usize = 64 * 1024;
pub const MAX_CONTAINER_REGISTRY_REPOSITORIES: usize = 20_000;
pub const MAX_CONTAINER_REGISTRY_TAGS: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainerRegistryId(pub Uuid);

impl ContainerRegistryId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ContainerRegistryId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ContainerRegistryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Registry 地址和凭据引用；密码不放在连接配置中。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerRegistryProfile {
    pub id: ContainerRegistryId,
    pub name: String,
    pub endpoint: String,
    #[serde(default)]
    pub credential_ref: Option<String>,
    #[serde(default)]
    pub allow_insecure_http: bool,
}

impl ContainerRegistryProfile {
    pub fn new(name: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            id: ContainerRegistryId::new(),
            name: name.into(),
            endpoint: endpoint.into(),
            credential_ref: None,
            allow_insecure_http: false,
        }
    }

    pub fn with_credential_ref(mut self, credential_ref: impl Into<String>) -> Self {
        self.credential_ref = Some(credential_ref.into());
        self
    }

    pub fn with_insecure_http(mut self, allow: bool) -> Self {
        self.allow_insecure_http = allow;
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_text(
            "Registry 名称",
            &self.name,
            MAX_CONTAINER_REGISTRY_NAME_BYTES,
            false,
        )?;
        validate_text(
            "Registry 端点",
            &self.endpoint,
            MAX_CONTAINER_REGISTRY_ENDPOINT_BYTES,
            false,
        )?;
        if self.endpoint.starts_with("https://") {
            return Ok(());
        }
        if self.allow_insecure_http && self.endpoint.starts_with("http://") {
            return Ok(());
        }
        Err("Registry 端点必须使用 HTTPS；本机测试若使用 HTTP 必须显式允许不安全连接".into())
    }
}

/// 凭据只在基础设施调用边界短暂存在，Debug 输出不会泄露密码。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerRegistryCredential {
    pub username: String,
    pub password: String,
}

impl fmt::Debug for ContainerRegistryCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContainerRegistryCredential")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

impl ContainerRegistryCredential {
    pub fn validate(&self) -> Result<(), String> {
        validate_text(
            "Registry 用户名",
            &self.username,
            MAX_CONTAINER_REGISTRY_CREDENTIAL_BYTES,
            false,
        )?;
        validate_text(
            "Registry 密码",
            &self.password,
            MAX_CONTAINER_REGISTRY_CREDENTIAL_BYTES,
            true,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerRegistryInfo {
    pub registry_id: ContainerRegistryId,
    pub api_version: Option<String>,
    pub server_version: Option<String>,
    pub authenticated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerRegistryRepository {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerRegistryTag {
    pub repository: String,
    pub name: String,
}

fn validate_text(
    field: &str,
    value: &str,
    max_bytes: usize,
    allow_empty: bool,
) -> Result<(), String> {
    if !allow_empty && value.trim().is_empty() {
        return Err(format!("{field}不能为空"));
    }
    if value.len() > max_bytes {
        return Err(format!("{field}超过 {max_bytes} bytes 限制"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field}不能包含控制字符"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_requires_tls_unless_insecure_http_is_explicit() {
        let profile = ContainerRegistryProfile::new("prod", "http://registry.example.test");
        assert!(profile.validate().is_err());
        assert!(profile.clone().with_insecure_http(true).validate().is_ok());
        assert!(
            ContainerRegistryProfile::new("prod", "https://registry.example.test")
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn credential_debug_redacts_password() {
        let credential = ContainerRegistryCredential {
            username: "alice".into(),
            password: "secret-value".into(),
        };
        let debug = format!("{credential:?}");
        assert!(debug.contains("alice"));
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("secret-value"));
    }
}
