//! 插件设置的版本化保存、有限迁移和备份恢复；不处理敏感设置。

use std::collections::BTreeMap;
use std::sync::Arc;

use ramag_domain::traits::MAX_PLUGIN_SETTINGS;
use ramag_domain::{DomainError, PluginDescriptor, PluginSettingValue, Storage};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{PluginSettingsError, PluginSettingsSnapshot};

/// 当前插件设置包络格式；版本变化必须通过后续迁移器处理。
pub const CURRENT_PLUGIN_SETTINGS_FORMAT_VERSION: u16 = 1;
/// 插件设置单条偏好允许进入编码器的最大字节数。
pub const MAX_PLUGIN_SETTINGS_PAYLOAD_BYTES: usize = 128 * 1024;
/// 单次加载最多执行的连续迁移步骤数。
pub const MAX_PLUGIN_SETTINGS_MIGRATION_STEPS: usize = 8;

/// 迁移函数失败时使用的无正文错误，避免设置值进入诊断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginSettingsMigrationFailure;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginSettingsMigrationRegistrationError {
    #[error("插件设置迁移必须是连续版本：{from_version} -> {to_version}")]
    NonConsecutiveVersions { from_version: u16, to_version: u16 },
    #[error("插件设置迁移版本 {from_version} 已注册")]
    DuplicateStep { from_version: u16 },
    #[error("插件设置迁移步骤超过 {max} 项上限")]
    TooManySteps { max: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginSettingsMigrationError {
    #[error("缺少从插件设置版本 {from_version} 开始的迁移步骤")]
    MissingStep { from_version: u16 },
    #[error("插件设置迁移 {from_version} -> {to_version} 失败")]
    Failed { from_version: u16, to_version: u16 },
    #[error("插件设置迁移步骤超过 {max} 项上限")]
    TooManySteps { max: usize },
    #[error("插件设置迁移输出超过 {max} 个键的上限")]
    TooManyValues { max: usize },
}

type MigrationFunction = Arc<
    dyn Fn(
            BTreeMap<String, PluginSettingValue>,
        ) -> Result<BTreeMap<String, PluginSettingValue>, PluginSettingsMigrationFailure>
        + Send
        + Sync,
>;

struct MigrationStep {
    to_version: u16,
    migrate: MigrationFunction,
}

/// 保存有界、连续的设置版本迁移步骤；迁移函数不得产生外部副作用。
#[derive(Default)]
pub struct PluginSettingsMigrator {
    steps: BTreeMap<u16, MigrationStep>,
}

impl Clone for PluginSettingsMigrator {
    fn clone(&self) -> Self {
        Self {
            steps: self
                .steps
                .iter()
                .map(|(from_version, step)| {
                    (
                        *from_version,
                        MigrationStep {
                            to_version: step.to_version,
                            migrate: step.migrate.clone(),
                        },
                    )
                })
                .collect(),
        }
    }
}

impl PluginSettingsMigrator {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册单步版本转换；版本必须连续，且总步骤数有上限。
    pub fn register<F>(
        &mut self,
        from_version: u16,
        to_version: u16,
        migrate: F,
    ) -> Result<(), PluginSettingsMigrationRegistrationError>
    where
        F: Fn(
                BTreeMap<String, PluginSettingValue>,
            )
                -> Result<BTreeMap<String, PluginSettingValue>, PluginSettingsMigrationFailure>
            + Send
            + Sync
            + 'static,
    {
        if to_version != from_version.saturating_add(1) {
            return Err(
                PluginSettingsMigrationRegistrationError::NonConsecutiveVersions {
                    from_version,
                    to_version,
                },
            );
        }
        if self.steps.contains_key(&from_version) {
            return Err(PluginSettingsMigrationRegistrationError::DuplicateStep { from_version });
        }
        if self.steps.len() >= MAX_PLUGIN_SETTINGS_MIGRATION_STEPS {
            return Err(PluginSettingsMigrationRegistrationError::TooManySteps {
                max: MAX_PLUGIN_SETTINGS_MIGRATION_STEPS,
            });
        }
        self.steps.insert(
            from_version,
            MigrationStep {
                to_version,
                migrate: Arc::new(migrate),
            },
        );
        Ok(())
    }

    fn migrate(
        &self,
        from_version: u16,
        mut values: BTreeMap<String, PluginSettingValue>,
    ) -> Result<BTreeMap<String, PluginSettingValue>, PluginSettingsMigrationError> {
        let mut version = from_version;
        for _ in 0..MAX_PLUGIN_SETTINGS_MIGRATION_STEPS {
            if version == CURRENT_PLUGIN_SETTINGS_FORMAT_VERSION {
                return Ok(values);
            }
            let step =
                self.steps
                    .get(&version)
                    .ok_or(PluginSettingsMigrationError::MissingStep {
                        from_version: version,
                    })?;
            let from_version = version;
            values = (step.migrate)(values).map_err(|_| PluginSettingsMigrationError::Failed {
                from_version,
                to_version: step.to_version,
            })?;
            if values.len() > MAX_PLUGIN_SETTINGS {
                return Err(PluginSettingsMigrationError::TooManyValues {
                    max: MAX_PLUGIN_SETTINGS,
                });
            }
            version = step.to_version;
        }
        Err(PluginSettingsMigrationError::TooManySteps {
            max: MAX_PLUGIN_SETTINGS_MIGRATION_STEPS,
        })
    }
}

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
    #[error("插件设置迁移失败：{source}")]
    Migration {
        #[source]
        source: PluginSettingsMigrationError,
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
    migrator: PluginSettingsMigrator,
}

impl PluginSettingsStore {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self::with_migrator(storage, PluginSettingsMigrator::default())
    }

    pub fn with_migrator(storage: Arc<dyn Storage>, migrator: PluginSettingsMigrator) -> Self {
        Self { storage, migrator }
    }

    /// 读取主记录；主记录缺失或损坏时尝试上一份备份，均失败则拒绝启动插件。
    pub async fn load(
        &self,
        descriptor: &PluginDescriptor,
    ) -> Result<PluginSettingsLoad, PluginSettingsStoreError> {
        let primary_key = primary_key(descriptor);
        let primary = self.read(&primary_key).await?;
        match primary {
            Some(raw) => match decode(descriptor, &raw, &self.migrator) {
                Ok(snapshot) => Ok(PluginSettingsLoad {
                    snapshot,
                    recovered_from_backup: false,
                }),
                Err(primary_error) => {
                    let backup = self.read(&backup_key(descriptor)).await?;
                    let Some(backup) = backup else {
                        return Err(primary_error);
                    };
                    decode(descriptor, &backup, &self.migrator)
                        .map(|snapshot| PluginSettingsLoad {
                            snapshot,
                            recovered_from_backup: true,
                        })
                        .map_err(|backup_error| recovery_error_pair(primary_error, backup_error))
                }
            },
            None => match self.read(&backup_key(descriptor)).await? {
                Some(backup) => {
                    decode(descriptor, &backup, &self.migrator).map(|snapshot| PluginSettingsLoad {
                        snapshot,
                        recovered_from_backup: true,
                    })
                }
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
    migrator: &PluginSettingsMigrator,
) -> Result<PluginSettingsSnapshot, PluginSettingsStoreError> {
    ensure_payload_size(raw.len())?;
    let envelope = serde_json::from_str::<StoredPluginSettings>(raw)
        .map_err(|_| PluginSettingsStoreError::CorruptPayload)?;
    if envelope.format_version > CURRENT_PLUGIN_SETTINGS_FORMAT_VERSION {
        return Err(PluginSettingsStoreError::UnsupportedFormat {
            version: envelope.format_version,
        });
    }
    let values = migrator
        .migrate(envelope.format_version, envelope.values)
        .map_err(|source| PluginSettingsStoreError::Migration { source })?;
    PluginSettingsSnapshot::from_namespaced_values(descriptor, values).map_err(snapshot_error)
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

fn recovery_error_pair(
    _primary: PluginSettingsStoreError,
    _backup: PluginSettingsStoreError,
) -> PluginSettingsStoreError {
    PluginSettingsStoreError::RecoveryFailed
}

#[cfg(test)]
#[path = "plugin_settings_store_tests.rs"]
mod tests;
