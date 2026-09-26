//! 插件设置的版本化保存和备份恢复；不迁移旧格式，也不处理敏感设置。

use std::collections::BTreeMap;
use std::sync::Arc;

use ramag_domain::{DomainError, PluginDescriptor, PluginSettingValue, Storage};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{PluginSettingsError, PluginSettingsSnapshot};

/// 当前插件设置包络格式；版本变化必须通过后续迁移器处理。
pub const CURRENT_PLUGIN_SETTINGS_FORMAT_VERSION: u16 = 1;
/// 插件设置单条偏好允许进入编码器的最大字节数。
pub const MAX_PLUGIN_SETTINGS_PAYLOAD_BYTES: usize = 128 * 1024;

#[derive(Debug, Error)]
pub enum PluginSettingsStoreError {
    #[error("插件设置存储失败：{source}")]
    Storage {
        #[source]
        source: DomainError,
    },
    #[error("插件设置包络无法解析")]
    CorruptPayload,
    #[error("插件设置格式版本 {version} 不受当前宿主支持")]
    UnsupportedFormat { version: u16 },
    #[error("插件设置包络超过 {max_bytes} bytes 上限（实际 {bytes} bytes）")]
    PayloadTooLarge { bytes: usize, max_bytes: usize },
    #[error("插件设置快照无效：{source}")]
    Snapshot {
        #[source]
        source: PluginSettingsError,
    },
    #[error("插件主设置和备份设置均无法恢复")]
    RecoveryFailed,
}

/// 一次读取操作的结果；恢复备份时保留可诊断状态供设置页展示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSettingsLoad {
    pub snapshot: PluginSettingsSnapshot,
    pub recovered_from_backup: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredPluginSettings {
    format_version: u16,
    values: BTreeMap<String, PluginSettingValue>,
}

/// 通过平台 `Storage` 偏好接口保存插件设置，并保持上一份有效数据。
pub struct PluginSettingsStore {
    storage: Arc<dyn Storage>,
}

impl PluginSettingsStore {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    /// 读取主记录；主记录缺失或损坏时尝试上一份备份，均失败则拒绝启动插件。
    pub async fn load(
        &self,
        descriptor: &PluginDescriptor,
    ) -> Result<PluginSettingsLoad, PluginSettingsStoreError> {
        let primary_key = primary_key(descriptor);
        let primary = self.read(&primary_key).await?;
        match primary {
            Some(raw) => match decode(descriptor, &raw) {
                Ok(snapshot) => Ok(PluginSettingsLoad {
                    snapshot,
                    recovered_from_backup: false,
                }),
                Err(primary_error) => {
                    let backup = self.read(&backup_key(descriptor)).await?;
                    let Some(backup) = backup else {
                        return Err(recovery_error(primary_error));
                    };
                    decode(descriptor, &backup)
                        .map(|snapshot| PluginSettingsLoad {
                            snapshot,
                            recovered_from_backup: true,
                        })
                        .map_err(|backup_error| recovery_error_pair(primary_error, backup_error))
                }
            },
            None => match self.read(&backup_key(descriptor)).await? {
                Some(backup) => decode(descriptor, &backup)
                    .map(|snapshot| PluginSettingsLoad {
                        snapshot,
                        recovered_from_backup: true,
                    })
                    .map_err(recovery_error),
                None => empty_snapshot(descriptor).map(|snapshot| PluginSettingsLoad {
                    snapshot,
                    recovered_from_backup: false,
                }),
            },
        }
    }

    /// 先备份当前主记录，再用一次 Storage 写入替换主记录；主记录写失败时旧值仍在。
    pub async fn save(
        &self,
        snapshot: &PluginSettingsSnapshot,
    ) -> Result<(), PluginSettingsStoreError> {
        let encoded = encode(snapshot)?;
        let primary_key = primary_key_for_id(snapshot.plugin_id());
        if let Some(current) = self.read(&primary_key).await? {
            ensure_payload_size(current.len())?;
            self.storage
                .set_preference(&backup_key_for_id(snapshot.plugin_id()), &current)
                .await
                .map_err(storage_error)?;
        }
        self.storage
            .set_preference(&primary_key, &encoded)
            .await
            .map_err(storage_error)
    }

    async fn read(&self, key: &str) -> Result<Option<String>, PluginSettingsStoreError> {
        self.storage
            .get_preference(key)
            .await
            .map_err(storage_error)
    }
}

fn empty_snapshot(
    descriptor: &PluginDescriptor,
) -> Result<PluginSettingsSnapshot, PluginSettingsStoreError> {
    PluginSettingsSnapshot::from_namespaced_values(
        descriptor,
        std::iter::empty::<(String, PluginSettingValue)>(),
    )
    .map_err(snapshot_error)
}

fn encode(snapshot: &PluginSettingsSnapshot) -> Result<String, PluginSettingsStoreError> {
    let envelope = StoredPluginSettings {
        format_version: CURRENT_PLUGIN_SETTINGS_FORMAT_VERSION,
        values: snapshot.namespaced_values(),
    };
    let encoded =
        serde_json::to_string(&envelope).map_err(|_| PluginSettingsStoreError::CorruptPayload)?;
    ensure_payload_size(encoded.len())?;
    Ok(encoded)
}

fn decode(
    descriptor: &PluginDescriptor,
    raw: &str,
) -> Result<PluginSettingsSnapshot, PluginSettingsStoreError> {
    ensure_payload_size(raw.len())?;
    let envelope = serde_json::from_str::<StoredPluginSettings>(raw)
        .map_err(|_| PluginSettingsStoreError::CorruptPayload)?;
    if envelope.format_version != CURRENT_PLUGIN_SETTINGS_FORMAT_VERSION {
        return Err(PluginSettingsStoreError::UnsupportedFormat {
            version: envelope.format_version,
        });
    }
    PluginSettingsSnapshot::from_namespaced_values(descriptor, envelope.values)
        .map_err(snapshot_error)
}

fn ensure_payload_size(bytes: usize) -> Result<(), PluginSettingsStoreError> {
    if bytes > MAX_PLUGIN_SETTINGS_PAYLOAD_BYTES {
        return Err(PluginSettingsStoreError::PayloadTooLarge {
            bytes,
            max_bytes: MAX_PLUGIN_SETTINGS_PAYLOAD_BYTES,
        });
    }
    Ok(())
}

fn primary_key(descriptor: &PluginDescriptor) -> String {
    primary_key_for_id(&descriptor.id)
}

fn backup_key(descriptor: &PluginDescriptor) -> String {
    backup_key_for_id(&descriptor.id)
}

fn primary_key_for_id(plugin_id: &ramag_domain::PluginId) -> String {
    format!("plugin.{plugin_id}.__snapshot.v1")
}

fn backup_key_for_id(plugin_id: &ramag_domain::PluginId) -> String {
    format!("plugin.{plugin_id}.__snapshot.backup.v1")
}

fn storage_error(source: DomainError) -> PluginSettingsStoreError {
    PluginSettingsStoreError::Storage { source }
}

fn snapshot_error(source: PluginSettingsError) -> PluginSettingsStoreError {
    PluginSettingsStoreError::Snapshot { source }
}

fn recovery_error(_error: PluginSettingsStoreError) -> PluginSettingsStoreError {
    PluginSettingsStoreError::RecoveryFailed
}

fn recovery_error_pair(
    _primary: PluginSettingsStoreError,
    _backup: PluginSettingsStoreError,
) -> PluginSettingsStoreError {
    PluginSettingsStoreError::RecoveryFailed
}

#[cfg(test)]
#[path = "plugin_settings_store_tests.rs"]
mod tests;
