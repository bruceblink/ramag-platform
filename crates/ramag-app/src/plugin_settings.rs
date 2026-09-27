//! 插件设置的命名空间隔离和内存快照；不负责文件持久化或秘密存储。

use std::collections::BTreeMap;

use ramag_domain::{
    PluginDescriptor, PluginId, PluginRegistrationError, PluginSettingDefinition,
    PluginSettingValue,
};
use thiserror::Error;

/// 插件设置在平台配置中的稳定键前缀。
const PLUGIN_NAMESPACE_PREFIX: &str = "plugin.";

/// 单个插件设置快照的校验失败原因。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginSettingsError {
    #[error("插件 `{plugin_id}` 清单无效：{source}")]
    InvalidDescriptor {
        plugin_id: PluginId,
        #[source]
        source: PluginRegistrationError,
    },
    #[error("设置键 `{key}` 不属于插件命名空间 `{expected}`")]
    NamespaceMismatch { expected: String, key: String },
    #[error("插件设置键重复：{key}")]
    DuplicateKey { key: String },
    #[error("插件 `{plugin_id}` 未声明设置 `{key}`")]
    UnknownSetting { plugin_id: PluginId, key: String },
    #[error("插件设置 `{key}` 是敏感值，必须通过秘密存储访问")]
    SensitiveSettingRequiresSecretStorage { key: String },
    #[error("插件设置 `{key}` 无效：{source}")]
    InvalidValue {
        key: String,
        #[source]
        source: PluginRegistrationError,
    },
}

/// 经过清单和类型校验的插件设置快照。
///
/// 快照内部只保存去掉 `plugin.<plugin-id>.` 前缀的设置键，外部读取和更新
/// 只能通过当前插件的定义完成；更新会复制出新快照，旧快照不会被修改。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSettingsSnapshot {
    plugin_id: PluginId,
    namespace: String,
    definitions: BTreeMap<String, PluginSettingDefinition>,
    values: BTreeMap<String, PluginSettingValue>,
}

impl PluginSettingsSnapshot {
    /// 从持久化层候选数据创建快照，并拒绝其他插件命名空间中的键。
    pub fn from_namespaced_values<I, K>(
        descriptor: &PluginDescriptor,
        values: I,
    ) -> Result<Self, PluginSettingsError>
    where
        I: IntoIterator<Item = (K, PluginSettingValue)>,
        K: Into<String>,
    {
        descriptor
            .validate()
            .map_err(|source| PluginSettingsError::InvalidDescriptor {
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
                return Err(PluginSettingsError::NamespaceMismatch {
                    expected: namespace.clone(),
                    key: qualified_key,
                });
            };
            let definition = definitions.get(setting_key).ok_or_else(|| {
                PluginSettingsError::UnknownSetting {
                    plugin_id: plugin_id.clone(),
                    key: setting_key.to_owned(),
                }
            })?;
            if definition.sensitive {
                return Err(PluginSettingsError::SensitiveSettingRequiresSecretStorage {
                    key: setting_key.to_owned(),
                });
            }
            validate_value(definition, &qualified_key, &value)?;
            if snapshot_values
                .insert(setting_key.to_owned(), value)
                .is_some()
            {
                return Err(PluginSettingsError::DuplicateKey { key: qualified_key });
            }
        }

        for definition in definitions.values() {
            if !definition.sensitive
                && let Some(default) = &definition.default
            {
                snapshot_values
                    .entry(definition.key.clone())
                    .or_insert_with(|| default.clone());
            }
        }

        Ok(Self {
            plugin_id,
            namespace,
            definitions,
            values: snapshot_values,
        })
    }

    /// 返回当前插件 ID。
    pub fn plugin_id(&self) -> &PluginId {
        &self.plugin_id
    }

    /// 返回当前插件唯一可写入的设置命名空间前缀。
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// 返回未加命名空间的设置值。
    pub fn get(&self, key: &str) -> Option<&PluginSettingValue> {
        self.values.get(key)
    }

    /// 按完整存储键读取设置，其他插件的命名空间会被拒绝。
    pub fn get_namespaced(
        &self,
        qualified_key: &str,
    ) -> Result<Option<&PluginSettingValue>, PluginSettingsError> {
        let key = self.setting_key(qualified_key)?;
        Ok(self.values.get(key))
    }

    /// 使用未加前缀的清单键生成当前插件的完整存储键。
    pub fn qualified_key(&self, key: &str) -> Result<String, PluginSettingsError> {
        let Some(definition) = self.definitions.get(key) else {
            return Err(PluginSettingsError::UnknownSetting {
                plugin_id: self.plugin_id.clone(),
                key: key.to_owned(),
            });
        };
        if definition.sensitive {
            return Err(PluginSettingsError::SensitiveSettingRequiresSecretStorage {
                key: key.to_owned(),
            });
        }
        Ok(format!("{}{key}", self.namespace))
    }

    /// 返回带命名空间的副本，供后续原子持久化层使用。
    pub fn namespaced_values(&self) -> BTreeMap<String, PluginSettingValue> {
        self.values
            .iter()
            .map(|(key, value)| (format!("{}{key}", self.namespace), value.clone()))
            .collect()
    }

    /// 校验并生成只包含一个新值的快照；当前快照保持不变。
    pub fn update(
        &self,
        key: &str,
        value: PluginSettingValue,
    ) -> Result<Self, PluginSettingsError> {
        let qualified_key = self.qualified_key(key)?;
        self.update_namespaced(&qualified_key, value)
    }

    /// 按完整存储键更新设置，并拒绝越权命名空间。
    pub fn update_namespaced(
        &self,
        qualified_key: &str,
        value: PluginSettingValue,
    ) -> Result<Self, PluginSettingsError> {
        let key = self.setting_key(qualified_key)?;
        let definition =
            self.definitions
                .get(key)
                .ok_or_else(|| PluginSettingsError::UnknownSetting {
                    plugin_id: self.plugin_id.clone(),
                    key: key.to_owned(),
                })?;
        validate_value(definition, qualified_key, &value)?;

        let mut next = self.clone();
        next.values.insert(key.to_owned(), value);
        Ok(next)
    }

    fn setting_key<'a>(&self, qualified_key: &'a str) -> Result<&'a str, PluginSettingsError> {
        qualified_key.strip_prefix(&self.namespace).ok_or_else(|| {
            PluginSettingsError::NamespaceMismatch {
                expected: self.namespace.clone(),
                key: qualified_key.to_owned(),
            }
        })
    }
}

fn namespace_for(plugin_id: &PluginId) -> String {
    format!("{PLUGIN_NAMESPACE_PREFIX}{plugin_id}.")
}

fn validate_value(
    definition: &PluginSettingDefinition,
    qualified_key: &str,
    value: &PluginSettingValue,
) -> Result<(), PluginSettingsError> {
    definition
        .validate_value(value)
        .map_err(|source| PluginSettingsError::InvalidValue {
            key: qualified_key.to_owned(),
            source,
        })
}

#[cfg(test)]
#[path = "plugin_settings_tests.rs"]
mod tests;
