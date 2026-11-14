use super::*;
use ramag_domain::{PluginApiVersion, PluginCapability, PluginPackageRuntime};
use tempfile::tempdir;

fn entry(archive_sha256: String) -> PluginPackageCatalogEntry {
    PluginPackageCatalogEntry {
        id: PluginId::new("cache.example").expect("valid ID"),
        name: "缓存测试插件".into(),
        version: "1.0.0".into(),
        api_version: PluginApiVersion::new(1, 0),
        runtime: PluginPackageRuntime::Process,
        package_url: "https://plugins.example.test/cache.tar.gz".into(),
        archive_sha256,
        capabilities: vec![PluginCapability::new("ui.entry")],
    }
}

fn archive(root: &Path, bytes: &[u8]) -> (PathBuf, String) {
    let path = root.join("cache.example-1.0.0-1.tar.gz");
    fs::write(&path, bytes).expect("archive");
    (path, hex::encode(Sha256::digest(bytes)))
}

#[test]
fn records_and_restores_a_verified_archive() {
    let root = tempdir().expect("root");
    let (archive_path, digest) = archive(root.path(), b"verified archive");
    let entry = entry(digest);
    let cache = PluginPackageCache::new(root.path().to_path_buf());
    cache
        .record_download(&entry, &archive_path)
        .expect("record");

    let restored = cache
        .restore(std::slice::from_ref(&entry))
        .expect("restore");
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].key(), "cache.example-1.0.0");
    assert_eq!(restored[0].path, archive_path);
}

#[test]
fn modified_archive_is_not_restored() {
    let root = tempdir().expect("root");
    let (archive_path, digest) = archive(root.path(), b"verified archive");
    let entry = entry(digest);
    let cache = PluginPackageCache::new(root.path().to_path_buf());
    cache
        .record_download(&entry, &archive_path)
        .expect("record");
    fs::write(&archive_path, b"modified archive").expect("modify");

    assert!(
        cache
            .restore(std::slice::from_ref(&entry))
            .expect("restore")
            .is_empty()
    );
}

#[test]
fn unknown_catalog_entry_is_not_restored() {
    let root = tempdir().expect("root");
    let (archive_path, digest) = archive(root.path(), b"verified archive");
    let entry = entry(digest);
    let cache = PluginPackageCache::new(root.path().to_path_buf());
    cache
        .record_download(&entry, &archive_path)
        .expect("record");

    let other = PluginPackageCatalogEntry {
        id: PluginId::new("other.example").expect("valid ID"),
        ..entry
    };
    assert!(cache.restore(&[other]).expect("restore").is_empty());
}

#[test]
fn rejects_path_traversal_in_index() {
    let root = tempdir().expect("root");
    let cache = PluginPackageCache::new(root.path().to_path_buf());
    fs::create_dir_all(root.path()).expect("root directory");
    let index = CacheIndex {
        format_version: CURRENT_PLUGIN_PACKAGE_CACHE_FORMAT_VERSION,
        entries: vec![CacheIndexEntry {
            plugin_id: PluginId::new("cache.example").expect("valid ID"),
            version: "1.0.0".into(),
            archive_sha256: "a".repeat(64),
            file_name: "../outside.tar.gz".into(),
        }],
    };
    fs::write(
        &cache.index_path,
        serde_json::to_vec(&index).expect("index"),
    )
    .expect("write");
    assert!(cache.restore(&[]).expect("restore").is_empty());
    assert!(
        cache
            .read_index()
            .expect("repaired index")
            .entries
            .is_empty()
    );
}

#[test]
fn remove_deletes_archive_and_index_record() {
    let root = tempdir().expect("root");
    let (archive_path, digest) = archive(root.path(), b"verified archive");
    let entry = entry(digest);
    let cache = PluginPackageCache::new(root.path().to_path_buf());
    cache
        .record_download(&entry, &archive_path)
        .expect("record");
    cache
        .remove_download(&entry, &archive_path)
        .expect("remove");
    assert!(!archive_path.exists());
    assert!(cache.restore(&[entry]).expect("restore").is_empty());
}

#[test]
fn rejects_relative_cache_root() {
    let cache = PluginPackageCache::new(PathBuf::from("relative-downloads"));
    assert!(matches!(
        cache.restore(&[]),
        Err(PluginPackageCacheError::RootInvalid { .. })
    ));
}

#[test]
fn rejects_directory_as_archive() {
    let root = tempdir().expect("root");
    let archive_path = root.path().join("cache.example-1.0.0-1.tar.gz");
    fs::create_dir(&archive_path).expect("archive directory");
    let entry = entry("a".repeat(64));
    let cache = PluginPackageCache::new(root.path().to_path_buf());
    assert!(matches!(
        cache.record_download(&entry, &archive_path),
        Err(PluginPackageCacheError::ArchivePathInvalid { .. })
    ));
}

#[cfg(unix)]
#[test]
fn rejects_symlink_as_archive() {
    use std::os::unix::fs::symlink;

    let root = tempdir().expect("root");
    let target = root.path().join("target.tar.gz");
    let archive_path = root.path().join("cache.example-1.0.0-1.tar.gz");
    fs::write(&target, b"archive").expect("target");
    symlink(&target, &archive_path).expect("symlink");
    let entry = entry(hex::encode(Sha256::digest(b"archive")));
    let cache = PluginPackageCache::new(root.path().to_path_buf());
    assert!(matches!(
        cache.record_download(&entry, &archive_path),
        Err(PluginPackageCacheError::ArchivePathInvalid { .. })
    ));
}
