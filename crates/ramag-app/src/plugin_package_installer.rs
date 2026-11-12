//! 可安装插件包的目录校验、摘要核对和原子替换；不启动插件进程。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;
use ramag_domain::{PluginPackageManifest, PluginPackageManifestError};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PluginInstallation, PluginInstallationStore, PluginInstallationStoreError, PluginProcessError,
    resolve_plugin_entrypoint,
};

/// 插件包清单文件名；内容是 `PluginPackageManifest` 的 JSON 表示。
pub const PLUGIN_PACKAGE_MANIFEST_FILE_NAME: &str = "plugin.json";
/// 单个插件包允许复制和摘要核对的总字节数。
pub const MAX_PLUGIN_PACKAGE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PLUGIN_MANIFEST_BYTES: u64 = 128 * 1024;
static PACKAGE_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 负责把已解包插件目录安装到本地包目录，并更新安装状态。
pub struct PluginPackageInstaller {
    packages_root: PathBuf,
    state_store: Arc<PluginInstallationStore>,
    operation_lock: Arc<Mutex<()>>,
}

impl PluginPackageInstaller {
    pub fn new(
        packages_root: impl Into<PathBuf>,
        state_store: Arc<PluginInstallationStore>,
    ) -> Self {
        Self {
            packages_root: packages_root.into(),
            state_store,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn packages_root(&self) -> &Path {
        &self.packages_root
    }

    /// 读取、校验并安装一个已解包插件目录；清单文件不参与自身摘要计算。
    pub fn install_from_directory(
        &self,
        source_root: &Path,
    ) -> Result<PluginInstallation, PluginPackageInstallError> {
        let _operation = self.operation_lock.lock();
        let source_root = canonical_source_root(source_root)?;
        let packages_root = ensure_packages_root(&self.packages_root)?;
        if source_root.starts_with(&packages_root) {
            return Err(PluginPackageInstallError::SourceInsidePackages {
                source_root,
                packages_root,
            });
        }
        let manifest = read_manifest(&source_root)?;
        resolve_plugin_entrypoint(&source_root, &manifest)
            .map_err(PluginPackageInstallError::Entrypoint)?;
        let digest = package_digest(&source_root)?;
        if digest != manifest.package_sha256 {
            return Err(PluginPackageInstallError::DigestMismatch {
                expected: manifest.package_sha256,
                actual: digest,
            });
        }

        let plugin_dir = packages_root.join(manifest.id.as_str());
        let target = plugin_dir.join(&manifest.version);
        let temp = packages_root.join(format!(
            ".{}.tmp-{}-{}",
            manifest.id,
            std::process::id(),
            PACKAGE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        copy_directory(&source_root, &temp)?;
        let backup = target.with_file_name(format!(
            ".{}.backup-{}-{}",
            manifest.id,
            std::process::id(),
            PACKAGE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let swap_result = swap_package_directory(&temp, &target, &backup);
        if let Err(error) = swap_result {
            let _ = fs::remove_dir_all(&temp);
            return Err(error);
        }

        let installation = PluginInstallation::new(manifest.clone());
        if let Err(error) = self.state_store.upsert(installation.clone()) {
            let _ = rollback_package_directory(&target, &backup);
            return Err(PluginPackageInstallError::State(error));
        }
        let _ = fs::remove_dir_all(&backup);
        Ok(installation)
    }

    /// 卸载已安装插件；先备份版本目录，再删除状态，失败时恢复两者。
    pub fn uninstall(
        &self,
        plugin_id: &ramag_domain::PluginId,
    ) -> Result<bool, PluginPackageInstallError> {
        let _operation = self.operation_lock.lock();
        let installations = self
            .state_store
            .load()
            .map_err(PluginPackageInstallError::State)?;
        let Some(installation) = installations
            .iter()
            .find(|installation| installation.manifest.id == *plugin_id)
            .cloned()
        else {
            return Ok(false);
        };

        let packages_root = ensure_packages_root(&self.packages_root)?;
        let target =
            installed_package_path(&packages_root, plugin_id, &installation.manifest.version)?;
        let backup = target.with_file_name(format!(
            ".{}.uninstall-backup-{}-{}",
            plugin_id,
            std::process::id(),
            PACKAGE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::rename(&target, &backup).map_err(|source| PluginPackageInstallError::Io {
            operation: "备份待卸载插件版本",
            source,
        })?;

        match self.state_store.remove(plugin_id) {
            Ok(true) => {}
            Ok(false) => {
                if let Err(source) = restore_uninstall_directory(&target, &backup) {
                    return Err(PluginPackageInstallError::Rollback {
                        operation: "恢复待卸载插件目录",
                        source,
                    });
                }
                return Ok(false);
            }
            Err(error) => {
                if let Err(restore_error) = restore_uninstall_directory(&target, &backup) {
                    return Err(PluginPackageInstallError::Rollback {
                        operation: "恢复待卸载插件目录",
                        source: restore_error,
                    });
                }
                return Err(PluginPackageInstallError::State(error));
            }
        }

        if let Err(source) = fs::remove_dir_all(&backup) {
            let restore_error = restore_uninstall_directory(&target, &backup).err();
            let state_restore = self.state_store.upsert(installation).err();
            if let Some(restore_error) = restore_error {
                return Err(PluginPackageInstallError::Rollback {
                    operation: "恢复待卸载插件目录",
                    source: restore_error,
                });
            }
            if let Some(state_restore) = state_restore {
                return Err(PluginPackageInstallError::StateRollback {
                    uninstall: source,
                    restore: state_restore,
                });
            }
            return Err(PluginPackageInstallError::Io {
                operation: "删除插件备份目录",
                source,
            });
        }
        remove_empty_plugin_directory(&packages_root, plugin_id);
        Ok(true)
    }
}

/// 校验已安装版本目录只位于插件包根目录内，且不经过符号链接。
fn installed_package_path(
    packages_root: &Path,
    plugin_id: &ramag_domain::PluginId,
    version: &str,
) -> Result<PathBuf, PluginPackageInstallError> {
    let plugin_dir = packages_root.join(plugin_id.as_str());
    let plugin_metadata = fs::symlink_metadata(&plugin_dir).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            PluginPackageInstallError::PackageMissing {
                path: plugin_dir.clone(),
            }
        } else {
            PluginPackageInstallError::Io {
                operation: "检查已安装插件目录",
                source,
            }
        }
    })?;
    if plugin_metadata.file_type().is_symlink() {
        return Err(PluginPackageInstallError::SymlinkEntry { path: plugin_dir });
    }
    if !plugin_metadata.is_dir() {
        return Err(PluginPackageInstallError::UnsupportedEntry { path: plugin_dir });
    }
    let version_path = Path::new(version);
    if !version_path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(PluginPackageInstallError::InvalidRelativePath {
            path: plugin_dir.join(version_path),
        });
    }
    let target = plugin_dir.join(version_path);
    let target_metadata = fs::symlink_metadata(&target).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            PluginPackageInstallError::PackageMissing {
                path: target.clone(),
            }
        } else {
            PluginPackageInstallError::Io {
                operation: "检查已安装插件版本目录",
                source,
            }
        }
    })?;
    if target_metadata.file_type().is_symlink() {
        return Err(PluginPackageInstallError::SymlinkEntry { path: target });
    }
    if !target_metadata.is_dir() {
        return Err(PluginPackageInstallError::UnsupportedEntry { path: target });
    }
    let canonical_target =
        target
            .canonicalize()
            .map_err(|source| PluginPackageInstallError::Io {
                operation: "解析已安装插件版本目录",
                source,
            })?;
    if !canonical_target.starts_with(packages_root) {
        return Err(PluginPackageInstallError::InvalidRelativePath {
            path: canonical_target,
        });
    }
    Ok(target)
}

/// 恢复卸载前的版本目录，发现目标被替换时直接失败以避免误删外部目录。
fn restore_uninstall_directory(target: &Path, backup: &Path) -> Result<(), std::io::Error> {
    match fs::symlink_metadata(target) {
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "待恢复插件目标目录已被替换",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::rename(backup, target)
}

/// 只删除已经为空的插件 ID 目录，保留其他版本目录。
fn remove_empty_plugin_directory(packages_root: &Path, plugin_id: &ramag_domain::PluginId) {
    let plugin_dir = packages_root.join(plugin_id.as_str());
    if let Ok(metadata) = fs::symlink_metadata(&plugin_dir)
        && metadata.is_dir()
        && !metadata.file_type().is_symlink()
    {
        let _ = fs::remove_dir(&plugin_dir);
    }
}

fn canonical_source_root(path: &Path) -> Result<PathBuf, PluginPackageInstallError> {
    if !path.is_absolute() {
        return Err(PluginPackageInstallError::SourceNotAbsolute {
            path: path.to_path_buf(),
        });
    }
    let metadata = fs::symlink_metadata(path).map_err(|source| PluginPackageInstallError::Io {
        operation: "检查插件源目录",
        source,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PluginPackageInstallError::SourceNotDirectory {
            path: path.to_path_buf(),
        });
    }
    path.canonicalize()
        .map_err(|source| PluginPackageInstallError::Io {
            operation: "解析插件源目录",
            source,
        })
}

fn ensure_packages_root(path: &Path) -> Result<PathBuf, PluginPackageInstallError> {
    if !path.is_absolute() {
        return Err(PluginPackageInstallError::PackagesRootNotAbsolute {
            path: path.to_path_buf(),
        });
    }
    fs::create_dir_all(path).map_err(|source| PluginPackageInstallError::Io {
        operation: "创建插件包目录",
        source,
    })?;
    if fs::symlink_metadata(path)
        .map_err(|source| PluginPackageInstallError::Io {
            operation: "检查插件包目录",
            source,
        })?
        .file_type()
        .is_symlink()
    {
        return Err(PluginPackageInstallError::PackagesRootSymlink {
            path: path.to_path_buf(),
        });
    }
    path.canonicalize()
        .map_err(|source| PluginPackageInstallError::Io {
            operation: "解析插件包目录",
            source,
        })
}

fn read_manifest(root: &Path) -> Result<PluginPackageManifest, PluginPackageInstallError> {
    let path = root.join(PLUGIN_PACKAGE_MANIFEST_FILE_NAME);
    let metadata = fs::metadata(&path).map_err(|source| PluginPackageInstallError::Io {
        operation: "读取插件清单元数据",
        source,
    })?;
    if metadata.len() > MAX_PLUGIN_MANIFEST_BYTES {
        return Err(PluginPackageInstallError::ManifestTooLarge {
            bytes: metadata.len(),
            max: MAX_PLUGIN_MANIFEST_BYTES,
        });
    }
    let bytes = fs::read(&path).map_err(|source| PluginPackageInstallError::Io {
        operation: "读取插件清单",
        source,
    })?;
    let manifest = serde_json::from_slice::<PluginPackageManifest>(&bytes)
        .map_err(PluginPackageInstallError::ManifestJson)?;
    manifest
        .validate()
        .map_err(PluginPackageInstallError::Manifest)?;
    Ok(manifest)
}

/// 按稳定的相对路径和文件内容计算包摘要；进程宿主启动前复用该规则核验已安装目录。
pub(crate) fn package_digest(root: &Path) -> Result<String, PluginPackageInstallError> {
    let mut files = BTreeMap::new();
    collect_files(root, root, &mut files)?;
    let mut total_bytes = 0_u64;
    let mut hasher = Sha256::new();
    for (relative, path) in files {
        if relative == Path::new(PLUGIN_PACKAGE_MANIFEST_FILE_NAME) {
            continue;
        }
        let bytes = fs::read(&path).map_err(|source| PluginPackageInstallError::Io {
            operation: "读取插件包文件",
            source,
        })?;
        total_bytes = total_bytes.saturating_add(bytes.len() as u64);
        if total_bytes > MAX_PLUGIN_PACKAGE_BYTES {
            return Err(PluginPackageInstallError::PackageTooLarge {
                bytes: total_bytes,
                max: MAX_PLUGIN_PACKAGE_BYTES,
            });
        }
        let relative = relative.to_string_lossy();
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(&bytes);
        hasher.update([0]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut BTreeMap<PathBuf, PathBuf>,
) -> Result<(), PluginPackageInstallError> {
    let mut entries = fs::read_dir(current)
        .map_err(|source| PluginPackageInstallError::Io {
            operation: "扫描插件包目录",
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| PluginPackageInstallError::Io {
            operation: "读取插件包目录项",
            source,
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|source| PluginPackageInstallError::Io {
                operation: "检查插件包路径",
                source,
            })?;
        if metadata.file_type().is_symlink() {
            return Err(PluginPackageInstallError::SymlinkEntry { path });
        }
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| PluginPackageInstallError::InvalidRelativePath { path: path.clone() })?
                .to_path_buf();
            if relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err(PluginPackageInstallError::InvalidRelativePath { path });
            }
            files.insert(relative, path);
        } else {
            return Err(PluginPackageInstallError::UnsupportedEntry { path });
        }
    }
    Ok(())
}

fn copy_directory(source: &Path, target: &Path) -> Result<(), PluginPackageInstallError> {
    fs::create_dir_all(target).map_err(|source| PluginPackageInstallError::Io {
        operation: "创建插件临时目录",
        source,
    })?;
    let mut files = BTreeMap::new();
    collect_files(source, source, &mut files)?;
    for (relative, source_file) in files {
        let target_file = target.join(relative);
        if let Some(parent) = target_file.parent() {
            fs::create_dir_all(parent).map_err(|source| PluginPackageInstallError::Io {
                operation: "创建插件文件目录",
                source,
            })?;
        }
        fs::copy(source_file, target_file).map_err(|source| PluginPackageInstallError::Io {
            operation: "复制插件包文件",
            source,
        })?;
    }
    Ok(())
}

fn swap_package_directory(
    temp: &Path,
    target: &Path,
    backup: &Path,
) -> Result<(), PluginPackageInstallError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|source| PluginPackageInstallError::Io {
            operation: "创建插件版本目录",
            source,
        })?;
    }
    let target_exists = fs::symlink_metadata(target).is_ok();
    if target_exists {
        fs::rename(target, backup).map_err(|source| PluginPackageInstallError::Io {
            operation: "备份旧插件版本",
            source,
        })?;
    }
    if let Err(source) = fs::rename(temp, target) {
        if target_exists {
            let _ = fs::rename(backup, target);
        }
        return Err(PluginPackageInstallError::Io {
            operation: "替换插件版本",
            source,
        });
    }
    Ok(())
}

fn rollback_package_directory(target: &Path, backup: &Path) -> Result<(), std::io::Error> {
    let _ = fs::remove_dir_all(target);
    if fs::symlink_metadata(backup).is_ok() {
        fs::rename(backup, target)?;
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum PluginPackageInstallError {
    #[error("插件源目录不是绝对路径：{path}")]
    SourceNotAbsolute { path: PathBuf },
    #[error("插件源目录不是普通目录：{path}")]
    SourceNotDirectory { path: PathBuf },
    #[error("插件包目录不是绝对路径：{path}")]
    PackagesRootNotAbsolute { path: PathBuf },
    #[error("插件包目录是符号链接：{path}")]
    PackagesRootSymlink { path: PathBuf },
    #[error("插件源目录位于插件包目录内：{source_root}")]
    SourceInsidePackages {
        source_root: PathBuf,
        packages_root: PathBuf,
    },
    #[error("插件清单超过 {max} 字节（实际 {bytes} 字节）")]
    ManifestTooLarge { bytes: u64, max: u64 },
    #[error("插件清单 JSON 无法解析：{0}")]
    ManifestJson(serde_json::Error),
    #[error("插件清单无效：{0}")]
    Manifest(PluginPackageManifestError),
    #[error("插件入口无效：{0}")]
    Entrypoint(PluginProcessError),
    #[error("插件包摘要不匹配：期望 {expected}，实际 {actual}")]
    DigestMismatch { expected: String, actual: String },
    #[error("插件包超过 {max} 字节（实际 {bytes} 字节）")]
    PackageTooLarge { bytes: u64, max: u64 },
    #[error("插件包包含符号链接：{path}")]
    SymlinkEntry { path: PathBuf },
    #[error("插件包路径无效：{path}")]
    InvalidRelativePath { path: PathBuf },
    #[error("已安装插件包不存在：{path}")]
    PackageMissing { path: PathBuf },
    #[error("插件包包含不支持的文件类型：{path}")]
    UnsupportedEntry { path: PathBuf },
    #[error("插件状态保存失败：{0}")]
    State(PluginInstallationStoreError),
    #[error("卸载状态恢复失败；目录清理错误：{uninstall}；状态恢复错误：{restore}")]
    StateRollback {
        uninstall: std::io::Error,
        restore: PluginInstallationStoreError,
    },
    #[error("插件卸载回滚失败：{operation}：{source}")]
    Rollback {
        operation: &'static str,
        source: std::io::Error,
    },
    #[error("{operation}失败：{source}")]
    Io {
        operation: &'static str,
        source: std::io::Error,
    },
}

#[cfg(test)]
#[path = "plugin_package_installer_tests.rs"]
mod tests;
