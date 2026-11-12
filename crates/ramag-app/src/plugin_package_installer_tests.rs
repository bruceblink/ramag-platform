use std::fs;
use std::sync::Arc;

use ramag_domain::{
    MAX_PLUGIN_PACKAGE_SHA256_BYTES, PluginCapability, PluginId, PluginPackageManifest,
};
use tempfile::tempdir;

use super::*;

fn manifest(digest: String) -> PluginPackageManifest {
    PluginPackageManifest::new(
        PluginId::new("example.package").unwrap(),
        "Example package",
        "1.0.0",
        "bin/plugin",
        digest,
    )
    .with_capabilities([PluginCapability::new("ui.entry")])
}

fn write_package(root: &std::path::Path, digest: String) {
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(root.join("bin/plugin"), b"plugin entry").unwrap();
    fs::write(
        root.join(PLUGIN_PACKAGE_MANIFEST_FILE_NAME),
        serde_json::to_vec_pretty(&manifest(digest)).unwrap(),
    )
    .unwrap();
}

#[test]
fn installer_verifies_digest_copies_package_and_updates_state() {
    let source = tempdir().unwrap();
    fs::create_dir_all(source.path().join("bin")).unwrap();
    fs::write(source.path().join("bin/plugin"), b"plugin entry").unwrap();
    let digest = package_digest(source.path()).unwrap();
    write_package(source.path(), digest);

    let packages = tempdir().unwrap();
    let state = tempdir().unwrap();
    let store = Arc::new(PluginInstallationStore::new(state.path().join("state")));
    let installer = PluginPackageInstaller::new(packages.path().join("packages"), store.clone());

    let installation = installer.install_from_directory(source.path()).unwrap();

    assert_eq!(installation.manifest.id.as_str(), "example.package");
    assert_eq!(store.load().unwrap(), [installation]);
    assert!(
        packages
            .path()
            .join("packages/example.package/1.0.0/bin/plugin")
            .is_file()
    );
}

#[test]
fn installer_rejects_digest_mismatch_without_writing_state() {
    let source = tempdir().unwrap();
    write_package(source.path(), "a".repeat(MAX_PLUGIN_PACKAGE_SHA256_BYTES));
    let packages = tempdir().unwrap();
    let state = tempdir().unwrap();
    let store = Arc::new(PluginInstallationStore::new(state.path().join("state")));
    let installer = PluginPackageInstaller::new(packages.path().join("packages"), store.clone());

    assert!(matches!(
        installer.install_from_directory(source.path()),
        Err(PluginPackageInstallError::DigestMismatch { .. })
    ));
    assert!(store.load().unwrap().is_empty());
}

#[test]
fn installer_rejects_source_inside_package_root() {
    let root = tempdir().unwrap();
    let source = root.path().join("packages/example.package");
    fs::create_dir_all(&source).unwrap();
    let store = Arc::new(PluginInstallationStore::new(root.path().join("packages")));
    let installer = PluginPackageInstaller::new(root.path().join("packages"), store);

    assert!(matches!(
        installer.install_from_directory(&source),
        Err(PluginPackageInstallError::SourceInsidePackages { .. })
    ));
}

#[cfg(unix)]
#[test]
fn installer_rejects_symlinked_package_entries() {
    let source = tempdir().unwrap();
    fs::create_dir_all(source.path().join("bin")).unwrap();
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("secret"), b"secret").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret"),
        source.path().join("bin/link"),
    )
    .unwrap();
    let digest = "a".repeat(MAX_PLUGIN_PACKAGE_SHA256_BYTES);
    write_package(source.path(), digest);
    let packages = tempdir().unwrap();
    let state = tempdir().unwrap();
    let store = Arc::new(PluginInstallationStore::new(state.path().join("state")));
    let installer = PluginPackageInstaller::new(packages.path().join("packages"), store);

    assert!(matches!(
        installer.install_from_directory(source.path()),
        Err(PluginPackageInstallError::SymlinkEntry { .. })
    ));
}

#[test]
fn uninstaller_removes_package_directory_and_state() {
    let source = tempdir().unwrap();
    fs::create_dir_all(source.path().join("bin")).unwrap();
    fs::write(source.path().join("bin/plugin"), b"plugin entry").unwrap();
    let digest = package_digest(source.path()).unwrap();
    write_package(source.path(), digest);

    let packages = tempdir().unwrap();
    let state = tempdir().unwrap();
    let store = Arc::new(PluginInstallationStore::new(state.path().join("state")));
    let installer = PluginPackageInstaller::new(packages.path().join("packages"), store.clone());
    installer.install_from_directory(source.path()).unwrap();

    assert!(
        installer
            .uninstall(&PluginId::new("example.package").unwrap())
            .unwrap()
    );
    assert!(store.load().unwrap().is_empty());
    assert!(
        !packages
            .path()
            .join("packages/example.package/1.0.0")
            .exists()
    );
    assert!(!packages.path().join("packages/example.package").exists());
}

#[test]
fn uninstaller_keeps_other_versions() {
    let source = tempdir().unwrap();
    fs::create_dir_all(source.path().join("bin")).unwrap();
    fs::write(source.path().join("bin/plugin"), b"plugin entry").unwrap();
    let digest = package_digest(source.path()).unwrap();
    write_package(source.path(), digest);

    let packages = tempdir().unwrap();
    let state = tempdir().unwrap();
    let store = Arc::new(PluginInstallationStore::new(state.path().join("state")));
    let installer = PluginPackageInstaller::new(packages.path().join("packages"), store);
    installer.install_from_directory(source.path()).unwrap();
    let other_version = packages
        .path()
        .join("packages/example.package/2.0.0/bin/plugin");
    fs::create_dir_all(other_version.parent().unwrap()).unwrap();
    fs::write(&other_version, b"other version").unwrap();

    assert!(
        installer
            .uninstall(&PluginId::new("example.package").unwrap())
            .unwrap()
    );
    assert!(other_version.is_file());
}

#[test]
fn uninstaller_returns_false_for_unknown_plugin() {
    let packages = tempdir().unwrap();
    let state = tempdir().unwrap();
    let store = Arc::new(PluginInstallationStore::new(state.path().join("state")));
    let installer = PluginPackageInstaller::new(packages.path().join("packages"), store);

    assert!(
        !installer
            .uninstall(&PluginId::new("missing.package").unwrap())
            .unwrap()
    );
}

#[test]
fn uninstaller_preserves_state_when_package_directory_is_missing() {
    let packages = tempdir().unwrap();
    let state = tempdir().unwrap();
    let store = Arc::new(PluginInstallationStore::new(state.path().join("state")));
    let installation =
        PluginInstallation::new(manifest("a".repeat(MAX_PLUGIN_PACKAGE_SHA256_BYTES)));
    store.upsert(installation.clone()).unwrap();
    let installer = PluginPackageInstaller::new(packages.path().join("packages"), store.clone());

    assert!(matches!(
        installer.uninstall(&installation.manifest.id),
        Err(PluginPackageInstallError::PackageMissing { .. })
    ));
    assert_eq!(store.load().unwrap(), [installation]);
}

#[cfg(unix)]
#[test]
fn uninstaller_rejects_symlinked_plugin_directory() {
    let packages = tempdir().unwrap();
    let state = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let plugin_dir = packages.path().join("packages/example.package");
    fs::create_dir_all(packages.path().join("packages")).unwrap();
    std::os::unix::fs::symlink(outside.path(), &plugin_dir).unwrap();
    let store = Arc::new(PluginInstallationStore::new(state.path().join("state")));
    let installation =
        PluginInstallation::new(manifest("a".repeat(MAX_PLUGIN_PACKAGE_SHA256_BYTES)));
    store.upsert(installation.clone()).unwrap();
    let installer = PluginPackageInstaller::new(packages.path().join("packages"), store);

    assert!(matches!(
        installer.uninstall(&installation.manifest.id),
        Err(PluginPackageInstallError::SymlinkEntry { .. })
    ));
}
