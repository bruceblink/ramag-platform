#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::fs;
use std::path::Path;
#[cfg(unix)]
use std::process::Command;

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

/// 去除测试辅助进程的调试段，避免 Debug 构建体积掩盖插件包上限验收。
#[cfg(unix)]
fn strip_fixture_debug_symbols(path: &Path) {
    let status = Command::new("strip")
        .arg("-S")
        .arg(path)
        .status()
        .expect("strip test helper binary");
    assert!(status.success(), "strip test helper binary failed");
}

#[tokio::test]
async fn host_starts_real_helper_process_invokes_and_closes() {
    let helper = Path::new(env!("CARGO_BIN_EXE_ramag-plugin-helper"));
    let package = tempdir().unwrap();
    let entrypoint = if cfg!(windows) {
        "helper.exe"
    } else {
        "helper"
    };
    let entrypoint_path = package.path().join(entrypoint);
    fs::copy(helper, &entrypoint_path).unwrap();
    #[cfg(unix)]
    strip_fixture_debug_symbols(&entrypoint_path);
    let helper_bytes = fs::read(&entrypoint_path).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(entrypoint.as_bytes());
    hasher.update([0]);
    hasher.update(helper_bytes);
    hasher.update([0]);
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
