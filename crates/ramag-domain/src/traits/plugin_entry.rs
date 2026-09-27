//! 静态插件工具入口的输入输出描述和有界载荷规则。

use serde::{Deserialize, Serialize};

use super::{
    MAX_PLUGIN_DESCRIPTION_BYTES, MAX_PLUGIN_ENTRY_ID_BYTES, MAX_PLUGIN_NAME_BYTES,
    PluginRegistrationError, validate_identifier, validate_text,
};

/// 单个插件清单允许声明的入口数量上限。
pub const MAX_PLUGIN_ENTRIES: usize = 16;
/// 单个入口一次输入或输出允许声明的最大字节数。
pub const MAX_PLUGIN_ENTRY_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_PLUGIN_ENTRY_PAYLOAD_BYTES: usize = 64 * 1024;

/// 标准工具入口支持的数据边界；它描述数据形状，不代表桌面端使用 WebView。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginEntryDataKind {
    Text,
    Json,
    Binary,
}

/// 入口输入或输出的大小上限。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginEntryDataSpec {
    pub kind: PluginEntryDataKind,
    pub max_bytes: usize,
}

impl PluginEntryDataSpec {
    /// 创建入口数据规格；调用方仍需通过入口清单校验才能注册。
    pub const fn new(kind: PluginEntryDataKind, max_bytes: usize) -> Self {
        Self { kind, max_bytes }
    }

    pub const fn text(max_bytes: usize) -> Self {
        Self::new(PluginEntryDataKind::Text, max_bytes)
    }

    pub const fn json(max_bytes: usize) -> Self {
        Self::new(PluginEntryDataKind::Json, max_bytes)
    }

    pub const fn binary(max_bytes: usize) -> Self {
        Self::new(PluginEntryDataKind::Binary, max_bytes)
    }
}

/// 一个插件入口的稳定身份、显示信息以及标准输入输出边界。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginEntryDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub input: PluginEntryDataSpec,
    pub output: PluginEntryDataSpec,
}

impl PluginEntryDescriptor {
    /// 创建一个默认文本输入输出入口，兼容现有单入口静态插件。
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            input: PluginEntryDataSpec::text(DEFAULT_PLUGIN_ENTRY_PAYLOAD_BYTES),
            output: PluginEntryDataSpec::text(DEFAULT_PLUGIN_ENTRY_PAYLOAD_BYTES),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub const fn with_input(mut self, input: PluginEntryDataSpec) -> Self {
        self.input = input;
        self
    }

    pub const fn with_output(mut self, output: PluginEntryDataSpec) -> Self {
        self.output = output;
        self
    }

    /// 校验入口身份、显示文本和输入输出上限，避免未受控的载荷进入统一渲染器。
    pub(super) fn validate(&self) -> Result<(), PluginRegistrationError> {
        validate_identifier("插件入口 ID", &self.id, MAX_PLUGIN_ENTRY_ID_BYTES)
            .map_err(|error| self.invalid(error))?;
        validate_text("插件入口名称", &self.name, MAX_PLUGIN_NAME_BYTES, false)
            .map_err(|error| self.invalid(error))?;
        validate_text(
            "插件入口描述",
            &self.description,
            MAX_PLUGIN_DESCRIPTION_BYTES,
            true,
        )
        .map_err(|error| self.invalid(error))?;
        self.validate_payload("输入", &self.input)?;
        self.validate_payload("输出", &self.output)?;
        Ok(())
    }

    fn validate_payload(
        &self,
        label: &'static str,
        spec: &PluginEntryDataSpec,
    ) -> Result<(), PluginRegistrationError> {
        if spec.max_bytes == 0 || spec.max_bytes > MAX_PLUGIN_ENTRY_PAYLOAD_BYTES {
            return Err(self.invalid(format!(
                "{label}载荷上限必须在 1 到 {MAX_PLUGIN_ENTRY_PAYLOAD_BYTES} 字节之间"
            )));
        }
        Ok(())
    }

    fn invalid(&self, reason: impl ToString) -> PluginRegistrationError {
        PluginRegistrationError::InvalidEntry {
            entry_id: self.id.clone(),
            reason: reason.to_string(),
        }
    }
}
