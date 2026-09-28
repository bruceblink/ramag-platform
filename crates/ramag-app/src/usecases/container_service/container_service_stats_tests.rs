use std::sync::{Arc, atomic::AtomicBool};

use async_trait::async_trait;
use ramag_domain::entities::{ContainerEndpointProfile, DockerContainerStats};
use ramag_domain::error::{ContainerErrorCategory, DomainError, Result};
use ramag_domain::traits::{ContainerDriver, ContainerOperationCancellation};

use super::ContainerService;

struct StatsDriver;

#[async_trait]
impl ContainerDriver for StatsDriver {
    async fn container_stats(
        &self,
        _profile: &ContainerEndpointProfile,
        container_id: &str,
        _cancellation: ContainerOperationCancellation,
    ) -> Result<DockerContainerStats> {
        Ok(DockerContainerStats {
            container_id: container_id.into(),
            name: Some("/test".into()),
            read_at: Some("2026-09-28T00:00:00Z".into()),
            cpu_percent: Some(12.5),
            memory_usage_bytes: Some(512),
            memory_limit_bytes: Some(1024),
            memory_percent: Some(50.0),
            network_rx_bytes: Some(10),
            network_tx_bytes: Some(20),
        })
    }
}

#[test]
fn delegates_container_stats_and_rejects_cancelled_request() {
    let service = ContainerService::new(Arc::new(StatsDriver));
    let profile = ContainerEndpointProfile::new_docker("test", "unix:///var/run/docker.sock");
    let stats = smol::block_on(service.container_stats(
        &profile,
        "container-1",
        Arc::new(AtomicBool::new(false)),
    ))
    .expect("容器指标请求应成功");
    assert_eq!(stats.container_id, "container-1");
    assert_eq!(stats.memory_percent, Some(50.0));

    let cancelled = smol::block_on(service.container_stats(
        &profile,
        "container-1",
        Arc::new(AtomicBool::new(true)),
    ));
    assert!(matches!(
        cancelled,
        Err(DomainError::Container(error))
            if error.category == ContainerErrorCategory::Cancelled
    ));
}
