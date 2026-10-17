use ramag_domain::entities::{ContainerEndpointId, DockerConnectionInfo, DockerContainerLogs};

use super::ContainerView;

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
