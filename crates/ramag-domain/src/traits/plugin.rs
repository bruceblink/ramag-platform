//! 插件描述、能力声明和静态注册校验，不包含动态代码加载。

use std::{collections::HashSet, fmt};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 当前宿主支持的插件 API 版本。
pub const CURRENT_PLUGIN_API_VERSION: PluginApiVersion = PluginApiVersion::new(1, 0);
/// 插件 ID、入口 ID 和设置键允许的最大字节数。
pub const MAX_PLUGIN_ID_BYTES: usize = 64;
pub const MAX_PLUGIN_ENTRY_ID_BYTES: usize = 128;
pub const MAX_PLUGIN_SETTING_KEY_BYTES: usize = 64;
/// 插件显示元数据允许的最大字节数。
pub const MAX_PLUGIN_NAME_BYTES: usize = 128;
pub const MAX_PLUGIN_DESCRIPTION_BYTES: usize = 512;
/// 单个插件清单的资源上限。
pub const MAX_PLUGIN_CAPABILITIES: usize = 16;
pub const MAX_PLUGIN_SETTINGS: usize = 64;
pub const MAX_PLUGIN_ENUM_VALUES: usize = 64;
pub const MAX_PLUGIN_SETTING_VALUE_BYTES: usize = 1024;
pub const MAX_PLUGIN_SETTING_LIST_ITEMS: usize = 64;

/// P0 阶段允许插件声明的能力集合；未知能力必须在注册前拒绝。
pub const KNOWN_PLUGIN_CAPABILITIES: &[&str] = &[
    "ui.entry",
    "ui.notification",
    "storage.plugin",
    "task.scoped",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginApiVersion {
    pub major: u16,
    pub minor: u16,
}

impl PluginApiVersion {
    /// 创建插件 API 版本；主版本不兼容时宿主必须拒绝插件。
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// 判断插件版本是否能由给定宿主版本提供。
    pub const fn is_supported_by(self, host: Self) -> bool {
        self.major == host.major && self.minor <= host.minor
    }
}

impl fmt::Display for PluginApiVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginId(String);

impl PluginId {
    /// 构造稳定插件 ID；只接受小写 ASCII 标识，避免显示名称成为身份。
    pub fn new(value: impl Into<String>) -> Result<Self, PluginRegistrationError> {
        let value = value.into();
        validate_identifier("插件 ID", &value, MAX_PLUGIN_ID_BYTES)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for PluginId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginCapability(String);

impl PluginCapability {
    /// 保留原始能力名称，让清单可以被解析后再由注册校验拒绝未知值。
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_known(&self) -> bool {
        KNOWN_PLUGIN_CAPABILITIES.contains(&self.as_str())
    }
}

impl AsRef<str> for PluginCapability {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginSettingKind {
    Boolean,
    Integer,
    String,
    Enum,
    StringList,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginSettingValue {
    Boolean(bool),
    Integer(i64),
    String(String),
    StringList(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSettingDefinition {
    pub key: String,
    pub kind: PluginSettingKind,
    pub default: Option<PluginSettingValue>,
    pub sensitive: bool,
    pub enum_values: Vec<String>,
}

impl PluginSettingDefinition {
    /// 创建一个有界设置定义；默认值和枚举值通过后续 builder 方法补充。
    pub fn new(key: impl Into<String>, kind: PluginSettingKind) -> Self {
        Self {
            key: key.into(),
            kind,
            default: None,
            sensitive: false,
            enum_values: Vec::new(),
        }
    }

    pub fn with_default(mut self, default: PluginSettingValue) -> Self {
        self.default = Some(default);
        self
    }

    pub fn sensitive(mut self, sensitive: bool) -> Self {
        self.sensitive = sensitive;
        self
    }

    pub fn with_enum_values<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.enum_values = values.into_iter().map(Into::into).collect();
        self
    }

    /// 校验设置键、枚举值和默认值，防止未定义数据进入持久化层。
    pub fn validate(&self) -> Result<(), PluginRegistrationError> {
        validate_identifier("插件设置键", &self.key, MAX_PLUGIN_SETTING_KEY_BYTES).map_err(
            |error| PluginRegistrationError::InvalidSetting {
                key: self.key.clone(),
                reason: error.to_string(),
            },
        )?;

        if self.kind != PluginSettingKind::Enum && !self.enum_values.is_empty() {
            return Err(invalid_setting(&self.key, "只有 Enum 设置可以声明枚举值"));
        }
        if self.kind == PluginSettingKind::Enum {
            if self.enum_values.is_empty() {
                return Err(invalid_setting(&self.key, "Enum 设置至少需要一个枚举值"));
            }
            if self.enum_values.len() > MAX_PLUGIN_ENUM_VALUES {
                return Err(invalid_setting(
                    &self.key,
                    format!("枚举值超过 {MAX_PLUGIN_ENUM_VALUES} 项"),
                ));
            }
            let mut values = HashSet::with_capacity(self.enum_values.len());
            for value in &self.enum_values {
                validate_setting_text(&self.key, value)?;
                if !values.insert(value) {
                    return Err(invalid_setting(&self.key, "枚举值重复"));
                }
            }
        }

        if let Some(default) = &self.default {
            self.validate_default(default)?;
        }
        Ok(())
    }

    fn validate_default(
        &self,
        default: &PluginSettingValue,
    ) -> Result<(), PluginRegistrationError> {
        let kind_matches = matches!(
            (&self.kind, default),
            (PluginSettingKind::Boolean, PluginSettingValue::Boolean(_))
                | (PluginSettingKind::Integer, PluginSettingValue::Integer(_))
                | (PluginSettingKind::String, PluginSettingValue::String(_))
                | (PluginSettingKind::Enum, PluginSettingValue::String(_))
                | (
                    PluginSettingKind::StringList,
                    PluginSettingValue::StringList(_)
                )
        );
        if !kind_matches {
            return Err(invalid_setting(&self.key, "默认值类型与设置类型不匹配"));
        }

        match default {
            PluginSettingValue::String(value) => {
                validate_setting_text(&self.key, value)?;
                if self.kind == PluginSettingKind::Enum
                    && !self.enum_values.iter().any(|item| item == value)
                {
                    return Err(invalid_setting(&self.key, "默认枚举值不在枚举列表中"));
                }
            }
            PluginSettingValue::StringList(values) => {
                if values.len() > MAX_PLUGIN_SETTING_LIST_ITEMS {
                    return Err(invalid_setting(
                        &self.key,
                        format!("字符串列表超过 {MAX_PLUGIN_SETTING_LIST_ITEMS} 项"),
                    ));
                }
                for value in values {
                    validate_setting_text(&self.key, value)?;
                }
            }
            PluginSettingValue::Boolean(_) | PluginSettingValue::Integer(_) => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginDescriptor {
    pub id: PluginId,
    pub api_version: PluginApiVersion,
    pub name: String,
    pub description: String,
    pub entry_id: String,
    pub capabilities: Vec<PluginCapability>,
    pub settings: Vec<PluginSettingDefinition>,
}

impl PluginDescriptor {
    /// 创建内置工具使用的插件描述，默认提供一个 UI 入口能力。
    pub fn new(id: PluginId, name: impl Into<String>, entry_id: impl Into<String>) -> Self {
        Self {
            id,
            api_version: CURRENT_PLUGIN_API_VERSION,
            name: name.into(),
            description: String::new(),
            entry_id: entry_id.into(),
            capabilities: vec![PluginCapability::new("ui.entry")],
            settings: Vec::new(),
        }
    }

    pub fn with_api_version(mut self, api_version: PluginApiVersion) -> Self {
        self.api_version = api_version;
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn with_capabilities<I>(mut self, capabilities: I) -> Self
    where
        I: IntoIterator<Item = PluginCapability>,
    {
        self.capabilities = capabilities.into_iter().collect();
        self
    }

    pub fn with_settings(mut self, settings: Vec<PluginSettingDefinition>) -> Self {
        self.settings = settings;
        self
    }

    /// 按当前宿主 API 校验插件清单。
    pub fn validate(&self) -> Result<(), PluginRegistrationError> {
        self.validate_for(CURRENT_PLUGIN_API_VERSION)
    }

    /// 按指定宿主 API 校验清单，供未来兼容性测试复用。
    pub fn validate_for(
        &self,
        host_api_version: PluginApiVersion,
    ) -> Result<(), PluginRegistrationError> {
        validate_identifier("插件 ID", self.id.as_str(), MAX_PLUGIN_ID_BYTES)?;
        if !self.api_version.is_supported_by(host_api_version) {
            return Err(PluginRegistrationError::UnsupportedApiVersion {
                plugin: self.api_version,
                host: host_api_version,
            });
        }
        validate_text("插件名称", &self.name, MAX_PLUGIN_NAME_BYTES, false)?;
        validate_text(
            "插件描述",
            &self.description,
            MAX_PLUGIN_DESCRIPTION_BYTES,
            true,
        )?;
        validate_identifier("插件入口 ID", &self.entry_id, MAX_PLUGIN_ENTRY_ID_BYTES)?;

        if self.capabilities.len() > MAX_PLUGIN_CAPABILITIES {
            return Err(PluginRegistrationError::TooManyCapabilities {
                max: MAX_PLUGIN_CAPABILITIES,
            });
        }
        let mut capabilities = HashSet::with_capacity(self.capabilities.len());
        for capability in &self.capabilities {
            if !capability.is_known() {
                return Err(PluginRegistrationError::UnknownCapability {
                    capability: capability.as_str().to_owned(),
                });
            }
            if !capabilities.insert(capability) {
                return Err(PluginRegistrationError::DuplicateCapability {
                    capability: capability.as_str().to_owned(),
                });
            }
        }

        if self.settings.len() > MAX_PLUGIN_SETTINGS {
            return Err(PluginRegistrationError::TooManySettings {
                max: MAX_PLUGIN_SETTINGS,
            });
        }
        let mut setting_keys = HashSet::with_capacity(self.settings.len());
        for setting in &self.settings {
            setting.validate()?;
            if !setting_keys.insert(&setting.key) {
                return Err(PluginRegistrationError::DuplicateSetting {
                    key: setting.key.clone(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginRegistrationError {
    #[error("{field}不能为空")]
    EmptyField { field: &'static str },
    #[error("{field}超过 {max_bytes} 字节")]
    FieldTooLong {
        field: &'static str,
        max_bytes: usize,
    },
    #[error("{field}包含非法字符：{value}")]
    InvalidIdentifier { field: &'static str, value: String },
    #[error("插件 API {plugin} 不受宿主 API {host} 支持")]
    UnsupportedApiVersion {
        plugin: PluginApiVersion,
        host: PluginApiVersion,
    },
    #[error("未知插件能力：{capability}")]
    UnknownCapability { capability: String },
    #[error("插件能力重复：{capability}")]
    DuplicateCapability { capability: String },
    #[error("插件能力超过 {max} 项")]
    TooManyCapabilities { max: usize },
    #[error("插件设置 `{key}` 无效：{reason}")]
    InvalidSetting { key: String, reason: String },
    #[error("插件设置重复：{key}")]
    DuplicateSetting { key: String },
    #[error("插件设置超过 {max} 项")]
    TooManySettings { max: usize },
    #[error("插件 ID 已注册：{id}")]
    DuplicatePluginId { id: PluginId },
    #[error("工具入口已注册：{entry_id}")]
    DuplicateEntryId { entry_id: String },
    #[error("插件 `{plugin_id}` 的入口 `{entry_id}` 与工具 ID `{tool_id}` 不一致")]
    EntryIdMismatch {
        plugin_id: PluginId,
        entry_id: String,
        tool_id: String,
    },
}

fn validate_identifier(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), PluginRegistrationError> {
    if value.is_empty() {
        return Err(PluginRegistrationError::EmptyField { field });
    }
    if value.len() > max_bytes {
        return Err(PluginRegistrationError::FieldTooLong { field, max_bytes });
    }
    let valid = value.bytes().enumerate().all(|(index, byte)| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || (index > 0 && matches!(byte, b'.' | b'_' | b'-'))
    });
    if !valid || !value.as_bytes()[0].is_ascii_alphanumeric() {
        return Err(PluginRegistrationError::InvalidIdentifier {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_text(
    field: &'static str,
    value: &str,
    max_bytes: usize,
    allow_empty: bool,
) -> Result<(), PluginRegistrationError> {
    if !allow_empty && value.is_empty() {
        return Err(PluginRegistrationError::EmptyField { field });
    }
    if value.len() > max_bytes {
        return Err(PluginRegistrationError::FieldTooLong { field, max_bytes });
    }
    Ok(())
}

fn validate_setting_text(key: &str, value: &str) -> Result<(), PluginRegistrationError> {
    if value.is_empty() {
        return Err(invalid_setting(key, "字符串值不能为空"));
    }
    if value.len() > MAX_PLUGIN_SETTING_VALUE_BYTES {
        return Err(invalid_setting(
            key,
            format!("字符串值超过 {MAX_PLUGIN_SETTING_VALUE_BYTES} 字节"),
        ));
    }
    Ok(())
}

fn invalid_setting(key: &str, reason: impl Into<String>) -> PluginRegistrationError {
    PluginRegistrationError::InvalidSetting {
        key: key.to_owned(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> Result<PluginDescriptor, PluginRegistrationError> {
        let plugin_id = PluginId::new("example.tool")?;
        Ok(PluginDescriptor::new(plugin_id, "Example", "example")
            .with_description("A built-in example tool")
            .with_capabilities([
                PluginCapability::new("ui.entry"),
                PluginCapability::new("task.scoped"),
            ])
            .with_settings(vec![
                PluginSettingDefinition::new("mode", PluginSettingKind::Enum)
                    .with_enum_values(["safe", "full"])
                    .with_default(PluginSettingValue::String("safe".into())),
            ]))
    }

    #[test]
    fn valid_descriptor_passes_all_registration_checks() {
        let Ok(descriptor) = descriptor() else {
            return;
        };
        assert!(descriptor.validate().is_ok());
        assert!(PluginApiVersion::new(1, 0).is_supported_by(CURRENT_PLUGIN_API_VERSION));
    }

    #[test]
    fn identifiers_reject_uppercase_and_path_characters() {
        assert!(matches!(
            PluginId::new("Example.Tool"),
            Err(PluginRegistrationError::InvalidIdentifier { .. })
        ));
        assert!(matches!(
            PluginId::new("../tool"),
            Err(PluginRegistrationError::InvalidIdentifier { .. })
        ));
    }

    #[test]
    fn unsupported_api_version_is_rejected() {
        let Ok(descriptor) = descriptor() else {
            return;
        };
        let descriptor = descriptor.with_api_version(PluginApiVersion::new(2, 0));
        assert!(matches!(
            descriptor.validate(),
            Err(PluginRegistrationError::UnsupportedApiVersion { .. })
        ));
    }

    #[test]
    fn unknown_and_duplicate_capabilities_are_rejected() {
        let Ok(descriptor) = descriptor() else {
            return;
        };
        let unknown = descriptor
            .clone()
            .with_capabilities([PluginCapability::new("network.any")]);
        assert!(matches!(
            unknown.validate(),
            Err(PluginRegistrationError::UnknownCapability { .. })
        ));

        let duplicate = descriptor.with_capabilities([
            PluginCapability::new("ui.entry"),
            PluginCapability::new("ui.entry"),
        ]);
        assert!(matches!(
            duplicate.validate(),
            Err(PluginRegistrationError::DuplicateCapability { .. })
        ));
    }

    #[test]
    fn setting_schema_rejects_type_and_enum_default_mismatches() {
        let Ok(descriptor) = descriptor() else {
            return;
        };
        let wrong_type = descriptor.clone().with_settings(vec![
            PluginSettingDefinition::new("enabled", PluginSettingKind::Boolean)
                .with_default(PluginSettingValue::String("yes".into())),
        ]);
        assert!(matches!(
            wrong_type.validate(),
            Err(PluginRegistrationError::InvalidSetting { .. })
        ));

        let wrong_enum = descriptor.with_settings(vec![
            PluginSettingDefinition::new("mode", PluginSettingKind::Enum)
                .with_enum_values(["safe", "full"])
                .with_default(PluginSettingValue::String("unknown".into())),
        ]);
        assert!(matches!(
            wrong_enum.validate(),
            Err(PluginRegistrationError::InvalidSetting { .. })
        ));
    }

    #[test]
    fn deserialized_invalid_descriptor_is_caught_before_registration() {
        let Ok(descriptor) = descriptor() else {
            return;
        };
        let serialized = serde_json::to_value(descriptor);
        assert!(serialized.is_ok());
        let Some(mut value) = serialized.ok() else {
            return;
        };
        value["id"] = serde_json::json!("Bad.Plugin");
        let deserialized: Result<PluginDescriptor, _> = serde_json::from_value(value);
        assert!(deserialized.is_ok());
        let Some(descriptor) = deserialized.ok() else {
            return;
        };
        assert!(matches!(
            descriptor.validate(),
            Err(PluginRegistrationError::InvalidIdentifier { .. })
        ));
    }
}
