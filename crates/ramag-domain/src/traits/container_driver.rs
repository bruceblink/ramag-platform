//! Docker 与 Kubernetes 管理适配器的领域接口。

use std::sync::{Arc, atomic::AtomicBool};

use async_trait::async_trait;

use crate::entities::{
    ContainerEndpointProfile, ContainerImageOperationRequest, ContainerImageOperationResult,
    ContainerListQuery, ContainerLogQuery, ContainerPage, ContainerRegistryCredential,
    DockerConnectionInfo, DockerContainerDetail, DockerContainerLogLine, DockerContainerLogs,
    DockerContainerStats, DockerContainerSummary, DockerImageDetail, DockerImageSummary,
    DockerNetworkDetail, DockerNetworkSummary, DockerOverview, DockerVolumeDetail,
    DockerVolumeSummary,
};
use crate::error::Result;

/// 镜像操作的取消标记；基础设施层在请求和流式响应之间检查它。
pub type ContainerOperationCancellation = Arc<AtomicBool>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerLogSinkResult {
    Accepted,
    Backpressured,
    Closed,
}

/// Docker 持续日志的有界接收入口；返回 Backpressured 时，适配器必须保留当前行并稍后重试。
pub type ContainerLogSink =
    Arc<dyn Fn(DockerContainerLogLine) -> ContainerLogSinkResult + Send + Sync>;

/// 容器工具只通过该接口访问外部平台；具体 HTTP、socket 和 named pipe 代码留在基础设施层。
#[async_trait]
pub trait ContainerDriver: Send + Sync {
    async fn test_connection(
        &self,
        _profile: &ContainerEndpointProfile,
    ) -> Result<DockerConnectionInfo> {
        Err(crate::error::DomainError::NotImplemented(
            "container_test_connection".into(),
        ))
    }

    async fn overview(&self, _profile: &ContainerEndpointProfile) -> Result<DockerOverview> {
        Err(crate::error::DomainError::NotImplemented(
            "container_overview".into(),
        ))
    }

    async fn list_containers(
        &self,
        _profile: &ContainerEndpointProfile,
        _query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerContainerSummary>> {
        Err(crate::error::DomainError::NotImplemented(
            "container_list_containers".into(),
        ))
    }

    async fn get_container(
        &self,
        _profile: &ContainerEndpointProfile,
        _container_id: &str,
    ) -> Result<DockerContainerDetail> {
        Err(crate::error::DomainError::NotImplemented(
            "container_get_container".into(),
        ))
    }

    async fn container_stats(
        &self,
        _profile: &ContainerEndpointProfile,
        _container_id: &str,
        _cancellation: ContainerOperationCancellation,
    ) -> Result<DockerContainerStats> {
        Err(crate::error::DomainError::NotImplemented(
            "container_stats".into(),
        ))
    }

    async fn container_logs(
        &self,
        _profile: &ContainerEndpointProfile,
        _container_id: &str,
        _query: &ContainerLogQuery,
        _cancellation: ContainerOperationCancellation,
    ) -> Result<DockerContainerLogs> {
        Err(crate::error::DomainError::NotImplemented(
            "container_logs".into(),
        ))
    }

    async fn follow_container_logs(
        &self,
        _profile: &ContainerEndpointProfile,
        _container_id: &str,
        _query: &ContainerLogQuery,
        _sink: ContainerLogSink,
        _cancellation: ContainerOperationCancellation,
    ) -> Result<()> {
        Err(crate::error::DomainError::NotImplemented(
            "container_follow_logs".into(),
        ))
    }

    async fn list_images(
        &self,
        _profile: &ContainerEndpointProfile,
        _query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerImageSummary>> {
        Err(crate::error::DomainError::NotImplemented(
            "container_list_images".into(),
        ))
    }

    async fn get_image(
        &self,
        _profile: &ContainerEndpointProfile,
        _image_id: &str,
    ) -> Result<DockerImageDetail> {
        Err(crate::error::DomainError::NotImplemented(
            "container_get_image".into(),
        ))
    }

    async fn execute_image_operation(
        &self,
        _profile: &ContainerEndpointProfile,
        _request: &ContainerImageOperationRequest,
        _credential: Option<&ContainerRegistryCredential>,
        _cancellation: ContainerOperationCancellation,
    ) -> Result<ContainerImageOperationResult> {
        Err(crate::error::DomainError::NotImplemented(
            "container_execute_image_operation".into(),
        ))
    }

    async fn list_networks(
        &self,
        _profile: &ContainerEndpointProfile,
        _query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerNetworkSummary>> {
        Err(crate::error::DomainError::NotImplemented(
            "container_list_networks".into(),
        ))
    }

    async fn get_network(
        &self,
        _profile: &ContainerEndpointProfile,
        _network_id: &str,
    ) -> Result<DockerNetworkDetail> {
        Err(crate::error::DomainError::NotImplemented(
            "container_get_network".into(),
        ))
    }

    async fn list_volumes(
        &self,
        _profile: &ContainerEndpointProfile,
        _query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerVolumeSummary>> {
        Err(crate::error::DomainError::NotImplemented(
            "container_list_volumes".into(),
        ))
    }

    async fn get_volume(
        &self,
        _profile: &ContainerEndpointProfile,
        _volume_name: &str,
    ) -> Result<DockerVolumeDetail> {
        Err(crate::error::DomainError::NotImplemented(
            "container_get_volume".into(),
        ))
    }
}
