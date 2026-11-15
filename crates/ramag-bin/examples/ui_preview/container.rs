//! 本机 Docker 只读预览。不会读取 Docker 配置文件或系统凭据。

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ramag_app::ContainerService;
use ramag_domain::entities::{
    ContainerEndpointProfile, ContainerListQuery, ContainerLogQuery, ContainerPage,
    DockerConnectionInfo, DockerContainerDetail, DockerContainerLogs, DockerContainerSummary,
    DockerImageDetail, DockerImageSummary, DockerNetworkDetail, DockerNetworkSummary,
    DockerOverview, DockerVolumeDetail, DockerVolumeSummary,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::{ContainerDriver, ContainerOperationCancellation};
use ramag_infra_container_docker::DockerDriver;

const DELAY: Duration = Duration::from_secs(3);
const NAMED_PIPE_PREFIX: &str = "npipe:////./pipe/ramag-container-preview-";
#[cfg(test)]
const TEST_CONTAINER_SEARCH: &str = "ramag-api-http-test";

pub(super) fn service(endpoint: &str) -> anyhow::Result<Arc<ContainerService>> {
    validate_local_endpoint(endpoint)?;
    let delay = explicit_delay()?;
    let driver = Arc::new(PreviewDockerDriver {
        inner: DockerDriver::new(),
        delay,
        endpoint: endpoint.to_owned(),
    });
    Ok(Arc::new(ContainerService::new(driver)))
}

/// 只接受本次验收专用的 named pipe；不回退到用户 Docker 配置。
pub(super) fn preview_endpoint() -> anyhow::Result<String> {
    let endpoint = std::env::var("RAMAG_CONTAINER_PREVIEW_ENDPOINT").map_err(|_| {
        anyhow::anyhow!("container-local 需要显式设置 RAMAG_CONTAINER_PREVIEW_ENDPOINT")
    })?;
    validate_local_endpoint(&endpoint)?;
    Ok(endpoint)
}

fn validate_local_endpoint(endpoint: &str) -> anyhow::Result<()> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || endpoint.contains(['\r', '\n', '\0']) {
        anyhow::bail!("容器预览端点无效");
    }
    if endpoint
        .strip_prefix(NAMED_PIPE_PREFIX)
        .is_some_and(|suffix| {
            !suffix.is_empty()
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Ok(());
    }
    anyhow::bail!("容器预览只允许本次预览专用 named pipe")
}

fn explicit_delay() -> anyhow::Result<Duration> {
    match std::env::var("RAMAG_CONTAINER_PREVIEW_DELAY") {
        Ok(value) if value == "3s" => Ok(DELAY),
        Ok(_) => anyhow::bail!("RAMAG_CONTAINER_PREVIEW_DELAY 只接受显式值 3s"),
        Err(std::env::VarError::NotPresent) => Ok(Duration::ZERO),
        Err(error) => Err(error.into()),
    }
}

struct PreviewDockerDriver {
    inner: DockerDriver,
    delay: Duration,
    endpoint: String,
}

impl PreviewDockerDriver {
    fn ensure_preview_endpoint(&self, profile: &ContainerEndpointProfile) -> Result<()> {
        if profile.platform.is_docker() && profile.address == self.endpoint {
            return Ok(());
        }
        Err(DomainError::InvalidConfig(
            "容器预览只允许访问启动时指定的本机端点".into(),
        ))
    }

    async fn delay(&self) -> Result<()> {
        if self.delay.is_zero() {
            return Ok(());
        }
        let delay = self.delay;
        ramag_app::run_blocking(move || {
            std::thread::sleep(delay);
            Ok(())
        })
        .await
    }
}

#[async_trait]
impl ContainerDriver for PreviewDockerDriver {
    async fn test_connection(
        &self,
        profile: &ContainerEndpointProfile,
    ) -> Result<DockerConnectionInfo> {
        self.ensure_preview_endpoint(profile)?;
        self.inner.test_connection(profile).await
    }

    async fn overview(&self, profile: &ContainerEndpointProfile) -> Result<DockerOverview> {
        self.ensure_preview_endpoint(profile)?;
        self.delay().await?;
        self.inner.overview(profile).await
    }

    async fn list_containers(
        &self,
        profile: &ContainerEndpointProfile,
        query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerContainerSummary>> {
        self.ensure_preview_endpoint(profile)?;
        self.delay().await?;
        self.inner.list_containers(profile, query).await
    }

    async fn get_container(
        &self,
        profile: &ContainerEndpointProfile,
        id: &str,
    ) -> Result<DockerContainerDetail> {
        self.ensure_preview_endpoint(profile)?;
        self.delay().await?;
        self.inner.get_container(profile, id).await
    }

    async fn container_logs(
        &self,
        profile: &ContainerEndpointProfile,
        id: &str,
        query: &ContainerLogQuery,
        cancellation: ContainerOperationCancellation,
    ) -> Result<DockerContainerLogs> {
        self.ensure_preview_endpoint(profile)?;
        self.delay().await?;
        self.inner
            .container_logs(profile, id, query, cancellation)
            .await
    }

    async fn list_images(
        &self,
        profile: &ContainerEndpointProfile,
        query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerImageSummary>> {
        self.ensure_preview_endpoint(profile)?;
        self.delay().await?;
        self.inner.list_images(profile, query).await
    }

    async fn get_image(
        &self,
        profile: &ContainerEndpointProfile,
        id: &str,
    ) -> Result<DockerImageDetail> {
        self.ensure_preview_endpoint(profile)?;
        self.delay().await?;
        self.inner.get_image(profile, id).await
    }

    async fn list_networks(
        &self,
        profile: &ContainerEndpointProfile,
        query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerNetworkSummary>> {
        self.ensure_preview_endpoint(profile)?;
        self.delay().await?;
        self.inner.list_networks(profile, query).await
    }

    async fn get_network(
        &self,
        profile: &ContainerEndpointProfile,
        id: &str,
    ) -> Result<DockerNetworkDetail> {
        self.ensure_preview_endpoint(profile)?;
        self.delay().await?;
        self.inner.get_network(profile, id).await
    }

    async fn list_volumes(
        &self,
        profile: &ContainerEndpointProfile,
        query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerVolumeSummary>> {
        self.ensure_preview_endpoint(profile)?;
        self.delay().await?;
        self.inner.list_volumes(profile, query).await
    }

    async fn get_volume(
        &self,
        profile: &ContainerEndpointProfile,
        name: &str,
    ) -> Result<DockerVolumeDetail> {
        self.ensure_preview_endpoint(profile)?;
        self.delay().await?;
        self.inner.get_volume(profile, name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_preview_owned_named_pipe() {
        for endpoint in ["npipe:////./pipe/ramag-container-preview-42"] {
            assert!(validate_local_endpoint(endpoint).is_ok(), "{endpoint}");
        }
        for endpoint in [
            "",
            "unix:///var/run/docker.sock",
            "tcp://127.0.0.1:2375",
            "https://127.0.0.1:2376",
            "https://docker.example.test",
            "npipe:////./pipe/docker_engine",
            "npipe:////./pipe/ramag-container-preview-42/path",
        ] {
            assert!(validate_local_endpoint(endpoint).is_err(), "{endpoint}");
        }
    }

    /// Read-only Docker smoke against the explicitly configured preview endpoint and fixture container.
    #[test]
    #[ignore = "requires an explicit local RAMAG_CONTAINER_PREVIEW_ENDPOINT and ramag-api-http-test"]
    fn real_docker_preview_reads_overview_fixture_detail_and_logs() {
        let endpoint = preview_endpoint().expect("explicit local Docker endpoint");
        let service = service(&endpoint).expect("read-only Docker service");
        let profile = ContainerEndpointProfile::new_docker("container-local preview", endpoint);
        let overview =
            futures::executor::block_on(service.overview(&profile)).expect("Docker overview");
        assert!(
            !overview
                .connection
                .server_version
                .as_deref()
                .unwrap_or_default()
                .is_empty()
        );

        let page = futures::executor::block_on(service.list_containers(
            &profile,
            &ContainerListQuery {
                search: Some(TEST_CONTAINER_SEARCH.into()),
                ..ContainerListQuery::default()
            },
        ))
        .expect("fixture container list");
        let container = page
            .items
            .iter()
            .find(|item| {
                item.names
                    .iter()
                    .any(|name| name.contains(TEST_CONTAINER_SEARCH))
            })
            .expect("local API test container");
        futures::executor::block_on(service.get_container(&profile, &container.id))
            .expect("fixture container detail");
        futures::executor::block_on(service.container_logs(
            &profile,
            &container.id,
            &ContainerLogQuery::default(),
        ))
        .expect("fixture container logs");
    }
}
