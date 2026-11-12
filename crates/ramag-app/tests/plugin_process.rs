#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;

use ramag_app::{PluginProcessConfig, PluginProcessHost};
use ramag_domain::{
    MAX_PLUGIN_PACKAGE_SHA256_BYTES, PluginCapability, PluginId, PluginPackageManifest,
};

fn manifest(entrypoint: &str) -> PluginPackageManifest {
    PluginPackageManifest::new(
        PluginId::new("example.process").unwrap(),
        "Example process",
        "1.0.0",
        entrypoint,
        "a".repeat(MAX_PLUGIN_PACKAGE_SHA256_BYTES),
    )
    .with_capabilities([PluginCapability::new("ui.entry")])
}

#[tokio::test]
async fn host_starts_real_helper_process_invokes_and_closes() {
    let helper_path = std::env::var("CARGO_BIN_EXE_ramag_plugin_helper").map_or_else(
        |_| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .and_then(Path::parent)
                .unwrap()
                .join("ramag-plugin-helper")
        },
        Into::into,
    );
    let helper = Path::new(&helper_path);
    let package_root = helper.parent().unwrap();
    let entrypoint = helper.file_name().unwrap().to_string_lossy();
    let host = PluginProcessHost::start(
        PluginProcessConfig::new(package_root, manifest(&entrypoint))
            .with_granted_capabilities([PluginCapability::new("ui.entry")]),
    )
    .await
    .unwrap();

    let result = host
        .invoke("echo", b"hello process".to_vec())
        .await
        .unwrap();
    assert_eq!(result, b"hello process");
    assert!(host.stderr_tail().is_empty());
    host.shutdown().await.unwrap();
}
