//! 插件敏感设置的加密快照；普通设置快照不能读取此模块的数据。

use std::collections::BTreeMap;
use std::sync::Arc;

use ramag_domain::{
    DomainError, PluginDescriptor, PluginId, PluginRegistrationError, PluginSettingDefinition,
    PluginSettingValue, Storage,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 当前敏感设置包络格式。
pub const CURRENT_PLUGIN_SECRET_FORMAT_VERSION: u16 = 1;
/// 加密前明文包络的最大字节数。
pub const MAX_PLUGIN_SECRET_PLAINTEXT_BYTES: usize = 128 * 1024;
/// 存储中的前缀和十六进制密文的最大字节数。
pub const MAX_PLUGIN_SECRET_STORED_BYTES: usize = 256 * 1024;
const ENCRYPTED_PLUGIN_SECRET_PREFIX: &str = "encrypted-v1:";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginSecretError {
    #[error("插件 `{plugin_id}` 清单无效：{source}")]
    InvalidDescriptor {
        plugin_id: PluginId,
        #[source]
        source: PluginRegistrationError,
    },
    #[error("设置键 `{key}` 不属于插件命名空间 `{expected}`")]
    NamespaceMismatch { expected: String, key: String },
    #[error("插件 `{plugin_id}` 未声明设置 `{key}`")]
    UnknownSetting { plugin_id: PluginId, key: String },
    #[error("设置 `{key}` 未声明为敏感设置")]
    NotSensitive { key: String },
    #[error("插件秘密设置键重复：{key}")]
    DuplicateKey { key: String },
    #[error("插件秘密设置 `{key}` 无效：{source}")]
    InvalidValue {
        key: String,
        #[source]
        source: PluginRegistrationError,
    },
}

#[derive(Debug, Error)]
pub enum PluginSecretStoreError {
    #[error("插件秘密存储失败：{source}")]
    Storage {
        #[source]
        source: DomainError,
    },
    #[error("插件秘密包络无法解析")]
    CorruptPayload,
    #[error("插件秘密格式版本 {version} 不受当前宿主支持")]
    UnsupportedFormat { version: u16 },
    #[error("插件秘密包络超过 {max_bytes} bytes 上限（实际 {bytes} bytes）")]
    PayloadTooLarge { bytes: usize, max_bytes: usize },
    #[error("插件秘密快照无效：{source}")]
    Snapshot {
        #[source]
        source: PluginSecretError,
    },
    #[error("插件主秘密和备份秘密均无法恢复")]
    RecoveryFailed,
}

/// 只包含当前插件、且已通过清单敏感标记和类型校验的秘密快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSecretSnapshot {
    plugin_id: PluginId,
    namespace: String,
    definitions: BTreeMap<String, PluginSettingDefinition>,
    values: BTreeMap<String, PluginSettingValue>,
}

impl PluginSecretSnapshot {
    /// 从命名空间密钥创建敏感设置快照；普通设置键会被拒绝。
    pub fn from_namespaced_values<I, K>(
        descriptor: &PluginDescriptor,
        values: I,
    ) -> Result<Self, PluginSecretError>
    where
        I: IntoIterator<Item = (K, PluginSettingValue)>,
        K: Into<String>,
    {
        descriptor
            .validate()
            .map_err(|source| PluginSecretError::InvalidDescriptor {
                plugin_id: descriptor.id.clone(),
                source,
            })?;
        let plugin_id = descriptor.id.clone();
        let namespace = namespace_for(&plugin_id);
        let definitions = descriptor
            .settings
            .iter()
            .cloned()
            .map(|definition| (definition.key.clone(), definition))
            .collect::<BTreeMap<_, _>>();
        let mut snapshot_values = BTreeMap::new();

        for (qualified_key, value) in values {
            let qualified_key = qualified_key.into();
            let Some(setting_key) = qualified_key.strip_prefix(&namespace) else {
                return Err(PluginSecretError::NamespaceMismatch {
                    expected: namespace.clone(),
                    key: qualified_key,
                });
            };
            let definition =
                definitions
                    .get(setting_key)
                    .ok_or_else(|| PluginSecretError::UnknownSetting {
                        plugin_id: plugin_id.clone(),
                        key: setting_key.to_owned(),
                    })?;
            if !definition.sensitive {
                return Err(PluginSecretError::NotSensitive {
                    key: setting_key.to_owned(),
                });
            }
            validate_value(definition, &qualified_key, &value)?;
            if snapshot_values
                .insert(setting_key.to_owned(), value)
                .is_some()
            {
                return Err(PluginSecretError::DuplicateKey { key: qualified_key });
            }
        }

        Ok(Self {
            plugin_id,
            namespace,
            definitions,
            values: snapshot_values,
        })
    }

    pub fn plugin_id(&self) -> &PluginId {
        &self.plugin_id
    }

    pub fn get(&self, key: &str) -> Option<&PluginSettingValue> {
        self.values.get(key)
    }

    pub fn qualified_key(&self, key: &str) -> Result<String, PluginSecretError> {
        let Some(definition) = self.definitions.get(key) else {
            return Err(PluginSecretError::UnknownSetting {
                plugin_id: self.plugin_id.clone(),
                key: key.to_owned(),
            });
        };
        if !definition.sensitive {
            return Err(PluginSecretError::NotSensitive {
                key: key.to_owned(),
            });
        }
        Ok(format!("{}{key}", self.namespace))
    }

    pub fn namespaced_values(&self) -> BTreeMap<String, PluginSettingValue> {
        self.values
            .iter()
            .map(|(key, value)| (format!("{}{key}", self.namespace), value.clone()))
            .collect()
    }

    /// 校验并返回新的秘密快照；旧快照和已保存密文不会被修改。
    pub fn update(&self, key: &str, value: PluginSettingValue) -> Result<Self, PluginSecretError> {
        let qualified_key = self.qualified_key(key)?;
        let Some(setting_key) = qualified_key.strip_prefix(&self.namespace) else {
            return Err(PluginSecretError::NamespaceMismatch {
                expected: self.namespace.clone(),
                key: qualified_key,
            });
        };
        let Some(definition) = self.definitions.get(setting_key) else {
            return Err(PluginSecretError::UnknownSetting {
                plugin_id: self.plugin_id.clone(),
                key: setting_key.to_owned(),
            });
        };
        validate_value(definition, &qualified_key, &value)?;
        let mut next = self.clone();
        next.values.insert(setting_key.to_owned(), value);
        Ok(next)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredPluginSecrets {
    format_version: u16,
    values: BTreeMap<String, PluginSettingValue>,
}

/// 使用平台主密钥加密保存插件敏感设置，并保留上一份密文备份。
pub struct PluginSecretStore {
    storage: Arc<dyn Storage>,
}

impl PluginSecretStore {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    pub async fn load(
        &self,
        descriptor: &PluginDescriptor,
    ) -> Result<PluginSecretSnapshot, PluginSecretStoreError> {
        let primary = self.read(&primary_key(descriptor)).await?;
        match primary {
            Some(raw) => match self.decode(descriptor, &raw).await {
                Ok(snapshot) => Ok(snapshot),
                Err(primary_error) => {
                    let backup = self.read(&backup_key(descriptor)).await?;
                    let Some(backup) = backup else {
                        return Err(primary_error);
                    };
                    self.decode(descriptor, &backup)
                        .await
                        .map_err(|_| PluginSecretStoreError::RecoveryFailed)
                }
            },
            None => match self.read(&backup_key(descriptor)).await? {
                Some(backup) => self.decode(descriptor, &backup).await,
                None => empty_snapshot(descriptor),
            },
        }
    }

    pub async fn save(
        &self,
        snapshot: &PluginSecretSnapshot,
    ) -> Result<(), PluginSecretStoreError> {
        let encoded = self.encode(snapshot).await?;
        let primary_key = primary_key_for_id(snapshot.plugin_id());
        if let Some(current) = self.read(&primary_key).await? {
            ensure_stored_size(current.len())?;
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

    async fn encode(
        &self,
        snapshot: &PluginSecretSnapshot,
    ) -> Result<String, PluginSecretStoreError> {
        let envelope = StoredPluginSecrets {
            format_version: CURRENT_PLUGIN_SECRET_FORMAT_VERSION,
            values: snapshot.namespaced_values(),
        };
        let plain =
            serde_json::to_vec(&envelope).map_err(|_| PluginSecretStoreError::CorruptPayload)?;
        ensure_plaintext_size(plain.len())?;
        let encrypted = self.storage.seal(&plain).await.map_err(storage_error)?;
        let stored = format!("{ENCRYPTED_PLUGIN_SECRET_PREFIX}{}", hex::encode(encrypted));
        ensure_stored_size(stored.len())?;
        Ok(stored)
    }

    async fn decode(
        &self,
        descriptor: &PluginDescriptor,
        stored: &str,
    ) -> Result<PluginSecretSnapshot, PluginSecretStoreError> {
        ensure_stored_size(stored.len())?;
        let encoded = stored
            .strip_prefix(ENCRYPTED_PLUGIN_SECRET_PREFIX)
            .ok_or(PluginSecretStoreError::CorruptPayload)?;
        let encrypted = hex::decode(encoded).map_err(|_| PluginSecretStoreError::CorruptPayload)?;
        let plain = self
            .storage
            .unseal(&encrypted)
            .await
            .map_err(storage_error)?;
        ensure_plaintext_size(plain.len())?;
        let envelope = serde_json::from_slice::<StoredPluginSecrets>(&plain)
            .map_err(|_| PluginSecretStoreError::CorruptPayload)?;
        if envelope.format_version != CURRENT_PLUGIN_SECRET_FORMAT_VERSION {
            return Err(PluginSecretStoreError::UnsupportedFormat {
                version: envelope.format_version,
            });
        }
        PluginSecretSnapshot::from_namespaced_values(descriptor, envelope.values)
            .map_err(snapshot_error)
    }

    async fn read(&self, key: &str) -> Result<Option<String>, PluginSecretStoreError> {
        self.storage
            .get_preference(key)
            .await
            .map_err(storage_error)
    }
}

fn empty_snapshot(
    descriptor: &PluginDescriptor,
) -> Result<PluginSecretSnapshot, PluginSecretStoreError> {
    PluginSecretSnapshot::from_namespaced_values(
        descriptor,
        std::iter::empty::<(String, PluginSettingValue)>(),
    )
    .map_err(snapshot_error)
}

fn validate_value(
    definition: &PluginSettingDefinition,
    qualified_key: &str,
    value: &PluginSettingValue,
) -> Result<(), PluginSecretError> {
    definition
        .validate_value(value)
        .map_err(|source| PluginSecretError::InvalidValue {
            key: qualified_key.to_owned(),
            source,
        })
}

fn namespace_for(plugin_id: &PluginId) -> String {
    format!("plugin.{plugin_id}.")
}

fn primary_key(descriptor: &PluginDescriptor) -> String {
    primary_key_for_id(&descriptor.id)
}

fn backup_key(descriptor: &PluginDescriptor) -> String {
    backup_key_for_id(&descriptor.id)
}

fn primary_key_for_id(plugin_id: &PluginId) -> String {
    format!("plugin.{plugin_id}.__secrets.v1")
}

fn backup_key_for_id(plugin_id: &PluginId) -> String {
    format!("plugin.{plugin_id}.__secrets.backup.v1")
}

fn ensure_plaintext_size(bytes: usize) -> Result<(), PluginSecretStoreError> {
    if bytes > MAX_PLUGIN_SECRET_PLAINTEXT_BYTES {
        return Err(PluginSecretStoreError::PayloadTooLarge {
            bytes,
            max_bytes: MAX_PLUGIN_SECRET_PLAINTEXT_BYTES,
        });
    }
    Ok(())
}

fn ensure_stored_size(bytes: usize) -> Result<(), PluginSecretStoreError> {
    if bytes > MAX_PLUGIN_SECRET_STORED_BYTES {
        return Err(PluginSecretStoreError::PayloadTooLarge {
            bytes,
            max_bytes: MAX_PLUGIN_SECRET_STORED_BYTES,
        });
    }
    Ok(())
}

fn storage_error(source: DomainError) -> PluginSecretStoreError {
    PluginSecretStoreError::Storage { source }
}

fn snapshot_error(source: PluginSecretError) -> PluginSecretStoreError {
    PluginSecretStoreError::Snapshot { source }
}

#[cfg(test)]
#[path = "plugin_secrets_tests.rs"]
mod tests;
