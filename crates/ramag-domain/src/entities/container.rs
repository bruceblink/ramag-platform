//! Docker 与 Kubernetes 连接配置的领域模型。
//!
//! CMT-001 只定义本地连接身份和平台区分，不负责调用 Docker Engine 或 Kubernetes API。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_CONTAINER_ENDPOINT_NAME_BYTES: usize = 256;
pub const MAX_CONTAINER_ENDPOINT_ADDRESS_BYTES: usize = 2 * 1024;
pub const MAX_CONTAINER_CONTEXT_BYTES: usize = 256;
pub const MAX_CONTAINER_NAMESPACE_BYTES: usize = 256;
pub const MAX_CONTAINER_ENDPOINTS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainerEndpointId(pub Uuid);

impl ContainerEndpointId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ContainerEndpointId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ContainerEndpointId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContainerPlatform {
    Docker,
    Kubernetes,
}

impl ContainerPlatform {
    pub const ALL: [Self; 2] = [Self::Docker, Self::Kubernetes];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Docker => "Docker",
            Self::Kubernetes => "Kubernetes",
        }
    }

    pub const fn is_docker(self) -> bool {
        matches!(self, Self::Docker)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerEndpointProfile {
    pub id: ContainerEndpointId,
    pub name: String,
    pub platform: ContainerPlatform,
    /// Docker socket/TCP address or Kubernetes API Server address；敏感凭据不放在这里。
    pub address: String,
    /// Kubernetes kubeconfig context；Docker 配置不使用该字段。
    #[serde(default)]
    pub context: Option<String>,
    /// Kubernetes 默认命名空间；Docker 配置不使用该字段。
    #[serde(default)]
    pub namespace: Option<String>,
    /// 新连接默认只读；后续写操作必须再经过连接策略和逐项确认。
    #[serde(default = "default_read_only")]
    pub read_only: bool,
}

fn default_read_only() -> bool {
    true
}

impl ContainerEndpointProfile {
    pub fn new_docker(name: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            id: ContainerEndpointId::new(),
            name: name.into(),
            platform: ContainerPlatform::Docker,
            address: address.into(),
            context: None,
            namespace: None,
            read_only: true,
        }
    }

    pub fn new_kubernetes(
        name: impl Into<String>,
        address: impl Into<String>,
        context: impl Into<String>,
    ) -> Self {
        Self {
            id: ContainerEndpointId::new(),
            name: name.into(),
            platform: ContainerPlatform::Kubernetes,
            address: address.into(),
            context: Some(context.into()),
            namespace: None,
            read_only: true,
        }
    }

    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// 校验连接身份和平台专属字段，避免同一条配置被另一种资源语义误读。
    pub fn validate(&self) -> Result<(), String> {
        validate_single_line(
            "容器连接名称",
            &self.name,
            MAX_CONTAINER_ENDPOINT_NAME_BYTES,
            false,
        )?;
        validate_single_line(
            "容器连接地址",
            &self.address,
            MAX_CONTAINER_ENDPOINT_ADDRESS_BYTES,
            false,
        )?;

        match self.platform {
            ContainerPlatform::Docker => {
                if self.context.is_some() || self.namespace.is_some() {
                    return Err("Docker 连接不支持 Kubernetes context 或命名空间".into());
                }
            }
            ContainerPlatform::Kubernetes => {
                let context = self
                    .context
                    .as_deref()
                    .ok_or_else(|| "Kubernetes 连接必须指定 context".to_string())?;
                validate_single_line(
                    "Kubernetes context",
                    context,
                    MAX_CONTAINER_CONTEXT_BYTES,
                    false,
                )?;
                if let Some(namespace) = self.namespace.as_deref() {
                    validate_single_line(
                        "Kubernetes 命名空间",
                        namespace,
                        MAX_CONTAINER_NAMESPACE_BYTES,
                        false,
                    )?;
                }
            }
        }
        Ok(())
    }
}

fn validate_single_line(
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
    if value
        .chars()
        .any(|character| character == '\r' || character == '\n')
    {
        return Err(format!("{field}不能包含换行"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_profile_defaults_to_read_only_and_validates() {
        let profile =
            ContainerEndpointProfile::new_docker("本机 Docker", "npipe:////./pipe/docker_engine");
        assert_eq!(profile.platform, ContainerPlatform::Docker);
        assert!(profile.read_only);
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn kubernetes_profile_keeps_context_and_namespace_separate() {
        let profile = ContainerEndpointProfile::new_kubernetes(
            "开发集群",
            "https://kubernetes.example.test",
            "dev-context",
        )
        .with_namespace("ramag-test");
        assert_eq!(profile.platform, ContainerPlatform::Kubernetes);
        assert_eq!(profile.context.as_deref(), Some("dev-context"));
        assert_eq!(profile.namespace.as_deref(), Some("ramag-test"));
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn validation_rejects_cross_platform_fields_and_missing_context() {
        let mut docker =
            ContainerEndpointProfile::new_docker("Docker", "unix:///var/run/docker.sock");
        docker.context = Some("unexpected-context".into());
        assert!(docker.validate().is_err());

        let kubernetes = ContainerEndpointProfile {
            id: ContainerEndpointId::new(),
            name: "Kubernetes".into(),
            platform: ContainerPlatform::Kubernetes,
            address: "https://kubernetes.example.test".into(),
            context: None,
            namespace: None,
            read_only: true,
        };
        assert!(kubernetes.validate().is_err());
    }

    #[test]
    fn validation_rejects_multiline_identity_fields() {
        let profile =
            ContainerEndpointProfile::new_docker("Docker\nprod", "unix:///var/run/docker.sock");
        assert!(profile.validate().is_err());
    }
}
