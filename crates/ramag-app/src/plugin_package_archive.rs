//! 受限本地 `tar.gz` 插件包解包；只解包到临时目录，再交给目录安装器校验。

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use async_compression::futures::bufread::GzipDecoder;
use async_tar::{Archive, EntryType};
use futures::StreamExt as _;
use futures::io::{BufReader, Cursor};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    MAX_PLUGIN_PACKAGE_BYTES, PluginInstallation, PluginPackageCatalogEntry,
    PluginPackageInstallError, PluginPackageInstaller,
};

/// 单个本地插件归档允许读取的压缩后字节数。
pub const MAX_PLUGIN_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
/// 单个本地插件归档允许解包的条目数量。
pub const MAX_PLUGIN_ARCHIVE_ENTRIES: usize = 1024;
/// 单个本地插件归档允许解包的总字节数。
pub const MAX_PLUGIN_ARCHIVE_UNPACKED_BYTES: u64 = MAX_PLUGIN_PACKAGE_BYTES;
const MAX_PLUGIN_ARCHIVE_PATH_BYTES: usize = 4096;
static ARCHIVE_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveEntryKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArchiveEntryPlan {
    path: PathBuf,
    kind: ArchiveEntryKind,
    size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArchivePlan {
    entries: Vec<ArchiveEntryPlan>,
}

/// 读取、扫描并安装本地 `tar.gz` 插件包。
pub(crate) fn install_from_archive(
    installer: &PluginPackageInstaller,
    archive_path: &Path,
) -> Result<PluginInstallation, PluginPackageArchiveError> {
    let bytes = read_archive_snapshot(archive_path)?;
    install_archive_bytes(installer, &bytes, None)
}

/// 重新核对归档摘要并在安装前核对包内清单身份。
pub(crate) fn install_from_catalog_archive(
    installer: &PluginPackageInstaller,
    catalog_entry: &PluginPackageCatalogEntry,
    archive_path: &Path,
) -> Result<PluginInstallation, PluginPackageArchiveError> {
    let bytes = read_archive_snapshot(archive_path)?;
    let actual = hex::encode(Sha256::digest(&bytes));
    if actual != catalog_entry.archive_sha256 {
        return Err(PluginPackageArchiveError::ArchiveDigestMismatch {
            expected: catalog_entry.archive_sha256.clone(),
            actual,
        });
    }
    install_archive_bytes(installer, &bytes, Some(catalog_entry))
}

fn install_archive_bytes(
    installer: &PluginPackageInstaller,
    bytes: &[u8],
    catalog_entry: Option<&PluginPackageCatalogEntry>,
) -> Result<PluginInstallation, PluginPackageArchiveError> {
    let plan = smol::block_on(scan_archive(bytes))?;
    let packages_root = installer.packages_root().to_path_buf();
    let packages_root = ensure_packages_root_for_archive(&packages_root)
        .map_err(PluginPackageArchiveError::Installer)?;
    let temp = create_archive_temp_directory(&packages_root)?;
    let _cleanup = TemporaryArchiveDirectory::new(temp.clone());
    smol::block_on(extract_archive(bytes, &temp, &plan))?;
    let installation = match catalog_entry {
        Some(catalog_entry) => installer.install_catalog_directory(&temp, catalog_entry),
        None => installer.install_from_directory(&temp),
    };
    installation.map_err(PluginPackageArchiveError::Installer)
}

fn read_archive_snapshot(path: &Path) -> Result<Vec<u8>, PluginPackageArchiveError> {
    if !path.is_absolute() {
        return Err(PluginPackageArchiveError::ArchivePathNotAbsolute {
            path: path.to_path_buf(),
        });
    }
    let metadata = fs::symlink_metadata(path).map_err(|source| PluginPackageArchiveError::Io {
        operation: "检查插件归档",
        source,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(PluginPackageArchiveError::ArchivePathNotFile {
            path: path.to_path_buf(),
        });
    }
    if metadata.len() > MAX_PLUGIN_ARCHIVE_BYTES {
        return Err(PluginPackageArchiveError::ArchiveTooLarge {
            bytes: metadata.len(),
            max: MAX_PLUGIN_ARCHIVE_BYTES,
        });
    }
    let file = File::open(path).map_err(|source| PluginPackageArchiveError::Io {
        operation: "打开插件归档",
        source,
    })?;
    let mut bytes = Vec::with_capacity(metadata.len().min(64 * 1024) as usize);
    file.take(MAX_PLUGIN_ARCHIVE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| PluginPackageArchiveError::Io {
            operation: "读取插件归档",
            source,
        })?;
    if bytes.len() as u64 > MAX_PLUGIN_ARCHIVE_BYTES {
        return Err(PluginPackageArchiveError::ArchiveTooLarge {
            bytes: bytes.len() as u64,
            max: MAX_PLUGIN_ARCHIVE_BYTES,
        });
    }
    Ok(bytes)
}

async fn scan_archive(bytes: &[u8]) -> Result<ArchivePlan, PluginPackageArchiveError> {
    let decoder = GzipDecoder::new(BufReader::new(Cursor::new(bytes.to_vec())));
    let archive = Archive::new(decoder);
    let mut entries = archive
        .entries()
        .map_err(|source| archive_error("创建插件归档条目流", source))?;
    let mut seen = HashSet::new();
    let mut planned = Vec::new();
    let mut total_bytes = 0_u64;

    while let Some(entry) = entries.next().await {
        let mut entry = entry.map_err(|source| archive_error("读取插件归档条目", source))?;
        if planned.len() >= MAX_PLUGIN_ARCHIVE_ENTRIES {
            return Err(PluginPackageArchiveError::TooManyEntries {
                entries: planned.len().saturating_add(1),
                max: MAX_PLUGIN_ARCHIVE_ENTRIES,
            });
        }
        let archive_path = entry
            .path()
            .map_err(|source| archive_error("读取插件归档路径", source))?;
        let path = validate_entry_path(std::path::Path::new(archive_path.as_os_str()))?;
        if !seen.insert(path.clone()) {
            return Err(PluginPackageArchiveError::DuplicateEntry { path });
        }
        let kind = entry_kind(&entry, &path)?;
        let size = entry
            .header()
            .entry_size()
            .map_err(|source| archive_error("读取插件归档条目大小", source))?;
        if kind == ArchiveEntryKind::Directory && size != 0 {
            return Err(PluginPackageArchiveError::DirectoryHasData { path, bytes: size });
        }
        if kind == ArchiveEntryKind::File {
            total_bytes = total_bytes.saturating_add(size);
            if total_bytes > MAX_PLUGIN_ARCHIVE_UNPACKED_BYTES {
                return Err(PluginPackageArchiveError::UnpackedTooLarge {
                    bytes: total_bytes,
                    max: MAX_PLUGIN_ARCHIVE_UNPACKED_BYTES,
                });
            }
        }
        futures::io::copy(&mut entry, &mut futures::io::sink())
            .await
            .map_err(|source| archive_error("扫描插件归档内容", source))?;
        planned.push(ArchiveEntryPlan { path, kind, size });
    }
    Ok(ArchivePlan { entries: planned })
}

async fn extract_archive(
    bytes: &[u8],
    destination: &Path,
    plan: &ArchivePlan,
) -> Result<(), PluginPackageArchiveError> {
    let decoder = GzipDecoder::new(BufReader::new(Cursor::new(bytes.to_vec())));
    let archive = Archive::new(decoder);
    let mut entries = archive
        .entries()
        .map_err(|source| archive_error("创建插件归档解包流", source))?;
    for expected in &plan.entries {
        let Some(entry) = entries.next().await else {
            return Err(PluginPackageArchiveError::ArchiveChanged);
        };
        let mut entry = entry.map_err(|source| archive_error("读取插件归档解包条目", source))?;
        let archive_path = entry
            .path()
            .map_err(|source| archive_error("读取插件归档解包路径", source))?;
        let path = validate_entry_path(std::path::Path::new(archive_path.as_os_str()))?;
        let kind = entry_kind(&entry, &path)?;
        let size = entry
            .header()
            .entry_size()
            .map_err(|source| archive_error("读取插件归档解包大小", source))?;
        if path != expected.path || kind != expected.kind || size != expected.size {
            return Err(PluginPackageArchiveError::ArchiveChanged);
        }
        if !entry
            .unpack_in(destination)
            .await
            .map_err(|source| archive_error("解包插件归档条目", source))?
        {
            return Err(PluginPackageArchiveError::ExtractionPathRejected { path });
        }
    }
    if entries.next().await.is_some() {
        return Err(PluginPackageArchiveError::ArchiveChanged);
    }
    Ok(())
}

fn validate_entry_path(path: &Path) -> Result<PathBuf, PluginPackageArchiveError> {
    let mut clean = PathBuf::new();
    let mut has_component = false;
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                clean.push(part);
                has_component = true;
            }
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(PluginPackageArchiveError::InvalidEntryPath {
                    path: path.to_path_buf(),
                });
            }
            Component::CurDir => {
                return Err(PluginPackageArchiveError::InvalidEntryPath {
                    path: path.to_path_buf(),
                });
            }
        }
    }
    if !has_component || clean.to_string_lossy().len() > MAX_PLUGIN_ARCHIVE_PATH_BYTES {
        return Err(PluginPackageArchiveError::InvalidEntryPath {
            path: path.to_path_buf(),
        });
    }
    Ok(clean)
}

fn entry_kind<R>(
    entry: &async_tar::Entry<R>,
    path: &Path,
) -> Result<ArchiveEntryKind, PluginPackageArchiveError>
where
    R: futures::io::AsyncRead + Unpin,
{
    match entry.header().entry_type() {
        EntryType::Regular => Ok(ArchiveEntryKind::File),
        EntryType::Directory => Ok(ArchiveEntryKind::Directory),
        entry_type => Err(PluginPackageArchiveError::UnsupportedEntry {
            path: path.to_path_buf(),
            entry_type: format!("{entry_type:?}"),
        }),
    }
}

fn archive_error(operation: &'static str, source: std::io::Error) -> PluginPackageArchiveError {
    PluginPackageArchiveError::Archive { operation, source }
}

fn ensure_packages_root_for_archive(
    packages_root: &Path,
) -> Result<PathBuf, PluginPackageInstallError> {
    if !packages_root.is_absolute() {
        return Err(PluginPackageInstallError::PackagesRootNotAbsolute {
            path: packages_root.to_path_buf(),
        });
    }
    fs::create_dir_all(packages_root).map_err(|source| PluginPackageInstallError::Io {
        operation: "创建插件包目录",
        source,
    })?;
    if fs::symlink_metadata(packages_root)
        .map_err(|source| PluginPackageInstallError::Io {
            operation: "检查插件包目录",
            source,
        })?
        .file_type()
        .is_symlink()
    {
        return Err(PluginPackageInstallError::PackagesRootSymlink {
            path: packages_root.to_path_buf(),
        });
    }
    packages_root
        .canonicalize()
        .map_err(|source| PluginPackageInstallError::Io {
            operation: "解析插件包目录",
            source,
        })
}

fn create_archive_temp_directory(
    packages_root: &Path,
) -> Result<PathBuf, PluginPackageArchiveError> {
    let Some(parent) = packages_root.parent() else {
        return Err(PluginPackageArchiveError::TempDirectory {
            operation: "定位插件归档临时目录",
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "插件包目录没有父目录"),
        });
    };
    let path = parent.join(format!(
        ".ramag-plugin-archive-{}-{}",
        std::process::id(),
        ARCHIVE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).map_err(|source| PluginPackageArchiveError::TempDirectory {
        operation: "创建插件归档临时目录",
        source,
    })?;
    Ok(path)
}

struct TemporaryArchiveDirectory {
    path: PathBuf,
}

impl TemporaryArchiveDirectory {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TemporaryArchiveDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path) {
            tracing::warn!(
                operation = "plugin_archive_temp_cleanup",
                path = %self.path.display(),
                error = %error,
                "删除插件归档临时目录失败"
            );
        }
    }
}

#[derive(Debug, Error)]
pub enum PluginPackageArchiveError {
    #[error("插件归档路径不是绝对路径：{path}")]
    ArchivePathNotAbsolute { path: PathBuf },
    #[error("插件归档不是普通文件：{path}")]
    ArchivePathNotFile { path: PathBuf },
    #[error("插件归档超过 {max} 字节（实际 {bytes} 字节）")]
    ArchiveTooLarge { bytes: u64, max: u64 },
    #[error("插件归档摘要不匹配：期望 {expected}，实际 {actual}")]
    ArchiveDigestMismatch { expected: String, actual: String },
    #[error("插件归档读取失败：{operation}：{source}")]
    Io {
        operation: &'static str,
        source: std::io::Error,
    },
    #[error("插件归档解包失败：{operation}：{source}")]
    Archive {
        operation: &'static str,
        source: std::io::Error,
    },
    #[error("插件归档条目路径无效：{path}")]
    InvalidEntryPath { path: PathBuf },
    #[error("插件归档条目重复：{path}")]
    DuplicateEntry { path: PathBuf },
    #[error("插件归档条目超过 {max} 项（实际至少 {entries} 项）")]
    TooManyEntries { entries: usize, max: usize },
    #[error("插件归档解包内容超过 {max} 字节（实际 {bytes} 字节）")]
    UnpackedTooLarge { bytes: u64, max: u64 },
    #[error("插件归档包含不支持的条目类型 {entry_type}：{path}")]
    UnsupportedEntry { path: PathBuf, entry_type: String },
    #[error("插件归档目录条目包含数据：{path}（{bytes} 字节）")]
    DirectoryHasData { path: PathBuf, bytes: u64 },
    #[error("插件归档在扫描和解包之间发生变化")]
    ArchiveChanged,
    #[error("插件归档解包路径被拒绝：{path}")]
    ExtractionPathRejected { path: PathBuf },
    #[error("插件归档临时目录操作失败：{operation}：{source}")]
    TempDirectory {
        operation: &'static str,
        source: std::io::Error,
    },
    #[error("插件包目录安装失败：{0}")]
    Installer(#[source] PluginPackageInstallError),
}

#[cfg(test)]
#[path = "plugin_package_archive_tests.rs"]
mod tests;
