//! 插件包下载缓存的有界索引、恢复和清理；不解包、不执行插件代码。

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ramag_domain::PluginId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    MAX_PLUGIN_CATALOG_VERSION_BYTES, MAX_PLUGIN_DOWNLOAD_BYTES, PLUGIN_PACKAGE_SHA256_HEX_BYTES,
    PluginPackageCatalogEntry, PluginPackageCatalogError,
};

/// 当前缓存索引格式版本。
pub const CURRENT_PLUGIN_PACKAGE_CACHE_FORMAT_VERSION: u16 = 1;
/// 缓存索引允许读取的最大字节数。
pub const MAX_PLUGIN_PACKAGE_CACHE_INDEX_BYTES: usize = 256 * 1024;
/// 缓存索引允许保存的最大条目数。
pub const MAX_PLUGIN_PACKAGE_CACHE_ENTRIES: usize = 256;
/// 单次启动恢复最多重新读取的归档总字节数。
pub const MAX_PLUGIN_PACKAGE_CACHE_RESTORE_BYTES: u64 = 256 * 1024 * 1024;

const CACHE_INDEX_FILE_NAME: &str = "index.json";
static CACHE_INDEX_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 启动后恢复的单个插件包缓存记录；路径已经通过缓存边界核验。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPackageCachedArchive {
    pub plugin_id: PluginId,
    pub version: String,
    pub path: PathBuf,
}

impl PluginPackageCachedArchive {
    /// 返回设置页使用的插件 ID/版本稳定键。
    pub fn key(&self) -> String {
        cache_key(&self.plugin_id, &self.version)
    }
}

/// 负责维护插件下载缓存索引；所有索引写入均使用临时文件和原子替换。
#[derive(Clone)]
pub struct PluginPackageCache {
    root: PathBuf,
    index_path: PathBuf,
    operation_lock: Arc<Mutex<()>>,
}

impl PluginPackageCache {
    pub fn new(root: PathBuf) -> Self {
        let index_path = root.join(CACHE_INDEX_FILE_NAME);
        Self {
            root,
            index_path,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    /// 记录已完成摘要核验的归档，供下一次应用启动恢复。
    pub fn record_download(
        &self,
        entry: &PluginPackageCatalogEntry,
        archive_path: &Path,
    ) -> Result<(), PluginPackageCacheError> {
        entry.validate().map_err(PluginPackageCacheError::Catalog)?;
        let _operation = self.lock()?;
        self.ensure_root_for_write()?;
        let file_name = validate_archive_path(&self.root, archive_path)?;
        verify_archive_digest(archive_path, &entry.archive_sha256)?;
        let mut index = self.read_index()?;
        index.entries.retain(|record| {
            validate_index_entry(record).is_ok()
                && (record.plugin_id != entry.id || record.version != entry.version)
        });
        if index
            .entries
            .iter()
            .any(|record| record.file_name == file_name)
        {
            return Err(PluginPackageCacheError::DuplicateArchive {
                file_name: file_name.to_owned(),
            });
        }
        index.entries.push(CacheIndexEntry {
            plugin_id: entry.id.clone(),
            version: entry.version.clone(),
            archive_sha256: entry.archive_sha256.clone(),
            file_name: file_name.to_owned(),
        });
        self.write_index(&index)
    }

    /// 读取并核验缓存索引；无效条目不会进入返回结果，也不会写入安装状态。
    pub fn restore(
        &self,
        catalog_entries: &[PluginPackageCatalogEntry],
    ) -> Result<Vec<PluginPackageCachedArchive>, PluginPackageCacheError> {
        for entry in catalog_entries {
            entry.validate().map_err(PluginPackageCacheError::Catalog)?;
        }
        let _operation = self.lock()?;
        if !self.ensure_root_for_read()? {
            return Ok(Vec::new());
        }
        let index = self.read_index()?;
        let catalog = catalog_entries
            .iter()
            .map(|entry| ((entry.id.clone(), entry.version.clone()), entry))
            .collect::<HashMap<_, _>>();
        let mut seen_keys = HashSet::with_capacity(index.entries.len());
        let mut seen_files = HashSet::with_capacity(index.entries.len());
        let mut valid_index_entries = Vec::with_capacity(index.entries.len());
        let mut restored = Vec::with_capacity(index.entries.len());
        let mut scanned_bytes = 0_u64;
        for record in &index.entries {
            if validate_index_entry(record).is_err() {
                continue;
            }
            let key = (record.plugin_id.clone(), record.version.clone());
            let Some(entry) = catalog.get(&key) else {
                continue;
            };
            if record.archive_sha256 != entry.archive_sha256
                || !seen_keys.insert(key.clone())
                || !seen_files.insert(record.file_name.clone())
            {
                continue;
            }
            let archive_path = self.root.join(&record.file_name);
            let Some(file_bytes) = bounded_file_size(&archive_path)? else {
                continue;
            };
            if scanned_bytes.saturating_add(file_bytes) > MAX_PLUGIN_PACKAGE_CACHE_RESTORE_BYTES
                || !is_regular_bounded_file(&archive_path)?
                || verify_archive_digest(&archive_path, &entry.archive_sha256).is_err()
            {
                continue;
            }
            scanned_bytes = scanned_bytes.saturating_add(file_bytes);
            valid_index_entries.push(record.clone());
            restored.push(PluginPackageCachedArchive {
                plugin_id: entry.id.clone(),
                version: entry.version.clone(),
                path: archive_path,
            });
        }
        if valid_index_entries.len() != index.entries.len() {
            let repaired = CacheIndex {
                format_version: CURRENT_PLUGIN_PACKAGE_CACHE_FORMAT_VERSION,
                entries: valid_index_entries,
            };
            if let Err(error) = self.write_index(&repaired) {
                tracing::warn!(
                    operation = "plugin_package_cache_repair",
                    path = %self.index_path.display(),
                    error = %error,
                    "插件包缓存索引修复失败"
                );
            }
        }
        Ok(restored)
    }

    /// 删除安装成功后的归档和对应索引记录；删除失败不会回滚已完成安装。
    pub fn remove_download(
        &self,
        entry: &PluginPackageCatalogEntry,
        archive_path: &Path,
    ) -> Result<(), PluginPackageCacheError> {
        entry.validate().map_err(PluginPackageCacheError::Catalog)?;
        let _operation = self.lock()?;
        if !self.ensure_root_for_read()? {
            return Ok(());
        }
        let file_name = validate_archive_path(&self.root, archive_path)?;
        let mut index = self.read_index()?;
        let before = index.entries.len();
        index.entries.retain(|record| {
            validate_index_entry(record).is_ok()
                && (record.plugin_id != entry.id || record.version != entry.version)
                && record.file_name != file_name
        });
        if before != index.entries.len() {
            self.write_index(&index)?;
        }
        match fs::remove_file(archive_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(PluginPackageCacheError::Io {
                operation: "删除插件包缓存",
                source,
            }),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, ()>, PluginPackageCacheError> {
        self.operation_lock
            .lock()
            .map_err(|_| PluginPackageCacheError::LockPoisoned)
    }

    fn ensure_root_for_read(&self) -> Result<bool, PluginPackageCacheError> {
        if !self.root.is_absolute() {
            return Err(PluginPackageCacheError::RootInvalid {
                path: self.root.clone(),
            });
        }
        match fs::symlink_metadata(&self.root) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                Err(PluginPackageCacheError::RootInvalid {
                    path: self.root.clone(),
                })
            }
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(source) => Err(PluginPackageCacheError::Io {
                operation: "读取插件包缓存目录",
                source,
            }),
        }
    }

    fn ensure_root_for_write(&self) -> Result<(), PluginPackageCacheError> {
        if !self.root.is_absolute() {
            return Err(PluginPackageCacheError::RootInvalid {
                path: self.root.clone(),
            });
        }
        fs::create_dir_all(&self.root).map_err(|source| PluginPackageCacheError::Io {
            operation: "创建插件包缓存目录",
            source,
        })?;
        if !self.ensure_root_for_read()? {
            return Err(PluginPackageCacheError::RootInvalid {
                path: self.root.clone(),
            });
        }
        Ok(())
    }

    fn read_index(&self) -> Result<CacheIndex, PluginPackageCacheError> {
        let metadata = match fs::symlink_metadata(&self.index_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CacheIndex::empty());
            }
            Err(source) => {
                return Err(PluginPackageCacheError::Io {
                    operation: "读取插件包缓存索引",
                    source,
                });
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(PluginPackageCacheError::IndexInvalid {
                path: self.index_path.clone(),
            });
        }
        if metadata.len() > MAX_PLUGIN_PACKAGE_CACHE_INDEX_BYTES as u64 {
            return Err(PluginPackageCacheError::IndexTooLarge {
                bytes: metadata.len(),
                max: MAX_PLUGIN_PACKAGE_CACHE_INDEX_BYTES,
            });
        }
        let bytes = fs::read(&self.index_path).map_err(|source| PluginPackageCacheError::Io {
            operation: "读取插件包缓存索引",
            source,
        })?;
        if bytes.len() > MAX_PLUGIN_PACKAGE_CACHE_INDEX_BYTES {
            return Err(PluginPackageCacheError::IndexTooLarge {
                bytes: bytes.len() as u64,
                max: MAX_PLUGIN_PACKAGE_CACHE_INDEX_BYTES,
            });
        }
        let index =
            serde_json::from_slice::<CacheIndex>(&bytes).map_err(PluginPackageCacheError::Json)?;
        if index.format_version != CURRENT_PLUGIN_PACKAGE_CACHE_FORMAT_VERSION {
            return Err(PluginPackageCacheError::UnsupportedFormat {
                version: index.format_version,
            });
        }
        if index.entries.len() > MAX_PLUGIN_PACKAGE_CACHE_ENTRIES {
            return Err(PluginPackageCacheError::TooManyEntries {
                max: MAX_PLUGIN_PACKAGE_CACHE_ENTRIES,
            });
        }
        Ok(index)
    }

    fn write_index(&self, index: &CacheIndex) -> Result<(), PluginPackageCacheError> {
        if index.entries.len() > MAX_PLUGIN_PACKAGE_CACHE_ENTRIES {
            return Err(PluginPackageCacheError::TooManyEntries {
                max: MAX_PLUGIN_PACKAGE_CACHE_ENTRIES,
            });
        }
        for record in &index.entries {
            validate_index_entry(record)?;
        }
        let bytes = serde_json::to_vec(index).map_err(PluginPackageCacheError::Json)?;
        if bytes.len() > MAX_PLUGIN_PACKAGE_CACHE_INDEX_BYTES {
            return Err(PluginPackageCacheError::IndexTooLarge {
                bytes: bytes.len() as u64,
                max: MAX_PLUGIN_PACKAGE_CACHE_INDEX_BYTES,
            });
        }
        let temp_path = self.root.join(format!(
            ".{CACHE_INDEX_FILE_NAME}.part-{}-{}",
            std::process::id(),
            CACHE_INDEX_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)
                .map_err(|source| PluginPackageCacheError::Io {
                    operation: "创建插件包缓存索引临时文件",
                    source,
                })?;
            file.write_all(&bytes)
                .and_then(|()| file.flush())
                .and_then(|()| file.sync_all())
                .map_err(|source| PluginPackageCacheError::Io {
                    operation: "写入插件包缓存索引",
                    source,
                })?;
            fs::rename(&temp_path, &self.index_path).map_err(|source| PluginPackageCacheError::Io {
                operation: "替换插件包缓存索引",
                source,
            })
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheIndex {
    format_version: u16,
    entries: Vec<CacheIndexEntry>,
}

impl CacheIndex {
    fn empty() -> Self {
        Self {
            format_version: CURRENT_PLUGIN_PACKAGE_CACHE_FORMAT_VERSION,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheIndexEntry {
    plugin_id: PluginId,
    version: String,
    archive_sha256: String,
    file_name: String,
}

fn cache_key(plugin_id: &PluginId, version: &str) -> String {
    format!("{plugin_id}-{version}")
}

fn validate_index_entry(record: &CacheIndexEntry) -> Result<(), PluginPackageCacheError> {
    record
        .plugin_id
        .validate()
        .map_err(|source| PluginPackageCacheError::IndexEntry { source })?;
    if record.version.len() > MAX_PLUGIN_CATALOG_VERSION_BYTES
        || semver::Version::parse(&record.version).is_err()
    {
        return Err(PluginPackageCacheError::IndexEntryVersion {
            version: record.version.clone(),
        });
    }
    if record.archive_sha256.len() != PLUGIN_PACKAGE_SHA256_HEX_BYTES
        || !record
            .archive_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || record
            .archive_sha256
            .bytes()
            .any(|byte| byte.is_ascii_uppercase())
    {
        return Err(PluginPackageCacheError::IndexEntryDigest {
            digest: record.archive_sha256.clone(),
        });
    }
    validate_file_name(&record.file_name)
}

fn validate_file_name(file_name: &str) -> Result<(), PluginPackageCacheError> {
    let path = Path::new(file_name);
    let mut components = path.components();
    let Some(Component::Normal(component)) = components.next() else {
        return Err(PluginPackageCacheError::FileNameInvalid {
            file_name: file_name.to_owned(),
        });
    };
    if components.next().is_some()
        || component.to_str().is_none()
        || file_name.len() > 256
        || file_name.chars().any(char::is_control)
        || !file_name.ends_with(".tar.gz")
        || file_name.contains(".part-")
    {
        return Err(PluginPackageCacheError::FileNameInvalid {
            file_name: file_name.to_owned(),
        });
    }
    Ok(())
}

fn validate_archive_path<'a>(
    root: &Path,
    archive_path: &'a Path,
) -> Result<&'a str, PluginPackageCacheError> {
    if !archive_path.is_absolute() || archive_path.parent() != Some(root) {
        return Err(PluginPackageCacheError::ArchivePathInvalid {
            path: archive_path.to_path_buf(),
        });
    }
    let file_name = archive_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| PluginPackageCacheError::ArchivePathInvalid {
            path: archive_path.to_path_buf(),
        })?;
    validate_file_name(file_name)?;
    if !is_regular_bounded_file(archive_path)? {
        return Err(PluginPackageCacheError::ArchivePathInvalid {
            path: archive_path.to_path_buf(),
        });
    }
    Ok(file_name)
}

fn is_regular_bounded_file(path: &Path) -> Result<bool, PluginPackageCacheError> {
    Ok(bounded_file_size(path)?.is_some())
}

fn bounded_file_size(path: &Path) -> Result<Option<u64>, PluginPackageCacheError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(PluginPackageCacheError::Io {
                operation: "读取插件包缓存归档",
                source,
            });
        }
    };
    if metadata.is_file() && metadata.len() <= MAX_PLUGIN_DOWNLOAD_BYTES {
        Ok(Some(metadata.len()))
    } else {
        Ok(None)
    }
}

fn verify_archive_digest(path: &Path, expected: &str) -> Result<(), PluginPackageCacheError> {
    if !is_regular_bounded_file(path)? {
        return Err(PluginPackageCacheError::ArchivePathInvalid {
            path: path.to_path_buf(),
        });
    }
    let mut file = File::open(path).map_err(|source| PluginPackageCacheError::Io {
        operation: "读取插件包缓存归档",
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| PluginPackageCacheError::Io {
                operation: "计算插件包缓存摘要",
                source,
            })?;
        if read == 0 {
            break;
        }
        bytes = bytes.saturating_add(read as u64);
        if bytes > MAX_PLUGIN_DOWNLOAD_BYTES {
            return Err(PluginPackageCacheError::ArchiveTooLarge {
                bytes,
                max: MAX_PLUGIN_DOWNLOAD_BYTES,
            });
        }
        hasher.update(&buffer[..read]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual != expected {
        return Err(PluginPackageCacheError::DigestMismatch {
            expected: expected.to_owned(),
            actual,
        });
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum PluginPackageCacheError {
    #[error("插件包缓存目录无效：{path}")]
    RootInvalid { path: PathBuf },
    #[error("插件包缓存索引无效：{path}")]
    IndexInvalid { path: PathBuf },
    #[error("插件包缓存索引超过 {max} 字节（实际 {bytes} 字节）")]
    IndexTooLarge { bytes: u64, max: usize },
    #[error("插件包缓存索引 JSON 无法解析：{0}")]
    Json(serde_json::Error),
    #[error("插件包缓存索引格式版本 {version} 不受支持")]
    UnsupportedFormat { version: u16 },
    #[error("插件包缓存索引条目超过 {max} 项")]
    TooManyEntries { max: usize },
    #[error("插件包缓存索引中的插件 ID 无效：{source}")]
    IndexEntry {
        source: ramag_domain::PluginRegistrationError,
    },
    #[error("插件包缓存索引中的版本无效：{version}")]
    IndexEntryVersion { version: String },
    #[error("插件包缓存索引中的摘要无效：{digest}")]
    IndexEntryDigest { digest: String },
    #[error("插件包缓存文件名无效：{file_name}")]
    FileNameInvalid { file_name: String },
    #[error("插件包缓存归档路径无效：{path}")]
    ArchivePathInvalid { path: PathBuf },
    #[error("插件包缓存归档超过 {max} 字节（实际 {bytes} 字节）")]
    ArchiveTooLarge { bytes: u64, max: u64 },
    #[error("插件包缓存归档摘要不匹配：期望 {expected}，实际 {actual}")]
    DigestMismatch { expected: String, actual: String },
    #[error("插件包缓存归档文件名重复：{file_name}")]
    DuplicateArchive { file_name: String },
    #[error("插件目录清单无效：{0}")]
    Catalog(PluginPackageCatalogError),
    #[error("插件包缓存操作锁不可用")]
    LockPoisoned,
    #[error("插件包缓存 I/O 失败：{operation}：{source}")]
    Io {
        operation: &'static str,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
#[path = "plugin_package_cache_tests.rs"]
mod tests;
