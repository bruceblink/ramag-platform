use super::*;
use async_compression::futures::write::GzipEncoder;
use async_tar::{Builder, Header};
use futures::io::{AsyncWriteExt, Cursor};
use ramag_domain::{
    PluginApiVersion, PluginCapability, PluginId, PluginPackageManifest, PluginPackageRuntime,
};
use sha2::{Digest, Sha256};
use tempfile::tempdir;

struct TestEntry {
    path: String,
    kind: EntryType,
    contents: Vec<u8>,
    link_name: Option<String>,
    declared_size: Option<u64>,
}

impl TestEntry {
    fn file(path: &str, contents: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.to_owned(),
            kind: EntryType::Regular,
            contents: contents.into(),
            link_name: None,
            declared_size: None,
        }
    }

    fn directory(path: &str) -> Self {
        Self {
            path: path.to_owned(),
            kind: EntryType::Directory,
            contents: Vec::new(),
            link_name: None,
            declared_size: Some(0),
        }
    }

    fn link(path: &str, kind: EntryType) -> Self {
        Self {
            path: path.to_owned(),
            kind,
            contents: Vec::new(),
            link_name: Some("target".into()),
            declared_size: Some(0),
        }
    }

    fn declared_size(mut self, size: u64) -> Self {
        self.declared_size = Some(size);
        self
    }
}

fn write_archive(path: &Path, entries: Vec<TestEntry>) {
    smol::block_on(async {
        let file = async_std::fs::File::create(path)
            .await
            .expect("archive file");
        let encoder = GzipEncoder::new(file);
        let mut builder = Builder::new(encoder);
        for entry in entries {
            let mut header = if entry.path.contains("..") || entry.path.starts_with('/') {
                let mut header = Header::new_old();
                let path = entry.path.as_bytes();
                header.as_old_mut().name[..path.len()].copy_from_slice(path);
                header
            } else {
                let mut header = Header::new_gnu();
                header.set_path(&entry.path).expect("archive path");
                header
            };
            header.set_entry_type(entry.kind);
            if let Some(link_name) = entry.link_name.as_deref() {
                header.set_link_name(link_name).expect("link name");
            }
            header.set_size(entry.declared_size.unwrap_or(entry.contents.len() as u64));
            header.set_cksum();
            builder
                .append(&header, Cursor::new(entry.contents))
                .await
                .expect("append archive entry");
        }
        let mut encoder = builder.into_inner().await.expect("finish tar");
        encoder.close().await.expect("finish gzip");
    });
}

fn package_entries() -> Vec<TestEntry> {
    let plugin = b"plugin";
    let source = tempdir().expect("temporary package source");
    fs::create_dir(source.path().join("bin")).expect("package bin directory");
    fs::write(source.path().join("bin/plugin"), plugin).expect("package entry");
    let digest =
        crate::plugin_package_installer::package_digest(source.path()).expect("package digest");
    let manifest = PluginPackageManifest::new(
        PluginId::new("archive.package").expect("valid plugin ID"),
        "归档测试插件",
        "1.0.0",
        "bin/plugin",
        digest,
    )
    .with_capabilities([PluginCapability::new("ui.entry")]);
    vec![
        TestEntry::directory("bin"),
        TestEntry::file(
            "plugin.json",
            serde_json::to_vec_pretty(&manifest).expect("manifest JSON"),
        ),
        TestEntry::file("bin/plugin", plugin),
    ]
}

fn installer(root: &Path) -> PluginPackageInstaller {
    PluginPackageInstaller::new(
        root.join("packages"),
        std::sync::Arc::new(crate::PluginInstallationStore::new(root.join("state"))),
    )
}

fn catalog_entry(archive_sha256: String) -> PluginPackageCatalogEntry {
    PluginPackageCatalogEntry {
        id: PluginId::new("archive.package").expect("valid plugin ID"),
        name: "归档测试插件".into(),
        version: "1.0.0".into(),
        api_version: PluginApiVersion::new(1, 0),
        runtime: PluginPackageRuntime::Process,
        package_url: "https://plugins.example.test/archive.tar.gz".into(),
        archive_sha256,
        capabilities: vec![PluginCapability::new("ui.entry")],
    }
}

fn assert_no_archive_temp(root: &Path) {
    assert!(
        std::fs::read_dir(root)
            .expect("temporary directory parent")
            .filter_map(Result::ok)
            .all(|entry| !entry
                .file_name()
                .to_string_lossy()
                .starts_with(".ramag-plugin-archive-"))
    );
}

#[test]
fn installs_valid_tar_gz_and_reuses_directory_installation_checks() {
    let root = tempdir().expect("tempdir");
    let archive = root.path().join("plugin.tar.gz");
    write_archive(&archive, package_entries());
    let installer = installer(root.path());

    let installation = installer
        .install_from_archive(&archive)
        .expect("valid archive installs");
    assert_eq!(installation.manifest.id.as_str(), "archive.package");
    assert_eq!(
        std::fs::read(
            root.path()
                .join("packages/archive.package/1.0.0/bin/plugin")
        )
        .expect("installed entry"),
        b"plugin"
    );
    let state =
        std::fs::read(root.path().join("state/installed.json")).expect("installation state file");
    let state =
        serde_json::from_slice::<Vec<PluginInstallation>>(&state).expect("installation state JSON");
    assert_eq!(state.len(), 1);
    assert_no_archive_temp(root.path());
}

#[test]
fn catalog_archive_rechecks_archive_digest_and_manifest_identity_before_install() {
    let archive_root = tempdir().expect("archive root");
    let archive = archive_root.path().join("plugin.tar.gz");
    write_archive(&archive, package_entries());
    let bytes = std::fs::read(&archive).expect("archive bytes");
    let archive_sha256 = hex::encode(Sha256::digest(&bytes));

    let digest_root = tempdir().expect("digest root");
    let digest_installer = installer(digest_root.path());
    let mut wrong_digest = catalog_entry(archive_sha256.clone());
    wrong_digest.archive_sha256 = "b".repeat(64);
    assert!(matches!(
        digest_installer.install_catalog_archive(&wrong_digest, &archive),
        Err(PluginPackageArchiveError::ArchiveDigestMismatch { .. })
    ));
    assert!(!digest_root.path().join("packages").exists());

    let identity_root = tempdir().expect("identity root");
    let identity_installer = installer(identity_root.path());
    let mut wrong_identity = catalog_entry(archive_sha256);
    wrong_identity.name = "另一个插件".into();
    assert!(matches!(
        identity_installer.install_catalog_archive(&wrong_identity, &archive),
        Err(PluginPackageArchiveError::Installer(
            PluginPackageInstallError::CatalogManifestMismatch {
                field: "插件名称",
                ..
            }
        ))
    ));
    assert!(!identity_root.path().join("state/installed.json").exists());
    assert_no_archive_temp(identity_root.path());
}

#[test]
fn archive_installer_cleans_temp_directory_when_directory_installation_fails() {
    let root = tempdir().expect("tempdir");
    let mut entries = package_entries();
    let invalid_manifest = PluginPackageManifest::new(
        PluginId::new("archive.package").expect("valid plugin ID"),
        "归档测试插件",
        "1.0.0",
        "bin/plugin",
        "a".repeat(ramag_domain::MAX_PLUGIN_PACKAGE_SHA256_BYTES),
    )
    .with_capabilities([PluginCapability::new("ui.entry")]);
    entries[1].contents = serde_json::to_vec_pretty(&invalid_manifest).unwrap();
    let archive = root.path().join("invalid.tar.gz");
    write_archive(&archive, entries);

    assert!(matches!(
        installer(root.path()).install_from_archive(&archive),
        Err(PluginPackageArchiveError::Installer(
            PluginPackageInstallError::DigestMismatch { .. }
        ))
    ));
    assert_no_archive_temp(root.path());
}

#[test]
fn rejects_traversal_links_special_entries_and_duplicate_paths() {
    for (name, entry) in [
        (
            "traversal",
            TestEntry::file("../escape", b"blocked".to_vec()),
        ),
        ("symlink", TestEntry::link("link", EntryType::Symlink)),
        ("hardlink", TestEntry::link("hard", EntryType::Link)),
        ("fifo", TestEntry::link("fifo", EntryType::Fifo)),
    ] {
        let root = tempdir().expect("tempdir");
        let archive = root.path().join(format!("{name}.tar.gz"));
        write_archive(&archive, vec![entry]);
        let error = installer(root.path())
            .install_from_archive(&archive)
            .expect_err("unsafe archive must be rejected");
        assert!(matches!(
            error,
            PluginPackageArchiveError::InvalidEntryPath { .. }
                | PluginPackageArchiveError::UnsupportedEntry { .. }
        ));
    }

    let root = tempdir().expect("tempdir");
    let archive = root.path().join("duplicate.tar.gz");
    write_archive(
        &archive,
        vec![
            TestEntry::file("same", b"one"),
            TestEntry::file("same", b"two"),
        ],
    );
    assert!(matches!(
        installer(root.path()).install_from_archive(&archive),
        Err(PluginPackageArchiveError::DuplicateEntry { .. })
    ));
}

#[test]
fn rejects_entry_count_and_unpacked_size_limits() {
    let root = tempdir().expect("tempdir");
    let archive = root.path().join("too-many.tar.gz");
    let entries = (0..=MAX_PLUGIN_ARCHIVE_ENTRIES)
        .map(|index| TestEntry::file(&format!("file-{index}"), b"x"))
        .collect();
    write_archive(&archive, entries);
    assert!(matches!(
        installer(root.path()).install_from_archive(&archive),
        Err(PluginPackageArchiveError::TooManyEntries { .. })
    ));

    let archive = root.path().join("too-large.tar.gz");
    write_archive(
        &archive,
        vec![
            TestEntry::file("large", Vec::new())
                .declared_size(MAX_PLUGIN_ARCHIVE_UNPACKED_BYTES + 1),
        ],
    );
    assert!(matches!(
        installer(root.path()).install_from_archive(&archive),
        Err(PluginPackageArchiveError::UnpackedTooLarge { .. })
    ));
}

#[test]
fn rejects_relative_and_symlink_archive_paths() {
    let root = tempdir().expect("tempdir");
    let installer = installer(root.path());
    assert!(matches!(
        installer.install_from_archive(Path::new("plugin.tar.gz")),
        Err(PluginPackageArchiveError::ArchivePathNotAbsolute { .. })
    ));

    #[cfg(unix)]
    {
        let target = root.path().join("target.tar.gz");
        write_archive(&target, package_entries());
        let link = root.path().join("link.tar.gz");
        std::os::unix::fs::symlink(&target, &link).expect("archive symlink");
        assert!(matches!(
            installer.install_from_archive(&link),
            Err(PluginPackageArchiveError::ArchivePathNotFile { .. })
        ));
    }
}
