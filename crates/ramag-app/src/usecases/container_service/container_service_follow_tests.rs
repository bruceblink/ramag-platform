use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ramag_domain::entities::{
    ContainerEndpointProfile, ContainerLogQuery, DockerContainerLogLine, DockerLogStream,
};
use ramag_domain::error::Result;
use ramag_domain::traits::{
    ContainerDriver, ContainerLogSink, ContainerLogSinkResult, ContainerOperationCancellation,
};

use super::ContainerService;

struct FollowDriver;

#[async_trait]
impl ContainerDriver for FollowDriver {
    async fn follow_container_logs(
        &self,
        _profile: &ContainerEndpointProfile,
        _container_id: &str,
        query: &ContainerLogQuery,
        sink: ContainerLogSink,
        cancellation: ContainerOperationCancellation,
    ) -> Result<()> {
        assert_eq!(query.tail, 20);
        assert!(!cancellation.load(std::sync::atomic::Ordering::Relaxed));
        assert_eq!(
            sink(DockerContainerLogLine {
                stream: DockerLogStream::Stderr,
                message: "token=follow-secret".into(),
            }),
            ContainerLogSinkResult::Accepted
        );
        Ok(())
    }
}

#[test]
fn follow_container_logs_redacts_lines_before_delivering_them() {
    let service = ContainerService::new(Arc::new(FollowDriver));
    let profile = ContainerEndpointProfile::new_docker("test", "unix:///var/run/docker.sock");
    let query = ContainerLogQuery {
        tail: 20,
        ..Default::default()
    };
    let received = Arc::new(Mutex::new(Vec::new()));
    let received_for_sink = received.clone();
    let sink: ContainerLogSink = Arc::new(move |line| {
        received_for_sink
            .lock()
            .expect("日志行锁不应中毒")
            .push(line);
        ContainerLogSinkResult::Accepted
    });

    smol::block_on(service.follow_container_logs(
        &profile,
        "container-1",
        &query,
        sink,
        Arc::new(AtomicBool::new(false)),
    ))
    .expect("持续读取应成功");

    let received = received.lock().expect("日志行锁不应中毒");
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].message, "[日志行包含敏感信息，已隐藏]");
}
