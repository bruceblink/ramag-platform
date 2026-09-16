//! 容器管理应用服务：把结构化只读请求转交给容器基础设施适配器。

use std::sync::Arc;

use ramag_domain::entities::{
    ContainerEndpointProfile, ContainerListQuery, ContainerPage, DockerConnectionInfo,
    DockerContainerDetail, DockerContainerSummary, DockerImageDetail, DockerImageSummary,
    DockerNetworkDetail, DockerNetworkSummary, DockerOverview, DockerVolumeDetail,
    DockerVolumeSummary,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::ContainerDriver;

/// 容器管理只依赖领域接口；Docker 客户端类型不进入应用层和 UI。
pub struct ContainerService {
    driver: Arc<dyn ContainerDriver>,
}

impl ContainerService {
    pub fn new(driver: Arc<dyn ContainerDriver>) -> Self {
        Self { driver }
    }

    pub fn driver(&self) -> &Arc<dyn ContainerDriver> {
        &self.driver
    }

    fn ensure_docker(profile: &ContainerEndpointProfile) -> Result<()> {
        unsupported_kubernetes(profile)
    }

    pub async fn test_connection(
        &self,
        profile: &ContainerEndpointProfile,
    ) -> Result<DockerConnectionInfo> {
        Self::ensure_docker(profile)?;
        self.driver.test_connection(profile).await
    }

    pub async fn overview(&self, profile: &ContainerEndpointProfile) -> Result<DockerOverview> {
        Self::ensure_docker(profile)?;
        self.driver.overview(profile).await
    }

    pub async fn list_containers(
        &self,
        profile: &ContainerEndpointProfile,
        query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerContainerSummary>> {
        Self::ensure_docker(profile)?;
        self.driver.list_containers(profile, query).await
    }

    pub async fn get_container(
        &self,
        profile: &ContainerEndpointProfile,
        container_id: &str,
    ) -> Result<DockerContainerDetail> {
        Self::ensure_docker(profile)?;
        self.driver.get_container(profile, container_id).await
    }

    pub async fn list_images(
        &self,
        profile: &ContainerEndpointProfile,
        query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerImageSummary>> {
        Self::ensure_docker(profile)?;
        self.driver.list_images(profile, query).await
    }

    pub async fn get_image(
        &self,
        profile: &ContainerEndpointProfile,
        image_id: &str,
    ) -> Result<DockerImageDetail> {
        Self::ensure_docker(profile)?;
        self.driver.get_image(profile, image_id).await
    }

    pub async fn list_networks(
        &self,
        profile: &ContainerEndpointProfile,
        query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerNetworkSummary>> {
        Self::ensure_docker(profile)?;
        self.driver.list_networks(profile, query).await
    }

    pub async fn get_network(
        &self,
        profile: &ContainerEndpointProfile,
        network_id: &str,
    ) -> Result<DockerNetworkDetail> {
        Self::ensure_docker(profile)?;
        self.driver.get_network(profile, network_id).await
    }

    pub async fn list_volumes(
        &self,
        profile: &ContainerEndpointProfile,
        query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerVolumeSummary>> {
        Self::ensure_docker(profile)?;
        self.driver.list_volumes(profile, query).await
    }

    pub async fn get_volume(
        &self,
        profile: &ContainerEndpointProfile,
        volume_name: &str,
    ) -> Result<DockerVolumeDetail> {
        Self::ensure_docker(profile)?;
        self.driver.get_volume(profile, volume_name).await
    }
}

/// Kubernetes 只读适配器接入前，应用层明确返回未实现，而不是把请求误发给 Docker。
pub fn unsupported_kubernetes(profile: &ContainerEndpointProfile) -> Result<()> {
    if profile.platform.is_docker() {
        return Ok(());
    }
    Err(DomainError::NotImplemented(
        "Kubernetes 只读查询将在 CMT-007 接入".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use ramag_domain::entities::{ContainerPlatform, DockerEngineVersion, DockerResourceCounts};

    struct MockContainerDriver;

    #[async_trait]
    impl ContainerDriver for MockContainerDriver {
        async fn test_connection(
            &self,
            profile: &ContainerEndpointProfile,
        ) -> Result<DockerConnectionInfo> {
            Ok(DockerConnectionInfo {
                endpoint_id: profile.id.clone(),
                api_version: Some("1.55".into()),
                server_version: Some("29.7.2".into()),
                server_name: Some("test-engine".into()),
                operating_system: Some("linux".into()),
                architecture: Some("amd64".into()),
                read_only: profile.read_only,
            })
        }

        async fn overview(&self, profile: &ContainerEndpointProfile) -> Result<DockerOverview> {
            Ok(DockerOverview {
                connection: self.test_connection(profile).await?,
                version: DockerEngineVersion {
                    api_version: Some("1.55".into()),
                    min_api_version: Some("1.40".into()),
                    server_version: Some("29.7.2".into()),
                    build_time: None,
                    git_commit: None,
                    go_version: None,
                    os: Some("linux".into()),
                    architecture: Some("amd64".into()),
                    kernel_version: None,
                    experimental: Some(false),
                },
                counts: DockerResourceCounts {
                    containers: 1,
                    running_containers: 1,
                    paused_containers: 0,
                    stopped_containers: 0,
                    images: 1,
                    networks: Some(1),
                    volumes: Some(1),
                },
                cpu_count: Some(12),
                memory_bytes: Some(1024),
            })
        }
    }

    #[test]
    fn delegates_connection_and_overview_to_driver() {
        let service = ContainerService::new(Arc::new(MockContainerDriver));
        let profile = ContainerEndpointProfile::new_docker("test", "unix:///var/run/docker.sock");
        let overview = smol::block_on(service.overview(&profile)).expect("overview should pass");
        assert_eq!(overview.counts.containers, 1);
        let connection =
            smol::block_on(service.test_connection(&profile)).expect("connection test should pass");
        assert_eq!(connection.endpoint_id, profile.id);
    }

    #[test]
    fn rejects_kubernetes_until_its_driver_is_added() {
        let profile = ContainerEndpointProfile {
            id: Default::default(),
            name: "kubernetes".into(),
            platform: ContainerPlatform::Kubernetes,
            address: "https://kubernetes.example.test".into(),
            context: Some("dev".into()),
            namespace: None,
            read_only: true,
        };
        assert!(matches!(
            unsupported_kubernetes(&profile),
            Err(DomainError::NotImplemented(_))
        ));
    }
}
