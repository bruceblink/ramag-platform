//! 可安装插件包的目录校验、摘要核对和原子替换；不启动插件进程。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

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
}

impl PluginPackageInstaller {
    pub fn new(
        packages_root: impl Into<PathBuf>,
        state_store: Arc<PluginInstallationStore>,
    ) -> Self {
        Self {
            packages_root: packages_root.into(),
            state_store,
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

fn package_digest(root: &Path) -> Result<String, PluginPackageInstallError> {
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
    #[error("插件包包含不支持的文件类型：{path}")]
    UnsupportedEntry { path: PathBuf },
    #[error("插件状态保存失败：{0}")]
    State(PluginInstallationStoreError),
    #[error("{operation}失败：{source}")]
    Io {
        operation: &'static str,
        source: std::io::Error,
    },
}

#[cfg(test)]
#[path = "plugin_package_installer_tests.rs"]
mod tests;
