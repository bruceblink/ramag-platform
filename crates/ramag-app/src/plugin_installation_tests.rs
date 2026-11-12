use std::fs;

use ramag_domain::{
    MAX_PLUGIN_PACKAGE_SHA256_BYTES, PluginCapability, PluginId, PluginPackageManifest,
};
use tempfile::tempdir;

use super::*;

fn manifest(id: &str) -> PluginPackageManifest {
    PluginPackageManifest::new(
        PluginId::new(id).expect("valid plugin ID"),
        format!("{id} package"),
        "1.0.0",
        "bin/plugin",
        "b".repeat(MAX_PLUGIN_PACKAGE_SHA256_BYTES),
    )
    .with_capabilities([PluginCapability::new("ui.entry")])
}

#[test]
fn store_round_trips_installation_and_restores_state() {
    let directory = tempdir().expect("temporary directory");
    let store = PluginInstallationStore::new(directory.path());
    let installation = PluginInstallation::new(manifest("example.one"));
    store
        .upsert(installation.clone())
        .expect("installation saved");

    let enabled = store
        .set_state(
            &PluginId::new("example.one").unwrap(),
            PluginInstallationState::Enabled,
            None,
        )
        .expect("state saved");
    assert_eq!(enabled.state, PluginInstallationState::Enabled);
    assert_eq!(store.load().unwrap(), std::slice::from_ref(&enabled));

    let raw = fs::read_to_string(store.path()).expect("state file exists");
    assert!(raw.contains("\"enabled\""));
    assert!(raw.contains("example.one"));
}

#[test]
fn store_orders_records_and_rejects_duplicate_ids() {
    let directory = tempdir().expect("temporary directory");
    let store = PluginInstallationStore::new(directory.path());
    store
        .save(&[
            PluginInstallation::new(manifest("example.two")),
            PluginInstallation::new(manifest("example.one")),
        ])
        .expect("records saved");
    assert_eq!(
        store
            .load()
            .unwrap()
            .into_iter()
            .map(|item| item.manifest.id.as_str().to_owned())
            .collect::<Vec<_>>(),
        ["example.one", "example.two"]
    );

    let duplicate = [
        PluginInstallation::new(manifest("example.one")),
        PluginInstallation::new(manifest("example.one")),
    ];
    assert!(matches!(
        store.save(&duplicate),
        Err(PluginInstallationStoreError::DuplicatePlugin { .. })
    ));
}

#[test]
fn store_quarantines_with_bounded_error_and_removes_without_package_deletion() {
    let directory = tempdir().expect("temporary directory");
    let store = PluginInstallationStore::new(directory.path());
    store
        .upsert(PluginInstallation::new(manifest("example.one")))
        .expect("installation saved");
    let quarantined = store
        .set_state(
            &PluginId::new("example.one").unwrap(),
            PluginInstallationState::Quarantined,
            Some("进程启动失败".into()),
        )
        .expect("quarantine state saved");
    assert_eq!(quarantined.last_error.as_deref(), Some("进程启动失败"));
    assert!(
        store
            .remove(&PluginId::new("example.one").unwrap())
            .unwrap()
    );
    assert!(store.load().unwrap().is_empty());
    assert!(directory.path().exists());
}

#[test]
fn corrupt_state_file_is_rejected_without_fallback_execution() {
    let directory = tempdir().expect("temporary directory");
    let store = PluginInstallationStore::new(directory.path());
    fs::create_dir_all(directory.path()).expect("directory exists");
    fs::write(store.path(), b"not-json").expect("corrupt state written");

    assert!(matches!(
        store.load(),
        Err(PluginInstallationStoreError::CorruptFile(_))
    ));
}
