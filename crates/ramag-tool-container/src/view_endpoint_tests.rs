use ramag_domain::entities::{ContainerEndpointId, DockerConnectionInfo, DockerContainerLogs};

use super::ContainerView;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[test]
fn delayed_same_endpoint_observation_does_not_invalidate_refresh() {
    let mut view = ContainerView::without_service();
    let endpoint = view.docker_endpoint.clone();
    view.request_id = 7;
    view.loading = true;

    assert!(!view.apply_docker_endpoint_change(endpoint));
    assert_eq!(view.request_id, 7);
    assert!(view.loading);
}

#[test]
fn changed_endpoint_invalidates_request_and_clears_connection_state() {
    let mut view = ContainerView::without_service();
    view.request_id = 7;
    view.loading = true;
    view.connection = Some(DockerConnectionInfo {
        endpoint_id: ContainerEndpointId::default(),
        api_version: Some("1.55".into()),
        server_version: Some("29.7.2".into()),
        server_name: None,
        operating_system: Some("linux".into()),
        architecture: Some("amd64".into()),
        read_only: true,
    });
    view.logs = Some(DockerContainerLogs {
        container_id: "container-before-endpoint-change".into(),
        lines: Vec::new(),
        bytes: 0,
        dropped_lines: 0,
        truncated: false,
    });
    view.error = Some("stale failure".into());

    assert!(view.apply_docker_endpoint_change("  tcp://localhost:2375  ".into()));
    assert_eq!(view.docker_endpoint, "tcp://localhost:2375");
    assert_eq!(view.profile.address, "tcp://localhost:2375");
    assert_eq!(view.request_id, 8);
    assert!(!view.loading);
    assert!(view.connection.is_none());
    assert!(view.logs.is_none());
    assert!(view.error.is_none());
}

#[test]
fn endpoint_change_cancels_owned_reads_and_ends_every_obsolete_loading_state() {
    let mut view = ContainerView::without_service();
    let logs = Arc::new(AtomicBool::new(false));
    let follow = Arc::new(AtomicBool::new(false));
    let stats = Arc::new(AtomicBool::new(false));
    view.log_cancellation = Some(logs.clone());
    view.log_follow_cancellation = Some(follow.clone());
    view.stats_cancellation = Some(stats.clone());
    view.loading = true;
    view.detail_loading = true;
    view.registry_loading = true;
    view.logs_loading = true;
    view.logs_following = true;
    view.stats_loading = true;

    assert!(view.apply_docker_endpoint_change("unix:///tmp/ramag-context-test.sock".into()));
    for cancellation in [logs, follow, stats] {
        assert!(cancellation.load(Ordering::Relaxed));
    }
    assert!(!view.loading);
    assert!(!view.detail_loading);
    assert!(!view.registry_loading);
    assert!(!view.logs_loading);
    assert!(!view.logs_following);
    assert!(!view.stats_loading);
    assert!(view.log_cancellation.is_none());
    assert!(view.log_follow_cancellation.is_none());
    assert!(view.stats_cancellation.is_none());
}

#[test]
fn registry_configuration_observer_keeps_current_request_for_an_unchanged_endpoint() {
    let mut view = ContainerView::without_service();
    view.request_id = 17;
    view.registry_loading = true;
    let endpoint = view.registry_endpoint.clone();
    assert!(!view.apply_registry_configuration(format!(" {endpoint} "), false));
    assert_eq!(view.request_id, 17);
    assert!(view.registry_loading);

    assert!(view.apply_registry_configuration(endpoint, true));
    assert_eq!(view.request_id, 18);
    assert!(!view.registry_loading);
}
