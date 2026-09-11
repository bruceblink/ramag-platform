use super::{
    AUTO_CHECK_INTERVAL, GitHubUpdateDriver, INITIAL_UPDATE_CHECK_DELAY, JumpServerHttpDriver,
    MainWindowOpenGate, build_plugin_host, build_tool_registry, install_tls_crypto_provider,
};

#[test]
fn automatic_update_checks_start_quickly_and_repeat_daily() {
    assert_eq!(
        INITIAL_UPDATE_CHECK_DELAY,
        std::time::Duration::from_secs(3)
    );
    assert_eq!(
        AUTO_CHECK_INTERVAL,
        std::time::Duration::from_secs(24 * 60 * 60)
    );
}

#[test]
fn tls_provider_is_ready_before_http_clients_are_built() {
    assert!(install_tls_crypto_provider().is_ok());
    assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    assert!(JumpServerHttpDriver::new().is_ok());
    assert!(ramag_infra_object_storage::ObjectStorageInfra::new().is_ok());
    assert!(GitHubUpdateDriver::new(env!("CARGO_PKG_VERSION")).is_ok());
}

#[test]
fn main_window_open_gate_coalesces_repeated_requests() {
    let mut gate = MainWindowOpenGate::default();

    assert!(gate.try_begin());
    assert!(!gate.try_begin());
    gate.finish();
    assert!(gate.try_begin());
}

#[test]
fn built_in_plugins_are_ready_before_the_main_window_uses_the_registry() {
    let host = build_plugin_host();

    assert!(!host.states().is_empty());
    assert!(
        host.states()
            .iter()
            .all(|(_, state)| *state == ramag_app::PluginState::Ready)
    );
    assert_eq!(host.registry().count(), host.states().len());
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[test]
fn clipboard_capture_retry_backoff_is_bounded() {
    use super::{CAPTURE_INTERVAL, CAPTURE_MAX_RETRY_INTERVAL, next_capture_retry_interval};

    let mut interval = CAPTURE_INTERVAL;
    assert_eq!(
        next_capture_retry_interval(interval),
        CAPTURE_INTERVAL.saturating_mul(2)
    );
    for _ in 0..16 {
        interval = next_capture_retry_interval(interval);
    }
    assert_eq!(interval, CAPTURE_MAX_RETRY_INTERVAL);
    assert_eq!(
        next_capture_retry_interval(interval),
        CAPTURE_MAX_RETRY_INTERVAL
    );
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[test]
fn clipboard_tool_is_registered_last() {
    let ids = build_tool_registry()
        .list()
        .into_iter()
        .map(|tool| tool.meta().id.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        [
            "dbclient",
            "kafka",
            "mqtt",
            "vcs",
            "ssh",
            "object_storage",
            "system",
            "clipboard"
        ]
    );
}

#[cfg(target_os = "linux")]
#[test]
fn clipboard_tool_is_not_registered_on_linux() {
    let ids = build_tool_registry()
        .list()
        .into_iter()
        .map(|tool| tool.meta().id.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        [
            "dbclient",
            "kafka",
            "mqtt",
            "vcs",
            "ssh",
            "object_storage",
            "system"
        ]
    );
}
