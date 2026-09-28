use std::sync::Arc;
use std::sync::atomic::Ordering;

use ramag_domain::entities::{ContainerEndpointProfile, ContainerLogQuery, DockerContainerLogLine};
use ramag_domain::error::{ContainerError, ContainerErrorCategory, DomainError, Result};
use ramag_domain::traits::{ContainerLogSink, ContainerOperationCancellation};

use super::{ContainerService, contains_sensitive_log_marker};

impl ContainerService {
    pub async fn follow_container_logs(
        &self,
        profile: &ContainerEndpointProfile,
        container_id: &str,
        query: &ContainerLogQuery,
        sink: ContainerLogSink,
        cancellation: ContainerOperationCancellation,
    ) -> Result<()> {
        Self::ensure_docker(profile)?;
        query.validate().map_err(DomainError::InvalidConfig)?;
        if cancellation.load(Ordering::Relaxed) {
            return Err(DomainError::Container(ContainerError::new(
                ContainerErrorCategory::Cancelled,
                "持续读取 Docker 容器日志",
                "Docker 容器日志读取已取消",
            )));
        }
        let redacting_sink: ContainerLogSink = Arc::new(move |mut line: DockerContainerLogLine| {
            if contains_sensitive_log_marker(&line.message) {
                line.message = "[日志行包含敏感信息，已隐藏]".into();
            }
            sink(line)
        });
        self.driver
            .follow_container_logs(profile, container_id, query, redacting_sink, cancellation)
            .await
    }
}
