//! Docker 与 Kubernetes 连接配置及 Docker 只读资源模型。
//!
//! 连接配置属于 CMT-001；资源模型供 CMT-002 的应用接口和 Docker 适配器共同使用。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_CONTAINER_ENDPOINT_NAME_BYTES: usize = 256;
pub const MAX_CONTAINER_ENDPOINT_ADDRESS_BYTES: usize = 2 * 1024;
pub const MAX_CONTAINER_CONTEXT_BYTES: usize = 256;
pub const MAX_CONTAINER_NAMESPACE_BYTES: usize = 256;
pub const MAX_CONTAINER_ENDPOINTS: usize = 256;
pub const MAX_CONTAINER_RESOURCE_ITEMS: usize = 20_000;
pub const MAX_CONTAINER_PAGE_SIZE: usize = 500;
pub const DEFAULT_CONTAINER_PAGE_SIZE: usize = 100;
pub const MAX_CONTAINER_QUERY_BYTES: usize = 512;
pub const MAX_CONTAINER_RESOURCE_ID_BYTES: usize = 4 * 1024;
pub const MAX_CONTAINER_LABELS: usize = 512;
pub const MAX_CONTAINER_PORTS: usize = 256;
pub const MAX_CONTAINER_MOUNTS: usize = 512;
pub const MAX_CONTAINER_NETWORKS: usize = 512;
pub const MAX_CONTAINER_REPOSITORY_REFERENCES: usize = 2_048;
pub const MAX_CONTAINER_IMAGE_REFERENCE_BYTES: usize = 4 * 1024;

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

    pub fn local_docker(name: impl Into<String>) -> Self {
        let address = if cfg!(windows) {
            "npipe:////./pipe/docker_engine"
        } else {
            "unix:///var/run/docker.sock"
        };
        Self::new_docker(name, address)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerImageOperationKind {
    Pull,
    Tag,
    Push,
    Delete,
}

impl ContainerImageOperationKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pull => "拉取镜像",
            Self::Tag => "标记镜像",
            Self::Push => "推送镜像",
            Self::Delete => "删除本地镜像",
        }
    }

    pub const fn destructive(self) -> bool {
        matches!(self, Self::Delete)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerImageOperationRequest {
    pub operation: ContainerImageOperationKind,
    pub source_reference: String,
    #[serde(default)]
    pub target_reference: Option<String>,
    #[serde(default)]
    pub force: bool,
}

impl ContainerImageOperationRequest {
    pub fn new(
        operation: ContainerImageOperationKind,
        source_reference: impl Into<String>,
    ) -> Self {
        Self {
            operation,
            source_reference: source_reference.into(),
            target_reference: None,
            force: false,
        }
    }

    pub fn with_target(mut self, target_reference: impl Into<String>) -> Self {
        self.target_reference = Some(target_reference.into());
        self
    }

    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_image_reference("镜像源引用", &self.source_reference)?;
        if let Some(target) = &self.target_reference {
            validate_image_reference("镜像目标引用", target)?;
        }
        if matches!(self.operation, ContainerImageOperationKind::Tag)
            && self.target_reference.is_none()
        {
            return Err("标记镜像必须指定目标引用".into());
        }
        if !matches!(self.operation, ContainerImageOperationKind::Tag)
            && self.target_reference.is_some()
        {
            return Err("只有标记镜像允许指定目标引用".into());
        }
        if !matches!(self.operation, ContainerImageOperationKind::Delete) && self.force {
            return Err("只有删除本地镜像允许强制选项".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerImageOperationPreview {
    pub endpoint_id: ContainerEndpointId,
    pub operation: ContainerImageOperationKind,
    pub source_reference: String,
    pub target_reference: Option<String>,
    pub force: bool,
    pub destructive: bool,
    pub requires_confirmation: bool,
    pub can_execute: bool,
    pub blocked_reason: Option<String>,
}

fn validate_image_reference(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field}不能为空"));
    }
    if value.len() > MAX_CONTAINER_IMAGE_REFERENCE_BYTES {
        return Err(format!(
            "{field}超过 {MAX_CONTAINER_IMAGE_REFERENCE_BYTES} bytes 限制"
        ));
    }
    if value
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(format!("{field}不能包含空白或控制字符"));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerListQuery {
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
    #[serde(default)]
    pub search: Option<String>,
}

const fn default_page() -> usize {
    1
}

const fn default_page_size() -> usize {
    DEFAULT_CONTAINER_PAGE_SIZE
}

impl Default for ContainerListQuery {
    fn default() -> Self {
        Self {
            page: default_page(),
            page_size: default_page_size(),
            search: None,
        }
    }
}

impl ContainerListQuery {
    pub fn validate(&self) -> Result<(), String> {
        if self.page == 0 {
            return Err("资源页码必须从 1 开始".into());
        }
        if self.page_size == 0 || self.page_size > MAX_CONTAINER_PAGE_SIZE {
            return Err(format!(
                "资源分页大小必须在 1 到 {MAX_CONTAINER_PAGE_SIZE} 之间"
            ));
        }
        if let Some(search) = self.search.as_deref() {
            validate_bounded_text("资源筛选词", search, MAX_CONTAINER_QUERY_BYTES, true)?;
        }
        Ok(())
    }

    pub fn normalized_search(&self) -> Option<String> {
        self.search
            .as_deref()
            .map(str::trim)
            .filter(|search| !search.is_empty())
            .map(str::to_lowercase)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerPage<T> {
    pub items: Vec<T>,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerEngineVersion {
    pub api_version: Option<String>,
    pub min_api_version: Option<String>,
    pub server_version: Option<String>,
    pub build_time: Option<String>,
    pub git_commit: Option<String>,
    pub go_version: Option<String>,
    pub os: Option<String>,
    pub architecture: Option<String>,
    pub kernel_version: Option<String>,
    pub experimental: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerResourceCounts {
    pub containers: usize,
    pub running_containers: usize,
    pub paused_containers: usize,
    pub stopped_containers: usize,
    pub images: usize,
    pub networks: Option<usize>,
    pub volumes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerConnectionInfo {
    pub endpoint_id: ContainerEndpointId,
    pub api_version: Option<String>,
    pub server_version: Option<String>,
    pub server_name: Option<String>,
    pub operating_system: Option<String>,
    pub architecture: Option<String>,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerOverview {
    pub connection: DockerConnectionInfo,
    pub version: DockerEngineVersion,
    pub counts: DockerResourceCounts,
    pub cpu_count: Option<usize>,
    pub memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerLabel {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerContainerPort {
    pub ip: Option<String>,
    pub private_port: Option<u16>,
    pub public_port: Option<u16>,
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerContainerSummary {
    pub id: String,
    pub names: Vec<String>,
    pub image: Option<String>,
    pub image_id: Option<String>,
    pub command: Option<String>,
    pub created: Option<i64>,
    pub state: Option<String>,
    pub status: Option<String>,
    pub health: Option<String>,
    pub ports: Vec<DockerContainerPort>,
    pub networks: Vec<String>,
    pub labels: Vec<DockerLabel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerMountSummary {
    pub source: Option<String>,
    pub target: Option<String>,
    pub kind: Option<String>,
    pub read_only: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerNetworkAttachment {
    pub name: Option<String>,
    pub network_id: Option<String>,
    pub endpoint_id: Option<String>,
    pub gateway: Option<String>,
    pub ip_address: Option<String>,
    pub mac_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerContainerDetail {
    pub summary: DockerContainerSummary,
    pub path: Option<String>,
    pub args: Vec<String>,
    pub platform: Option<String>,
    pub working_directory: Option<String>,
    pub entrypoint: Vec<String>,
    pub command: Vec<String>,
    pub restart_policy: Option<String>,
    pub env_keys: Vec<String>,
    pub mounts: Vec<DockerMountSummary>,
    pub networks: Vec<DockerNetworkAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageSummary {
    pub id: String,
    pub repository_tags: Vec<String>,
    pub repository_digests: Vec<String>,
    pub created: Option<i64>,
    pub size_bytes: Option<i64>,
    pub shared_size_bytes: Option<i64>,
    pub containers: Option<i64>,
    pub architecture: Option<String>,
    pub operating_system: Option<String>,
    pub labels: Vec<DockerLabel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageDetail {
    pub summary: DockerImageSummary,
    pub parent_id: Option<String>,
    pub comment: Option<String>,
    pub author: Option<String>,
    pub docker_version: Option<String>,
    pub container_config_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerNetworkSubnet {
    pub subnet: Option<String>,
    pub gateway: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerNetworkSummary {
    pub id: String,
    pub name: Option<String>,
    pub driver: Option<String>,
    pub scope: Option<String>,
    pub internal: Option<bool>,
    pub attachable: Option<bool>,
    pub ingress: Option<bool>,
    pub labels: Vec<DockerLabel>,
    pub container_count: usize,
    pub subnets: Vec<DockerNetworkSubnet>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerNetworkDetail {
    pub summary: DockerNetworkSummary,
    pub options: BTreeMap<String, String>,
    pub containers: Vec<DockerNetworkAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerVolumeSummary {
    pub name: String,
    pub driver: Option<String>,
    pub mountpoint: Option<String>,
    pub scope: Option<String>,
    pub labels: Vec<DockerLabel>,
    pub container_count: usize,
    pub usage_size_bytes: Option<i64>,
    pub usage_reference_count: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerVolumeDetail {
    pub summary: DockerVolumeSummary,
    pub options: BTreeMap<String, String>,
    pub status: BTreeMap<String, String>,
}

fn validate_bounded_text(
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

    #[test]
    fn local_docker_profile_uses_platform_default_endpoint() {
        let profile = ContainerEndpointProfile::local_docker("本机 Docker");
        assert!(profile.address.starts_with("unix://") || profile.address.starts_with("npipe://"));
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn list_query_bounds_page_and_search_without_empty_filter() {
        let query = ContainerListQuery {
            page: 2,
            page_size: 50,
            search: Some("  nginx  ".into()),
        };
        assert!(query.validate().is_ok());
        assert_eq!(query.normalized_search().as_deref(), Some("nginx"));
        assert!(ContainerListQuery { page: 0, ..query }.validate().is_err());
    }
}
