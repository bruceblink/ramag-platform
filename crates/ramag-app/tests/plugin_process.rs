#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::fs;
use std::path::Path;

use ramag_app::{PluginProcessConfig, PluginProcessHost};
use ramag_domain::{
    MAX_PLUGIN_PACKAGE_SHA256_BYTES, PluginCapability, PluginId, PluginPackageManifest,
};
use sha2::{Digest, Sha256};
use tempfile::tempdir;

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
    let package = tempdir().unwrap();
    let entrypoint_path = package.path().join("helper");
    fs::copy(helper, &entrypoint_path).unwrap();
    let helper_bytes = fs::read(&entrypoint_path).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(b"helper");
    hasher.update([0]);
    hasher.update(helper_bytes);
    hasher.update([0]);
    let entrypoint = "helper";
    let mut descriptor = manifest(entrypoint);
    descriptor.package_sha256 = hex::encode(hasher.finalize());
    let host = PluginProcessHost::start(
        PluginProcessConfig::new(package.path(), descriptor)
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
