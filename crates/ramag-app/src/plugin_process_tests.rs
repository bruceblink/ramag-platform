use std::fs;

use ramag_domain::{
    MAX_PLUGIN_PACKAGE_SHA256_BYTES, PluginApiVersion, PluginCapability, PluginId,
    PluginPackageManifest,
};
use tempfile::tempdir;

use super::*;
use crate::PluginReady;

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

#[test]
fn resolver_accepts_regular_file_inside_package_root() {
    let directory = tempdir().unwrap();
    fs::create_dir_all(directory.path().join("bin")).unwrap();
    let entrypoint = directory.path().join("bin/plugin");
    fs::write(&entrypoint, b"helper").unwrap();

    let resolved = resolve_plugin_entrypoint(directory.path(), &manifest("bin/plugin")).unwrap();
    assert_eq!(resolved, entrypoint.canonicalize().unwrap());
}

#[test]
fn resolver_rejects_relative_parent_and_symlink_escape() {
    let directory = tempdir().unwrap();
    fs::write(directory.path().join("plugin"), b"helper").unwrap();
    assert!(matches!(
        resolve_plugin_entrypoint(directory.path(), &manifest("../plugin")),
        Err(PluginProcessError::InvalidEntrypoint { .. })
    ));

    #[cfg(unix)]
    {
        let outside = tempdir().unwrap();
        let outside_entry = outside.path().join("plugin");
        fs::write(&outside_entry, b"outside").unwrap();
        std::os::unix::fs::symlink(&outside_entry, directory.path().join("link")).unwrap();
        assert!(matches!(
            resolve_plugin_entrypoint(directory.path(), &manifest("link")),
            Err(PluginProcessError::SymlinkEntrypoint { .. })
        ));
    }
}

#[test]
fn process_package_digest_must_match_before_start() {
    let directory = tempdir().unwrap();
    fs::create_dir_all(directory.path().join("bin")).unwrap();
    fs::write(directory.path().join("bin/plugin"), b"original").unwrap();
    let mut descriptor = manifest("bin/plugin");
    descriptor.package_sha256 =
        crate::plugin_package_installer::package_digest(directory.path()).unwrap();
    assert!(verify_package_digest(directory.path(), &descriptor).is_ok());

    fs::write(directory.path().join("bin/plugin"), b"tampered").unwrap();
    assert!(matches!(
        verify_package_digest(directory.path(), &descriptor),
        Err(PluginProcessError::PackageDigestMismatch { .. })
    ));
}

#[test]
fn ready_message_requires_manifest_identity_api_and_capabilities() {
    let descriptor = manifest("plugin");
    let ready = PluginProcessMessage::Ready(PluginReady {
        protocol_version: CURRENT_PLUGIN_PROTOCOL_VERSION,
        plugin_id: descriptor.id.clone(),
        api_version: descriptor.api_version,
        capabilities: descriptor.capabilities.clone(),
    });
    assert!(validate_ready(&descriptor, ready).is_ok());

    let wrong_identity = PluginProcessMessage::Ready(PluginReady {
        protocol_version: CURRENT_PLUGIN_PROTOCOL_VERSION,
        plugin_id: PluginId::new("other.process").unwrap(),
        api_version: descriptor.api_version,
        capabilities: descriptor.capabilities.clone(),
    });
    assert!(matches!(
        validate_ready(&descriptor, wrong_identity),
        Err(PluginProcessError::HandshakeIdentity { .. })
    ));

    let wrong_api = PluginProcessMessage::Ready(PluginReady {
        protocol_version: CURRENT_PLUGIN_PROTOCOL_VERSION,
        plugin_id: descriptor.id.clone(),
        api_version: PluginApiVersion::new(2, 0),
        capabilities: descriptor.capabilities.clone(),
    });
    assert!(matches!(
        validate_ready(&descriptor, wrong_api),
        Err(PluginProcessError::HandshakeApi { .. })
    ));

    let extra_capability = PluginProcessMessage::Ready(PluginReady {
        protocol_version: CURRENT_PLUGIN_PROTOCOL_VERSION,
        plugin_id: descriptor.id,
        api_version: descriptor.api_version,
        capabilities: vec![PluginCapability::new("task.scoped")],
    });
    assert!(matches!(
        validate_ready(&manifest("plugin"), extra_capability),
        Err(PluginProcessError::HandshakeCapability { .. })
    ));
}

#[test]
fn granted_capabilities_reject_unknown_and_duplicate_values() {
    assert!(matches!(
        validate_granted_capabilities(&[PluginCapability::new("network.any")]),
        Err(PluginProcessError::UnknownCapability { .. })
    ));
    assert!(matches!(
        validate_granted_capabilities(&[
            PluginCapability::new("ui.entry"),
            PluginCapability::new("ui.entry"),
        ]),
        Err(PluginProcessError::DuplicateCapability { .. })
    ));
}

#[cfg(unix)]
#[test]
fn unix_process_group_termination_stops_descendant_processes() {
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    let mut child = Command::new("/bin/sh")
        .args(["-c", "sleep 30 & wait"])
        .process_group(0)
        .spawn()
        .expect("启动进程组测试子进程");
    let process_group = i32::try_from(child.id()).expect("读取测试进程组 ID");
    assert!(kill_process_group(process_group));
    let status = child.wait().expect("等待进程组测试子进程");
    assert!(!status.success());
}
