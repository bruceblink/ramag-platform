//! 可安装插件的清单状态存储；本阶段只保存元数据，不启动外部插件代码。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicU64, Ordering};

use ramag_domain::{PluginId, PluginPackageManifest, PluginPackageManifestError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 本地安装状态文件最多记录的插件数量。
pub const MAX_PLUGIN_INSTALLATIONS: usize = 256;
/// 隔离原因允许保存的最大字节数。
pub const MAX_PLUGIN_INSTALLATION_ERROR_BYTES: usize = 512;
const INSTALLATIONS_FILE_NAME: &str = "installed.json";
const MAX_INSTALLATIONS_FILE_BYTES: usize = 1024 * 1024;
#[cfg(target_os = "windows")]
static INSTALLATION_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 插件包的本地生命周期状态；启用状态不会触发进程启动。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginInstallationState {
    Installed,
    Enabled,
    Disabled,
    Quarantined,
}

impl std::fmt::Display for PluginInstallationState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Installed => "installed",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::Quarantined => "quarantined",
        })
    }
}

/// 一个已识别插件包的清单、状态和最近失败原因。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInstallation {
    pub manifest: PluginPackageManifest,
    pub state: PluginInstallationState,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl PluginInstallation {
    pub fn new(manifest: PluginPackageManifest) -> Self {
        Self {
            manifest,
            state: PluginInstallationState::Installed,
            last_error: None,
        }
    }

    /// 在写入状态文件前验证清单和错误消息边界。
    pub fn validate(&self) -> Result<(), PluginInstallationError> {
        self.manifest
            .validate()
            .map_err(|source| PluginInstallationError::Manifest {
                plugin_id: self.manifest.id.clone(),
                source,
            })?;
        if let Some(error) = &self.last_error
            && error.len() > MAX_PLUGIN_INSTALLATION_ERROR_BYTES
        {
            return Err(PluginInstallationError::ErrorTooLong {
                max_bytes: MAX_PLUGIN_INSTALLATION_ERROR_BYTES,
            });
        }
        Ok(())
    }
}

/// 读取、校验并原子写入插件安装状态。
pub struct PluginInstallationStore {
    root: PathBuf,
}

impl PluginInstallationStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn path(&self) -> PathBuf {
        self.root.join(INSTALLATIONS_FILE_NAME)
    }

    /// 读取全部安装状态；文件不存在时返回空列表。
    pub fn load(&self) -> Result<Vec<PluginInstallation>, PluginInstallationStoreError> {
        let path = self.path();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(storage_error("读取插件安装状态", source)),
        };
        if bytes.len() > MAX_INSTALLATIONS_FILE_BYTES {
            return Err(PluginInstallationStoreError::FileTooLarge {
                bytes: bytes.len(),
                max_bytes: MAX_INSTALLATIONS_FILE_BYTES,
            });
        }
        let installations = serde_json::from_slice::<Vec<PluginInstallation>>(&bytes)
            .map_err(PluginInstallationStoreError::CorruptFile)?;
        validate_installations(installations)
    }

    /// 替换全部安装状态；写入完成前保留原状态文件。
    pub fn save(
        &self,
        installations: &[PluginInstallation],
    ) -> Result<(), PluginInstallationStoreError> {
        let mut installations = installations.to_vec();
        validate_installations(installations.clone())?;
        installations
            .sort_by(|left, right| left.manifest.id.as_str().cmp(right.manifest.id.as_str()));
        let bytes = serde_json::to_vec_pretty(&installations)
            .map_err(PluginInstallationStoreError::Serialize)?;
        if bytes.len() > MAX_INSTALLATIONS_FILE_BYTES {
            return Err(PluginInstallationStoreError::FileTooLarge {
                bytes: bytes.len(),
                max_bytes: MAX_INSTALLATIONS_FILE_BYTES,
            });
        }
        fs::create_dir_all(&self.root)
            .map_err(|source| storage_error("创建插件状态目录", source))?;
        write_atomic(&self.path(), &bytes)
    }

    /// 插入新插件或替换同 ID 的现有清单。
    pub fn upsert(
        &self,
        installation: PluginInstallation,
    ) -> Result<(), PluginInstallationStoreError> {
        installation.validate()?;
        let mut installations = self.load()?;
        if let Some(current) = installations
            .iter_mut()
            .find(|current| current.manifest.id == installation.manifest.id)
        {
            *current = installation;
        } else {
            if installations.len() >= MAX_PLUGIN_INSTALLATIONS {
                return Err(PluginInstallationStoreError::TooManyInstallations {
                    max: MAX_PLUGIN_INSTALLATIONS,
                });
            }
            installations.push(installation);
        }
        self.save(&installations)
    }

    /// 更新已安装插件状态，并返回更新后的记录。
    pub fn set_state(
        &self,
        plugin_id: &PluginId,
        state: PluginInstallationState,
        last_error: Option<String>,
    ) -> Result<PluginInstallation, PluginInstallationStoreError> {
        let mut installations = self.load()?;
        let installation = installations
            .iter_mut()
            .find(|installation| installation.manifest.id == *plugin_id)
            .ok_or_else(|| PluginInstallationStoreError::NotFound {
                plugin_id: plugin_id.clone(),
            })?;
        installation.state = state;
        installation.last_error = last_error;
        installation.validate()?;
        let updated = installation.clone();
        self.save(&installations)?;
        Ok(updated)
    }

    /// 删除本地状态记录；不会删除插件包目录。
    pub fn remove(&self, plugin_id: &PluginId) -> Result<bool, PluginInstallationStoreError> {
        let mut installations = self.load()?;
        let original_len = installations.len();
        installations.retain(|installation| installation.manifest.id != *plugin_id);
        if installations.len() == original_len {
            return Ok(false);
        }
        self.save(&installations)?;
        Ok(true)
    }
}

fn validate_installations(
    installations: Vec<PluginInstallation>,
) -> Result<Vec<PluginInstallation>, PluginInstallationStoreError> {
    if installations.len() > MAX_PLUGIN_INSTALLATIONS {
        return Err(PluginInstallationStoreError::TooManyInstallations {
            max: MAX_PLUGIN_INSTALLATIONS,
        });
    }
    let mut ids = std::collections::HashSet::with_capacity(installations.len());
    for installation in &installations {
        installation.validate()?;
        if !ids.insert(installation.manifest.id.clone()) {
            return Err(PluginInstallationStoreError::DuplicatePlugin {
                plugin_id: installation.manifest.id.clone(),
            });
        }
    }
    Ok(installations)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), PluginInstallationStoreError> {
    let temp_path = path.with_file_name(format!(
        "{INSTALLATIONS_FILE_NAME}.tmp-{}",
        std::process::id()
    ));
    let write_result = (|| -> Result<(), PluginInstallationStoreError> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|source| storage_error("创建插件状态临时文件", source))?;
        file.write_all(bytes)
            .map_err(|source| storage_error("写入插件状态临时文件", source))?;
        file.sync_all()
            .map_err(|source| storage_error("同步插件状态临时文件", source))?;
        drop(file);
        commit_state_temp(&temp_path, path)
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

#[cfg(not(target_os = "windows"))]
fn commit_state_temp(temp_path: &Path, target: &Path) -> Result<(), PluginInstallationStoreError> {
    fs::rename(temp_path, target).map_err(|source| storage_error("替换插件状态文件", source))
}

#[cfg(target_os = "windows")]
fn commit_state_temp(temp_path: &Path, target: &Path) -> Result<(), PluginInstallationStoreError> {
    let exists = match fs::symlink_metadata(target) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(source) => return Err(storage_error("检查插件状态文件", source)),
    };
    if !exists {
        return fs::rename(temp_path, target)
            .map_err(|source| storage_error("保存插件状态文件", source));
    }

    let backup = target.with_file_name(format!(
        "{INSTALLATIONS_FILE_NAME}.backup-{}-{}",
        std::process::id(),
        INSTALLATION_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::rename(target, &backup).map_err(|source| storage_error("备份插件状态文件", source))?;
    match fs::rename(temp_path, target) {
        Ok(()) => {
            if let Err(error) = fs::remove_file(&backup) {
                tracing::warn!(
                    operation = "plugin_installation_cleanup",
                    error = %error,
                    path = %backup.display(),
                    "remove plugin state backup failed"
                );
            }
            Ok(())
        }
        Err(source) => {
            let restore = fs::rename(&backup, target).err();
            let message = restore.map_or_else(
                || format!("替换插件状态文件失败：{source}"),
                |restore| format!("替换插件状态文件失败：{source}；恢复原文件失败：{restore}"),
            );
            Err(PluginInstallationStoreError::Storage {
                operation: "替换插件状态文件",
                source: std::io::Error::other(message),
            })
        }
    }
}

fn storage_error(operation: &'static str, source: std::io::Error) -> PluginInstallationStoreError {
    PluginInstallationStoreError::Storage { operation, source }
}

#[derive(Debug, Error)]
pub enum PluginInstallationError {
    #[error("插件 `{plugin_id}` 清单无效：{source}")]
    Manifest {
        plugin_id: PluginId,
        #[source]
        source: PluginPackageManifestError,
    },
    #[error("插件状态错误消息超过 {max_bytes} 字节")]
    ErrorTooLong { max_bytes: usize },
}

#[derive(Debug, Error)]
pub enum PluginInstallationStoreError {
    #[error("{operation}失败：{source}")]
    Storage {
        operation: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("插件安装状态文件无法解析：{0}")]
    CorruptFile(serde_json::Error),
    #[error("插件安装状态序列化失败：{0}")]
    Serialize(serde_json::Error),
    #[error("插件安装状态文件超过 {max_bytes} 字节（实际 {bytes} 字节）")]
    FileTooLarge { bytes: usize, max_bytes: usize },
    #[error("插件安装记录超过 {max} 项")]
    TooManyInstallations { max: usize },
    #[error("插件安装记录重复：{plugin_id}")]
    DuplicatePlugin { plugin_id: PluginId },
    #[error("未找到插件安装记录：{plugin_id}")]
    NotFound { plugin_id: PluginId },
    #[error("插件安装记录无效：{0}")]
    Invalid(#[from] PluginInstallationError),
}

#[cfg(test)]
#[path = "plugin_installation_tests.rs"]
mod tests;
