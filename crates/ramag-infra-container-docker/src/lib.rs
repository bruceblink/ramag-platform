#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

//! Docker Engine API 的只读适配器。
//!
//! 该 crate 不暴露 bollard 类型给应用层；每次查询在独立 Tokio 运行时中执行，
//! 通过领域模型返回有界的资源摘要和详情。

use async_trait::async_trait;
use bollard::{
    Docker,
    auth::DockerCredentials,
    errors::Error as BollardError,
    query_parameters::{
        CreateImageOptionsBuilder, ListContainersOptionsBuilder, PushImageOptionsBuilder,
        RemoveImageOptionsBuilder, TagImageOptionsBuilder,
    },
};
use futures::StreamExt;
use ramag_domain::{
    ContainerDriver,
    entities::{
        ContainerEndpointProfile, ContainerImageOperationKind, ContainerImageOperationRequest,
        ContainerImageOperationResult, ContainerListQuery, ContainerPage,
        ContainerRegistryCredential, DockerConnectionInfo, DockerContainerDetail,
        DockerContainerPort, DockerContainerSummary, DockerEngineVersion, DockerImageDetail,
        DockerImageSummary, DockerLabel, DockerMountSummary, DockerNetworkAttachment,
        DockerNetworkDetail, DockerNetworkSubnet, DockerNetworkSummary, DockerOverview,
        DockerResourceCounts, DockerVolumeDetail, DockerVolumeSummary, MAX_CONTAINER_LABELS,
        MAX_CONTAINER_MOUNTS, MAX_CONTAINER_NETWORKS, MAX_CONTAINER_PORTS,
        MAX_CONTAINER_REPOSITORY_REFERENCES, MAX_CONTAINER_RESOURCE_ID_BYTES,
        MAX_CONTAINER_RESOURCE_ITEMS,
    },
    error::{ContainerError, ContainerErrorCategory, DomainError, READ_ONLY_MESSAGE, Result},
};
use serde::Serialize;
use serde_json::Value;
use tracing::debug;

const MAX_DOCKER_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default)]
pub struct DockerDriver;

impl DockerDriver {
    pub const fn new() -> Self {
        Self
    }

    fn validate_profile(profile: &ContainerEndpointProfile) -> Result<()> {
        profile.validate().map_err(DomainError::InvalidConfig)?;
        if !profile.platform.is_docker() {
            return Err(DomainError::InvalidConfig(
                "Docker 适配器不能读取 Kubernetes 连接".into(),
            ));
        }
        let address = profile.address.to_ascii_lowercase();
        if address.starts_with("tcp://") || address.starts_with("http://") {
            return Err(DomainError::Container(ContainerError::new(
                ContainerErrorCategory::Tls,
                "创建 Docker 连接",
                "远程 Docker TCP 端点必须使用 HTTPS/TLS",
            )));
        }
        if !(address.starts_with("unix://")
            || address.starts_with("npipe://")
            || address.starts_with("https://"))
        {
            return Err(DomainError::Container(ContainerError::new(
                ContainerErrorCategory::Unsupported,
                "创建 Docker 连接",
                "Docker 连接只支持 Unix socket、Windows named pipe 或 HTTPS",
            )));
        }
        Ok(())
    }

    fn connect(profile: &ContainerEndpointProfile) -> Result<Docker> {
        Self::validate_profile(profile)?;
        Docker::connect_with_host(&profile.address)
            .map_err(|error| map_bollard_error("创建 Docker 连接", error))
    }

    async fn connect_and<T, F>(
        profile: &ContainerEndpointProfile,
        operation: &'static str,
        action: F,
    ) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(
                Docker,
                ContainerEndpointProfile,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>
            + Send
            + 'static,
    {
        let profile = profile.clone();
        let endpoint = profile.clone();
        smol::unblock(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| runtime_error(operation))?;
            runtime.block_on(async move {
                let docker = Self::connect(&endpoint)?
                    .negotiate_version()
                    .await
                    .map_err(|error| map_bollard_error(operation, error))?;
                action(docker, profile).await
            })
        })
        .await
    }

    async fn test_connection_async(
        docker: Docker,
        profile: ContainerEndpointProfile,
    ) -> Result<DockerConnectionInfo> {
        let version = docker
            .version()
            .await
            .map_err(|error| map_bollard_error("测试 Docker 连接", error))?;
        let info = docker
            .info()
            .await
            .map_err(|error| map_bollard_error("读取 Docker Engine 信息", error))?;
        let version = json_value(&version, "Docker 版本响应")?;
        let info = json_value(&info, "Docker Engine 信息响应")?;
        Ok(connection_info(&profile, &version, &info))
    }

    async fn overview_async(
        docker: Docker,
        profile: ContainerEndpointProfile,
    ) -> Result<DockerOverview> {
        let version = docker
            .version()
            .await
            .map_err(|error| map_bollard_error("读取 Docker 版本", error))?;
        let info = docker
            .info()
            .await
            .map_err(|error| map_bollard_error("读取 Docker Engine 概览", error))?;
        let containers = docker
            .list_containers(Some(
                ListContainersOptionsBuilder::default().all(true).build(),
            ))
            .await
            .map_err(|error| map_bollard_error("读取 Docker 容器数量", error))?;
        let images = docker
            .list_images(None::<bollard::query_parameters::ListImagesOptions>)
            .await
            .map_err(|error| map_bollard_error("读取 Docker 镜像数量", error))?;
        let networks = docker
            .list_networks(None)
            .await
            .map_err(|error| map_bollard_error("读取 Docker 网络数量", error))?;
        let volumes = docker
            .list_volumes(None::<bollard::query_parameters::ListVolumesOptions>)
            .await
            .map_err(|error| map_bollard_error("读取 Docker 数据卷数量", error))?;
        ensure_response_size(&containers, "读取 Docker 容器数量")?;
        ensure_response_size(&images, "读取 Docker 镜像数量")?;
        ensure_response_size(&networks, "读取 Docker 网络数量")?;
        ensure_response_size(&volumes, "读取 Docker 数据卷数量")?;
        let version_value = json_value(&version, "Docker 版本响应")?;
        let info_value = json_value(&info, "Docker Engine 信息响应")?;
        let counts = DockerResourceCounts {
            containers: containers.len(),
            running_containers: number(&info_value, &["ContainersRunning", "containers_running"])
                .unwrap_or_else(|| count_container_state(&containers, "running")),
            paused_containers: number(&info_value, &["ContainersPaused", "containers_paused"])
                .unwrap_or_else(|| count_container_state(&containers, "paused")),
            stopped_containers: number(&info_value, &["ContainersStopped", "containers_stopped"])
                .unwrap_or_else(|| count_container_state(&containers, "exited")),
            images: images.len(),
            networks: Some(networks.len()),
            volumes: volumes.volumes.as_ref().map(Vec::len),
        };
        Ok(DockerOverview {
            connection: connection_info(&profile, &version_value, &info_value),
            version: engine_version(&version_value),
            counts,
            cpu_count: number(&info_value, &["NCPU", "ncpu"]),
            memory_bytes: u64_number(&info_value, &["MemTotal", "mem_total"]),
        })
    }

    async fn list_containers_async(
        docker: Docker,
        _profile: ContainerEndpointProfile,
        query: ContainerListQuery,
    ) -> Result<ContainerPage<DockerContainerSummary>> {
        let values = docker
            .list_containers(Some(
                ListContainersOptionsBuilder::default().all(true).build(),
            ))
            .await
            .map_err(|error| map_bollard_error("读取 Docker 容器列表", error))?;
        let values = json_values(&values, "Docker 容器列表响应")?;
        let mut items = Vec::new();
        for value in values.into_iter().take(MAX_CONTAINER_RESOURCE_ITEMS) {
            let value = json_value(&value, "Docker 容器响应")?;
            let summary = container_summary(&value)?;
            if matches_search(&summary, query.normalized_search().as_deref()) {
                items.push(summary);
            }
        }
        Ok(paginate(items, &query))
    }

    async fn get_container_async(
        docker: Docker,
        _profile: ContainerEndpointProfile,
        id: String,
    ) -> Result<DockerContainerDetail> {
        let value = docker
            .inspect_container(&id, None)
            .await
            .map_err(|error| map_bollard_error("读取 Docker 容器详情", error))?;
        let value = json_value(&value, "Docker 容器详情响应")?;
        container_detail(&value)
    }

    async fn list_images_async(
        docker: Docker,
        _profile: ContainerEndpointProfile,
        query: ContainerListQuery,
    ) -> Result<ContainerPage<DockerImageSummary>> {
        let values = docker
            .list_images(None::<bollard::query_parameters::ListImagesOptions>)
            .await
            .map_err(|error| map_bollard_error("读取 Docker 镜像列表", error))?;
        let values = json_values(&values, "Docker 镜像列表响应")?;
        let mut items = Vec::new();
        for value in values.into_iter().take(MAX_CONTAINER_RESOURCE_ITEMS) {
            let value = json_value(&value, "Docker 镜像响应")?;
            let summary = image_summary(&value)?;
            if matches_search(&summary, query.normalized_search().as_deref()) {
                items.push(summary);
            }
        }
        Ok(paginate(items, &query))
    }

    async fn get_image_async(
        docker: Docker,
        _profile: ContainerEndpointProfile,
        id: String,
    ) -> Result<DockerImageDetail> {
        let value = docker
            .inspect_image(&id)
            .await
            .map_err(|error| map_bollard_error("读取 Docker 镜像详情", error))?;
        let value = json_value(&value, "Docker 镜像详情响应")?;
        image_detail(&value)
    }

    async fn execute_image_operation_async(
        docker: Docker,
        profile: ContainerEndpointProfile,
        request: ContainerImageOperationRequest,
        credential: Option<ContainerRegistryCredential>,
    ) -> Result<ContainerImageOperationResult> {
        if profile.read_only {
            return Err(DomainError::Forbidden(READ_ONLY_MESSAGE.into()));
        }
        request.validate().map_err(DomainError::InvalidConfig)?;
        if let Some(credential) = &credential {
            credential.validate().map_err(DomainError::InvalidConfig)?;
        }
        let docker_credentials = credential.as_ref().map(docker_credentials);
        let image_id = match request.operation {
            ContainerImageOperationKind::Pull => {
                let options = CreateImageOptionsBuilder::default()
                    .from_image(&request.source_reference)
                    .build();
                let mut stream =
                    docker.create_image(Some(options), None, docker_credentials.clone());
                let mut image_id = None;
                while let Some(item) = stream.next().await {
                    let item =
                        item.map_err(|error| map_bollard_error("拉取 Docker 镜像", error))?;
                    if item.error_detail.is_some() {
                        return Err(image_operation_error("Docker 拉取镜像返回错误"));
                    }
                    if item.id.is_some() {
                        image_id = item.id;
                    }
                }
                image_id
            }
            ContainerImageOperationKind::Tag => {
                let target = request
                    .target_reference
                    .as_deref()
                    .ok_or_else(|| DomainError::InvalidConfig("标记镜像必须指定目标引用".into()))?;
                let (repository, tag) = split_tag_reference(target)?;
                docker
                    .tag_image(
                        &request.source_reference,
                        Some(
                            TagImageOptionsBuilder::default()
                                .repo(&repository)
                                .tag(&tag)
                                .build(),
                        ),
                    )
                    .await
                    .map_err(|error| map_bollard_error("标记 Docker 镜像", error))?;
                None
            }
            ContainerImageOperationKind::Push => {
                let options = match split_optional_tag(&request.source_reference)? {
                    Some(tag) => PushImageOptionsBuilder::default().tag(&tag).build(),
                    None => PushImageOptionsBuilder::default().build(),
                };
                let mut stream =
                    docker.push_image(&request.source_reference, Some(options), docker_credentials);
                while let Some(item) = stream.next().await {
                    let item =
                        item.map_err(|error| map_bollard_error("推送 Docker 镜像", error))?;
                    if item.error_detail.is_some() {
                        return Err(image_operation_error("Docker 推送镜像返回错误"));
                    }
                }
                None
            }
            ContainerImageOperationKind::Delete => {
                let items = docker
                    .remove_image(
                        &request.source_reference,
                        Some(
                            RemoveImageOptionsBuilder::default()
                                .force(request.force)
                                .noprune(false)
                                .build(),
                        ),
                        None,
                    )
                    .await
                    .map_err(|error| map_bollard_error("删除 Docker 镜像", error))?;
                items
                    .into_iter()
                    .find_map(|item| item.deleted.or(item.untagged))
            }
        };
        Ok(ContainerImageOperationResult {
            operation: request.operation,
            source_reference: request.source_reference,
            target_reference: request.target_reference,
            image_id,
        })
    }

    async fn list_networks_async(
        docker: Docker,
        _profile: ContainerEndpointProfile,
        query: ContainerListQuery,
    ) -> Result<ContainerPage<DockerNetworkSummary>> {
        let values = docker
            .list_networks(None)
            .await
            .map_err(|error| map_bollard_error("读取 Docker 网络列表", error))?;
        let values = json_values(&values, "Docker 网络列表响应")?;
        let mut items = Vec::new();
        for value in values.into_iter().take(MAX_CONTAINER_NETWORKS) {
            let value = json_value(&value, "Docker 网络响应")?;
            let summary = network_summary(&value)?;
            if matches_search(&summary, query.normalized_search().as_deref()) {
                items.push(summary);
            }
        }
        Ok(paginate(items, &query))
    }

    async fn get_network_async(
        docker: Docker,
        _profile: ContainerEndpointProfile,
        id: String,
    ) -> Result<DockerNetworkDetail> {
        let value = docker
            .inspect_network(&id, None)
            .await
            .map_err(|error| map_bollard_error("读取 Docker 网络详情", error))?;
        let value = json_value(&value, "Docker 网络详情响应")?;
        network_detail(&value)
    }

    async fn list_volumes_async(
        docker: Docker,
        _profile: ContainerEndpointProfile,
        query: ContainerListQuery,
    ) -> Result<ContainerPage<DockerVolumeSummary>> {
        let response = docker
            .list_volumes(None::<bollard::query_parameters::ListVolumesOptions>)
            .await
            .map_err(|error| map_bollard_error("读取 Docker 数据卷列表", error))?;
        let values = response.volumes.unwrap_or_default();
        let values = json_values(&values, "Docker 数据卷列表响应")?;
        let mut items = Vec::new();
        for value in values.into_iter().take(MAX_CONTAINER_RESOURCE_ITEMS) {
            let summary = volume_summary(&value)?;
            if matches_search(&summary, query.normalized_search().as_deref()) {
                items.push(summary);
            }
        }
        Ok(paginate(items, &query))
    }

    async fn get_volume_async(
        docker: Docker,
        _profile: ContainerEndpointProfile,
        name: String,
    ) -> Result<DockerVolumeDetail> {
        let value = docker
            .inspect_volume(&name)
            .await
            .map_err(|error| map_bollard_error("读取 Docker 数据卷详情", error))?;
        let value = json_value(&value, "Docker 数据卷详情响应")?;
        volume_detail(&value)
    }
}

#[async_trait]
impl ContainerDriver for DockerDriver {
    async fn test_connection(
        &self,
        profile: &ContainerEndpointProfile,
    ) -> Result<DockerConnectionInfo> {
        Self::connect_and(profile, "测试 Docker 连接", |docker, profile| {
            Box::pin(Self::test_connection_async(docker, profile))
        })
        .await
    }

    async fn overview(&self, profile: &ContainerEndpointProfile) -> Result<DockerOverview> {
        Self::connect_and(profile, "读取 Docker 概览", |docker, profile| {
            Box::pin(Self::overview_async(docker, profile))
        })
        .await
    }

    async fn list_containers(
        &self,
        profile: &ContainerEndpointProfile,
        query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerContainerSummary>> {
        query.validate().map_err(DomainError::InvalidConfig)?;
        let query = query.clone();
        Self::connect_and(
            profile,
            "读取 Docker 容器列表",
            move |docker, profile| Box::pin(Self::list_containers_async(docker, profile, query)),
        )
        .await
    }

    async fn get_container(
        &self,
        profile: &ContainerEndpointProfile,
        id: &str,
    ) -> Result<DockerContainerDetail> {
        let id = validate_resource_id(id, "容器 ID")?;
        Self::connect_and(
            profile,
            "读取 Docker 容器详情",
            move |docker, profile| Box::pin(Self::get_container_async(docker, profile, id)),
        )
        .await
    }

    async fn list_images(
        &self,
        profile: &ContainerEndpointProfile,
        query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerImageSummary>> {
        query.validate().map_err(DomainError::InvalidConfig)?;
        let query = query.clone();
        Self::connect_and(
            profile,
            "读取 Docker 镜像列表",
            move |docker, profile| Box::pin(Self::list_images_async(docker, profile, query)),
        )
        .await
    }

    async fn get_image(
        &self,
        profile: &ContainerEndpointProfile,
        id: &str,
    ) -> Result<DockerImageDetail> {
        let id = validate_resource_id(id, "镜像 ID")?;
        Self::connect_and(
            profile,
            "读取 Docker 镜像详情",
            move |docker, profile| Box::pin(Self::get_image_async(docker, profile, id)),
        )
        .await
    }

    async fn execute_image_operation(
        &self,
        profile: &ContainerEndpointProfile,
        request: &ContainerImageOperationRequest,
        credential: Option<&ContainerRegistryCredential>,
    ) -> Result<ContainerImageOperationResult> {
        profile.validate().map_err(DomainError::InvalidConfig)?;
        request.validate().map_err(DomainError::InvalidConfig)?;
        if let Some(credential) = credential {
            credential.validate().map_err(DomainError::InvalidConfig)?;
        }
        if profile.read_only {
            return Err(DomainError::Forbidden(READ_ONLY_MESSAGE.into()));
        }
        let request = request.clone();
        let credential = credential.cloned();
        Self::connect_and(
            profile,
            "执行 Docker 镜像操作",
            move |docker, profile| {
                Box::pin(Self::execute_image_operation_async(
                    docker, profile, request, credential,
                ))
            },
        )
        .await
    }

    async fn list_networks(
        &self,
        profile: &ContainerEndpointProfile,
        query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerNetworkSummary>> {
        query.validate().map_err(DomainError::InvalidConfig)?;
        let query = query.clone();
        Self::connect_and(
            profile,
            "读取 Docker 网络列表",
            move |docker, profile| Box::pin(Self::list_networks_async(docker, profile, query)),
        )
        .await
    }

    async fn get_network(
        &self,
        profile: &ContainerEndpointProfile,
        id: &str,
    ) -> Result<DockerNetworkDetail> {
        let id = validate_resource_id(id, "网络 ID")?;
        Self::connect_and(
            profile,
            "读取 Docker 网络详情",
            move |docker, profile| Box::pin(Self::get_network_async(docker, profile, id)),
        )
        .await
    }

    async fn list_volumes(
        &self,
        profile: &ContainerEndpointProfile,
        query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerVolumeSummary>> {
        query.validate().map_err(DomainError::InvalidConfig)?;
        let query = query.clone();
        Self::connect_and(
            profile,
            "读取 Docker 数据卷列表",
            move |docker, profile| Box::pin(Self::list_volumes_async(docker, profile, query)),
        )
        .await
    }

    async fn get_volume(
        &self,
        profile: &ContainerEndpointProfile,
        name: &str,
    ) -> Result<DockerVolumeDetail> {
        let name = validate_resource_id(name, "数据卷名称")?;
        Self::connect_and(
            profile,
            "读取 Docker 数据卷详情",
            move |docker, profile| Box::pin(Self::get_volume_async(docker, profile, name)),
        )
        .await
    }
}

fn runtime_error(operation: &'static str) -> DomainError {
    DomainError::Container(ContainerError::new(
        ContainerErrorCategory::Unknown,
        operation,
        "初始化 Docker 查询运行时失败",
    ))
}

fn docker_credentials(credential: &ContainerRegistryCredential) -> DockerCredentials {
    DockerCredentials {
        username: Some(credential.username.clone()),
        password: Some(credential.password.clone()),
        ..DockerCredentials::default()
    }
}

fn split_optional_tag(reference: &str) -> Result<Option<String>> {
    if reference.contains('@') {
        return Ok(None);
    }
    let slash = reference.rfind('/').unwrap_or(0);
    let Some(colon) = reference.rfind(':') else {
        return Ok(None);
    };
    if colon <= slash || colon + 1 >= reference.len() || colon == 0 {
        return Ok(None);
    }
    Ok(Some(reference[colon + 1..].to_owned()))
}

fn split_tag_reference(reference: &str) -> Result<(String, String)> {
    if reference.contains('@') {
        return Err(DomainError::InvalidConfig(
            "镜像目标引用必须使用 Tag，不能使用 digest".into(),
        ));
    }
    let slash = reference.rfind('/').unwrap_or(0);
    let Some(colon) = reference.rfind(':') else {
        return Err(DomainError::InvalidConfig(
            "镜像目标引用必须包含 Tag".into(),
        ));
    };
    if colon <= slash || colon == 0 || colon + 1 >= reference.len() {
        return Err(DomainError::InvalidConfig(
            "镜像目标引用的 Repository 或 Tag 为空".into(),
        ));
    }
    Ok((
        reference[..colon].to_owned(),
        reference[colon + 1..].to_owned(),
    ))
}

fn image_operation_error(message: &str) -> DomainError {
    DomainError::Container(ContainerError::new(
        ContainerErrorCategory::Protocol,
        "执行 Docker 镜像操作",
        message,
    ))
}

fn map_bollard_error(operation: &'static str, error: BollardError) -> DomainError {
    let (category, message, retryable) = match error {
        BollardError::DockerResponseServerError { status_code, .. } => {
            let (category, message) = match status_code {
                401 => (ContainerErrorCategory::Authentication, "Docker 认证失败"),
                403 => (ContainerErrorCategory::PermissionDenied, "Docker 权限不足"),
                404 => (ContainerErrorCategory::NotFound, "Docker 资源不存在"),
                408 | 504 => (ContainerErrorCategory::Timeout, "Docker 请求超时"),
                _ => (
                    ContainerErrorCategory::Protocol,
                    "Docker Engine 返回错误响应",
                ),
            };
            (
                category,
                message,
                matches!(status_code, 408 | 429 | 500..=599),
            )
        }
        BollardError::RequestTimeoutError => {
            (ContainerErrorCategory::Timeout, "Docker 请求超时", true)
        }
        BollardError::UnsupportedURISchemeError { .. } => (
            ContainerErrorCategory::Unsupported,
            "Docker 连接地址协议不受支持",
            false,
        ),
        BollardError::SocketNotFoundError(_) | BollardError::IOError { .. } => (
            ContainerErrorCategory::Network,
            "无法连接 Docker Engine",
            true,
        ),
        BollardError::JsonDataError { .. } | BollardError::JsonSerdeError { .. } => (
            ContainerErrorCategory::Protocol,
            "Docker Engine 返回的数据格式无效",
            false,
        ),
        _ => (
            ContainerErrorCategory::Network,
            "Docker Engine 请求失败",
            true,
        ),
    };
    debug!(operation, category = ?category, retryable, "docker engine request failed");
    DomainError::Container(ContainerError::new(category, operation, message).retryable(retryable))
}

fn json_value<T: Serialize + ?Sized>(value: &T, operation: &'static str) -> Result<Value> {
    let value = serde_json::to_value(value).map_err(|_| {
        DomainError::Container(ContainerError::new(
            ContainerErrorCategory::Protocol,
            operation,
            "Docker Engine 返回的数据无法解析",
        ))
    })?;
    ensure_response_size(&value, operation)?;
    Ok(value)
}

fn json_values<T: Serialize>(values: &[T], operation: &'static str) -> Result<Vec<Value>> {
    let value = json_value(values, operation)?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| protocol_error("Docker Engine 列表响应不是数组"))
}

fn ensure_response_size<T: Serialize + ?Sized>(value: &T, operation: &'static str) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|_| runtime_error(operation))?;
    if bytes.len() > MAX_DOCKER_RESPONSE_BYTES {
        return Err(DomainError::Container(ContainerError::new(
            ContainerErrorCategory::Protocol,
            operation,
            "Docker Engine 响应超过大小限制",
        )));
    }
    Ok(())
}

fn validate_resource_id(value: &str, field: &str) -> Result<String> {
    if value.trim().is_empty()
        || value.len() > MAX_CONTAINER_RESOURCE_ID_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(DomainError::InvalidConfig(format!(
            "{field}无效或超过长度限制"
        )));
    }
    Ok(value.to_owned())
}

fn paginate<T>(mut items: Vec<T>, query: &ContainerListQuery) -> ContainerPage<T> {
    let total = items.len();
    let start = query.page.saturating_sub(1).saturating_mul(query.page_size);
    let has_more = start.saturating_add(query.page_size) < total;
    if start >= total {
        items.clear();
    } else {
        items = items
            .into_iter()
            .skip(start)
            .take(query.page_size)
            .collect();
    }
    ContainerPage {
        items,
        page: query.page,
        page_size: query.page_size,
        total,
        has_more,
    }
}

fn get<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

fn string(value: &Value, keys: &[&str]) -> Option<String> {
    get(value, keys).and_then(Value::as_str).map(str::to_owned)
}

fn bool_value(value: &Value, keys: &[&str]) -> Option<bool> {
    get(value, keys).and_then(Value::as_bool)
}

fn number(value: &Value, keys: &[&str]) -> Option<usize> {
    get(value, keys)
        .and_then(Value::as_u64)
        .and_then(|number| usize::try_from(number).ok())
}

fn i64_number(value: &Value, keys: &[&str]) -> Option<i64> {
    get(value, keys).and_then(Value::as_i64)
}

fn u64_number(value: &Value, keys: &[&str]) -> Option<u64> {
    get(value, keys).and_then(Value::as_u64)
}

fn labels(value: &Value, keys: &[&str]) -> Vec<DockerLabel> {
    get(value, keys)
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|map| map.iter())
        .take(MAX_CONTAINER_LABELS)
        .map(|(key, value)| DockerLabel {
            key: key.clone(),
            value: value.as_str().unwrap_or_default().to_owned(),
        })
        .collect()
}

fn strings(value: &Value, keys: &[&str], limit: usize) -> Vec<String> {
    get(value, keys)
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|values| values.iter())
        .filter_map(Value::as_str)
        .take(limit)
        .map(str::to_owned)
        .collect()
}

fn connection_info(
    profile: &ContainerEndpointProfile,
    version: &Value,
    info: &Value,
) -> DockerConnectionInfo {
    DockerConnectionInfo {
        endpoint_id: profile.id.clone(),
        api_version: string(version, &["ApiVersion", "api_version"]),
        server_version: string(version, &["Version", "version"]),
        server_name: string(info, &["Name", "name"]),
        operating_system: string(version, &["Os", "os"]),
        architecture: string(version, &["Arch", "architecture"]),
        read_only: profile.read_only,
    }
}

fn engine_version(value: &Value) -> DockerEngineVersion {
    DockerEngineVersion {
        api_version: string(value, &["ApiVersion", "api_version"]),
        min_api_version: string(value, &["MinAPIVersion", "min_api_version"]),
        server_version: string(value, &["Version", "version"]),
        build_time: string(value, &["BuildTime", "build_time"]),
        git_commit: string(value, &["GitCommit", "git_commit"]),
        go_version: string(value, &["GoVersion", "go_version"]),
        os: string(value, &["Os", "os"]),
        architecture: string(value, &["Arch", "architecture"]),
        kernel_version: string(value, &["KernelVersion", "kernel_version"]),
        experimental: bool_value(value, &["Experimental", "experimental"]),
    }
}

fn count_container_state(values: &[bollard::models::ContainerSummary], state: &str) -> usize {
    values
        .iter()
        .filter(|value| {
            serde_json::to_value(value)
                .ok()
                .and_then(|value| string(&value, &["State", "state"]))
                .is_some_and(|value| value.eq_ignore_ascii_case(state))
        })
        .count()
}

fn matches_search<T: Serialize>(value: &T, search: Option<&str>) -> bool {
    let Some(search) = search else { return true };
    serde_json::to_value(value)
        .ok()
        .and_then(|value| serde_json::to_string(&value).ok())
        .is_some_and(|value| value.to_lowercase().contains(search))
}

fn container_summary(value: &Value) -> Result<DockerContainerSummary> {
    let id = string(value, &["Id", "id"]).ok_or_else(|| protocol_error("容器响应缺少 ID"))?;
    Ok(DockerContainerSummary {
        id,
        names: strings(value, &["Names", "names"], MAX_CONTAINER_NETWORKS),
        image: string(value, &["Image", "image"]),
        image_id: string(value, &["ImageID", "image_id"]),
        command: string(value, &["Command", "command"]),
        created: i64_number(value, &["Created", "created"]),
        state: string(value, &["State", "state"]),
        status: string(value, &["Status", "status"]),
        health: string(value, &["Health", "health"]),
        ports: ports(value),
        networks: strings(value, &["Networks", "networks"], MAX_CONTAINER_NETWORKS),
        labels: labels(value, &["Labels", "labels"]),
    })
}

fn ports(value: &Value) -> Vec<DockerContainerPort> {
    get(value, &["Ports", "ports"])
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|values| values.iter())
        .take(MAX_CONTAINER_PORTS)
        .map(|value| DockerContainerPort {
            ip: string(value, &["IP", "ip"]),
            private_port: get(value, &["PrivatePort", "private_port"])
                .and_then(Value::as_u64)
                .and_then(|v| u16::try_from(v).ok()),
            public_port: get(value, &["PublicPort", "public_port"])
                .and_then(Value::as_u64)
                .and_then(|v| u16::try_from(v).ok()),
            protocol: string(value, &["Type", "type"]),
        })
        .collect()
}

fn container_detail(value: &Value) -> Result<DockerContainerDetail> {
    let mut summary_value = value.clone();
    if let Some(config) = get(value, &["Config", "config"])
        && let Some(object) = summary_value.as_object_mut()
    {
        if let Some(image) = string(config, &["Image", "image"]) {
            object.insert("Image".into(), Value::String(image));
        }
        if let Some(labels) = get(config, &["Labels", "labels"]) {
            object.insert("Labels".into(), labels.clone());
        }
    }
    let summary = container_summary(&summary_value)?;
    let config = get(value, &["Config", "config"]);
    let host_config = get(value, &["HostConfig", "host_config"]);
    let network_settings = get(value, &["NetworkSettings", "network_settings"]);
    Ok(DockerContainerDetail {
        summary,
        path: string(value, &["Path", "path"]),
        args: strings(value, &["Args", "args"], MAX_CONTAINER_RESOURCE_ITEMS),
        platform: string(value, &["Platform", "platform"]),
        working_directory: config.and_then(|value| string(value, &["WorkingDir", "working_dir"])),
        entrypoint: config.map_or_else(Vec::new, |value| {
            strings(
                value,
                &["Entrypoint", "entrypoint"],
                MAX_CONTAINER_RESOURCE_ITEMS,
            )
        }),
        command: config.map_or_else(Vec::new, |value| {
            strings(value, &["Cmd", "cmd"], MAX_CONTAINER_RESOURCE_ITEMS)
        }),
        restart_policy: host_config
            .and_then(|value| get(value, &["RestartPolicy", "restart_policy"]))
            .and_then(|value| string(value, &["Name", "name"])),
        env_keys: config.map_or_else(Vec::new, |value| {
            strings(value, &["Env", "env"], MAX_CONTAINER_RESOURCE_ITEMS)
                .into_iter()
                .filter_map(|env| env.split_once('=').map(|(key, _)| key.to_owned()))
                .collect()
        }),
        mounts: mounts(value),
        networks: network_attachments(
            network_settings
                .and_then(|value| get(value, &["Networks", "networks"]))
                .unwrap_or(&Value::Null),
        ),
    })
}

fn mounts(value: &Value) -> Vec<DockerMountSummary> {
    get(value, &["Mounts", "mounts"])
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|values| values.iter())
        .take(MAX_CONTAINER_MOUNTS)
        .map(|value| DockerMountSummary {
            source: string(value, &["Source", "source"]),
            target: string(value, &["Destination", "destination"]),
            kind: string(value, &["Type", "type"]),
            read_only: bool_value(value, &["RW", "rw"]).map(|read_write| !read_write),
        })
        .collect()
}

fn network_attachments(value: &Value) -> Vec<DockerNetworkAttachment> {
    value
        .as_object()
        .into_iter()
        .flat_map(|map| map.iter())
        .take(MAX_CONTAINER_NETWORKS)
        .map(|(name, value)| DockerNetworkAttachment {
            name: string(value, &["Name", "name"]).or_else(|| Some(name.clone())),
            network_id: string(value, &["NetworkID", "network_id"]),
            endpoint_id: string(value, &["EndpointID", "endpoint_id"]),
            gateway: string(value, &["Gateway", "gateway"]),
            ip_address: string(value, &["IPAddress", "ip_address"]),
            mac_address: string(value, &["MacAddress", "mac_address"]),
        })
        .collect()
}

fn image_summary(value: &Value) -> Result<DockerImageSummary> {
    Ok(DockerImageSummary {
        id: string(value, &["Id", "id"]).ok_or_else(|| protocol_error("镜像响应缺少 ID"))?,
        repository_tags: strings(
            value,
            &["RepoTags", "repo_tags"],
            MAX_CONTAINER_REPOSITORY_REFERENCES,
        ),
        repository_digests: strings(
            value,
            &["RepoDigests", "repo_digests"],
            MAX_CONTAINER_REPOSITORY_REFERENCES,
        ),
        created: i64_number(value, &["Created", "created"]),
        size_bytes: i64_number(value, &["Size", "size"]),
        shared_size_bytes: i64_number(value, &["SharedSize", "shared_size"]),
        containers: i64_number(value, &["Containers", "containers"]),
        architecture: string(value, &["Architecture", "architecture"]),
        operating_system: string(value, &["Os", "os"]),
        labels: labels(value, &["Labels", "labels"]),
    })
}

fn image_detail(value: &Value) -> Result<DockerImageDetail> {
    let summary = image_summary(value)?;
    Ok(DockerImageDetail {
        summary,
        parent_id: string(value, &["Parent", "parent"]),
        comment: string(value, &["Comment", "comment"]),
        author: string(value, &["Author", "author"]),
        docker_version: string(value, &["DockerVersion", "docker_version"]),
        container_config_summary: get(value, &["ContainerConfig", "container_config"])
            .and_then(|value| serde_json::to_string(value).ok()),
    })
}

fn network_summary(value: &Value) -> Result<DockerNetworkSummary> {
    let containers = get(value, &["Containers", "containers"]).and_then(Value::as_object);
    Ok(DockerNetworkSummary {
        id: string(value, &["Id", "id"]).ok_or_else(|| protocol_error("网络响应缺少 ID"))?,
        name: string(value, &["Name", "name"]),
        driver: string(value, &["Driver", "driver"]),
        scope: string(value, &["Scope", "scope"]),
        internal: bool_value(value, &["Internal", "internal"]),
        attachable: bool_value(value, &["Attachable", "attachable"]),
        ingress: bool_value(value, &["Ingress", "ingress"]),
        labels: labels(value, &["Labels", "labels"]),
        container_count: containers.map_or(0, |containers| containers.len()),
        subnets: subnets(value),
    })
}

fn subnets(value: &Value) -> Vec<DockerNetworkSubnet> {
    get(value, &["IPAM", "Ipam", "ipam"])
        .and_then(|value| get(value, &["Config", "config"]))
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|values| values.iter())
        .take(MAX_CONTAINER_NETWORKS)
        .map(|value| DockerNetworkSubnet {
            subnet: string(value, &["Subnet", "subnet"]),
            gateway: string(value, &["Gateway", "gateway"]),
        })
        .collect()
}

fn network_detail(value: &Value) -> Result<DockerNetworkDetail> {
    let summary = network_summary(value)?;
    let options = get(value, &["Options", "options"])
        .and_then(Value::as_object)
        .map(|values| {
            values
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();
    let containers = get(value, &["Containers", "containers"])
        .map(network_attachments)
        .unwrap_or_default();
    Ok(DockerNetworkDetail {
        summary,
        options,
        containers,
    })
}

fn volume_summary(value: &Value) -> Result<DockerVolumeSummary> {
    Ok(DockerVolumeSummary {
        name: string(value, &["Name", "name"])
            .ok_or_else(|| protocol_error("数据卷响应缺少名称"))?,
        driver: string(value, &["Driver", "driver"]),
        mountpoint: string(value, &["Mountpoint", "mountpoint"]),
        scope: string(value, &["Scope", "scope"]),
        labels: labels(value, &["Labels", "labels"]),
        container_count: get(value, &["UsageData", "usage_data"])
            .and_then(|value| number(value, &["RefCount", "ref_count"]))
            .unwrap_or(0),
        usage_size_bytes: get(value, &["UsageData", "usage_data"])
            .and_then(|value| i64_number(value, &["Size", "size"])),
        usage_reference_count: get(value, &["UsageData", "usage_data"])
            .and_then(|value| i64_number(value, &["RefCount", "ref_count"])),
    })
}

fn volume_detail(value: &Value) -> Result<DockerVolumeDetail> {
    Ok(DockerVolumeDetail {
        summary: volume_summary(value)?,
        options: get(value, &["Options", "options"])
            .and_then(|value| value.as_object())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        status: get(value, &["Status", "status"])
            .and_then(|value| value.as_object())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn protocol_error(message: &str) -> DomainError {
    DomainError::Container(ContainerError::new(
        ContainerErrorCategory::Protocol,
        "解析 Docker 响应",
        message,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_plain_remote_tcp_and_accepts_local_socket_shape() {
        let tcp = ContainerEndpointProfile::new_docker("remote", "tcp://127.0.0.1:2375");
        assert!(
            matches!(DockerDriver::validate_profile(&tcp), Err(DomainError::Container(error)) if error.category == ContainerErrorCategory::Tls)
        );
        let local = ContainerEndpointProfile::new_docker("local", "unix:///var/run/docker.sock");
        assert!(DockerDriver::validate_profile(&local).is_ok());
    }

    #[test]
    fn paginates_and_filters_without_returning_unbounded_items() {
        let query = ContainerListQuery {
            page: 2,
            page_size: 2,
            search: Some("web".into()),
        };
        let page = paginate(vec!["web-1", "web-2", "web-3", "web-4", "web-5"], &query);
        assert_eq!(page.items, vec!["web-3", "web-4"]);
        assert!(page.has_more);
        assert_eq!(page.total, 5);
    }

    #[test]
    fn maps_server_permissions_without_returning_server_body() {
        let error = map_bollard_error(
            "读取 Docker 容器列表",
            BollardError::DockerResponseServerError {
                status_code: 403,
                message: "secret-body".into(),
            },
        );
        assert!(
            matches!(error, DomainError::Container(error) if error.category == ContainerErrorCategory::PermissionDenied && !error.safe_message.contains("secret-body"))
        );
    }

    #[test]
    fn splits_registry_port_from_image_tag() {
        assert_eq!(
            split_optional_tag("127.0.0.1:15000/library/app:stable").expect("tag"),
            Some("stable".into())
        );
        assert_eq!(
            split_tag_reference("127.0.0.1:15000/library/app:stable").expect("reference"),
            ("127.0.0.1:15000/library/app".into(), "stable".into())
        );
        assert!(split_tag_reference("library/app@sha256:0123").is_err());
    }

    #[test]
    fn blocks_image_operations_before_connecting_read_only_engine() {
        let profile = ContainerEndpointProfile::local_docker("本机 Docker");
        let request = ContainerImageOperationRequest::new(
            ContainerImageOperationKind::Pull,
            "library/alpine:3.20",
        );
        let result =
            smol::block_on(DockerDriver::new().execute_image_operation(&profile, &request, None));
        assert!(matches!(
            result,
            Err(DomainError::Forbidden(message)) if message == READ_ONLY_MESSAGE
        ));
    }

    #[test]
    #[ignore = "需要本机 Docker Engine，使用 cargo test -- --ignored 执行"]
    fn reads_local_engine_without_write_operations() {
        let profile = ContainerEndpointProfile::local_docker("本机 Docker");
        let driver = DockerDriver::new();
        let info =
            smol::block_on(driver.test_connection(&profile)).expect("本机 Docker 连接测试应成功");
        assert_eq!(info.endpoint_id, profile.id);
        assert!(info.server_version.is_some());

        let overview = smol::block_on(driver.overview(&profile)).expect("本机 Docker 概览应成功");
        assert_eq!(overview.connection.endpoint_id, profile.id);

        let query = ContainerListQuery {
            page: 1,
            page_size: 10,
            search: None,
        };
        let containers = smol::block_on(driver.list_containers(&profile, &query))
            .expect("本机 Docker 容器列表应成功");
        let images = smol::block_on(driver.list_images(&profile, &query))
            .expect("本机 Docker 镜像列表应成功");
        let networks = smol::block_on(driver.list_networks(&profile, &query))
            .expect("本机 Docker 网络列表应成功");
        let volumes = smol::block_on(driver.list_volumes(&profile, &query))
            .expect("本机 Docker 数据卷列表应成功");
        assert!(containers.total <= MAX_CONTAINER_RESOURCE_ITEMS);
        assert!(images.total <= MAX_CONTAINER_RESOURCE_ITEMS);
        assert!(networks.total <= MAX_CONTAINER_NETWORKS);
        assert!(volumes.total <= MAX_CONTAINER_RESOURCE_ITEMS);

        let page_query = ContainerListQuery {
            page: 1,
            page_size: 1,
            search: None,
        };
        let first_page = smol::block_on(driver.list_containers(&profile, &page_query))
            .expect("本机 Docker 容器第一页应成功");
        assert!(first_page.items.len() <= 1);
        assert_eq!(first_page.has_more, first_page.total > 1);
        if first_page.total > 1 {
            let second_page = smol::block_on(driver.list_containers(
                &profile,
                &ContainerListQuery {
                    page: 2,
                    ..page_query
                },
            ))
            .expect("本机 Docker 容器第二页应成功");
            assert!(second_page.items.len() <= 1);
            assert_eq!(second_page.page, 2);
        }

        let missing_container = smol::block_on(
            driver.get_container(&profile, "ramag-cmt-002-resource-that-does-not-exist"),
        );
        assert!(matches!(
            missing_container,
            Err(DomainError::Container(error))
                if error.category == ContainerErrorCategory::NotFound
        ));

        if let Some(container) = containers.items.first() {
            let detail = smol::block_on(driver.get_container(&profile, &container.id))
                .expect("本机 Docker 容器详情应成功");
            assert_eq!(detail.summary.id, container.id);
        }
        if let Some(image) = images.items.first() {
            let detail = smol::block_on(driver.get_image(&profile, &image.id))
                .expect("本机 Docker 镜像详情应成功");
            assert_eq!(detail.summary.id, image.id);
        }
        if let Some(network) = networks.items.first() {
            let detail = smol::block_on(driver.get_network(&profile, &network.id))
                .expect("本机 Docker 网络详情应成功");
            assert_eq!(detail.summary.id, network.id);
        }
        if let Some(volume) = volumes.items.first() {
            let detail = smol::block_on(driver.get_volume(&profile, &volume.name))
                .expect("本机 Docker 数据卷详情应成功");
            assert_eq!(detail.summary.name, volume.name);
        }
    }
}
