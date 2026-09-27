//! 原生 JSON Path 提取器：领域核心通过插件入口执行，界面只负责输入和结果展示。

#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

mod view;

use std::sync::Arc;

use gpui_kit::{App, AppContext as _, Entity, Window};
use ramag_app::{
    PluginContext, PluginEntryFuture, PluginOperationError, StaticPlugin, StaticPluginHost,
};
use ramag_domain::json_path::{
    MAX_JSON_INPUT_BYTES, MAX_JSON_PATH_OUTPUT_BYTES, extract_json_path,
};
use ramag_domain::traits::{
    PluginCapability, PluginDescriptor, PluginEntryDataSpec, PluginEntryDescriptor, PluginId, Tool,
    ToolMeta,
};
use serde::{Deserialize, Serialize};

pub const PLUGIN_ID: &str = "it-tools.json-path";
pub const ENTRY_ID: &str = "json-path-extractor";

/// 传给 JSON Path 插件入口的结构化请求；原始 JSON 保持为文本以支持 JSON5。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonPathRequest {
    pub raw_json: String,
    pub path: String,
}

/// JSON Path 工具在 Activity Bar 中显示的稳定元数据。
pub struct JsonPathTool {
    meta: ToolMeta,
}

impl JsonPathTool {
    pub const ID: &'static str = ENTRY_ID;

    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(
                Self::ID,
                "JSON Path 提取器",
                "从 JSON 或 JSON5 文本中按路径提取值",
            )
            .with_icon("braces"),
        }
    }
}

impl Default for JsonPathTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for JsonPathTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

/// 首个 IT Tools 迁移样例的静态插件；只执行领域核心，不加载外部代码。
pub struct JsonPathPlugin {
    descriptor: PluginDescriptor,
    tool: Arc<JsonPathTool>,
}

impl JsonPathPlugin {
    pub fn new() -> Result<Self, ramag_domain::PluginRegistrationError> {
        let plugin_id = PluginId::new(PLUGIN_ID)?;
        let entry = PluginEntryDescriptor::new(ENTRY_ID, "JSON Path 提取器")
            .with_description("输入 JSON/JSON5 文本和 JSON Path，输出格式化提取结果")
            .with_input(PluginEntryDataSpec::json(MAX_JSON_INPUT_BYTES))
            .with_output(PluginEntryDataSpec::text(MAX_JSON_PATH_OUTPUT_BYTES));
        let descriptor = PluginDescriptor::new(plugin_id, "IT Tools · JSON Path", ENTRY_ID)
            .with_description("IT Tools JSON Path 提取器的原生桌面实现")
            .with_capabilities([
                PluginCapability::new("ui.entry"),
                PluginCapability::new("task.scoped"),
            ])
            .with_entries(vec![entry]);
        Ok(Self {
            descriptor,
            tool: Arc::new(JsonPathTool::new()),
        })
    }
}

impl StaticPlugin for JsonPathPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn tool(&self) -> Arc<dyn Tool> {
        self.tool.clone()
    }

    fn execute(
        &self,
        entry_id: &str,
        input: Vec<u8>,
        context: &PluginContext,
    ) -> Result<PluginEntryFuture, PluginOperationError> {
        context
            .ensure_available()
            .map_err(|error| PluginOperationError::new(error.to_string()))?;
        if entry_id != ENTRY_ID {
            return Err(PluginOperationError::new("未知 JSON Path 入口"));
        }
        let request: JsonPathRequest = serde_json::from_slice(&input)
            .map_err(|error| PluginOperationError::new(format!("JSON Path 请求无效：{error}")))?;
        let output = extract_json_path(&request.raw_json, &request.path)
            .map_err(|error| PluginOperationError::new(error.to_string()))?;
        Ok(Box::pin(async move { Ok(output.into_bytes()) }))
    }
}

/// 把静态 JSON Path 插件加入宿主；调用方须先为 `task.scoped` 配置授予策略。
pub fn register_json_path_plugin(host: &StaticPluginHost) -> Result<(), String> {
    let plugin = JsonPathPlugin::new().map_err(|error| error.to_string())?;
    host.register_plugin(Arc::new(plugin))
        .map_err(|error| error.to_string())
}

/// 创建 JSON Path 原生 GPUI 入口视图。
pub fn create_json_path_view(
    host: Arc<StaticPluginHost>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<JsonPathView> {
    cx.new(|cx| JsonPathView::new(host, window, cx))
}

pub use view::JsonPathView;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use ramag_app::{PluginPermissionPolicy, PluginTaskBudget, ToolRegistry};
    use ramag_domain::PluginCapability;

    #[test]
    fn plugin_metadata_declares_native_json_path_entry() {
        let plugin = JsonPathPlugin::new().expect("static plugin descriptor is valid");
        assert_eq!(plugin.descriptor().id.as_str(), PLUGIN_ID);
        assert_eq!(plugin.descriptor().entry_id, ENTRY_ID);
        assert_eq!(plugin.tools()[0].meta().id, ENTRY_ID);
        assert!(
            plugin
                .descriptor()
                .capabilities
                .iter()
                .any(|capability| capability.as_str() == "task.scoped")
        );
    }

    #[test]
    fn registered_plugin_executes_json_path_through_host_boundary() {
        let mut policy = PluginPermissionPolicy::default();
        policy.grant(
            ramag_domain::PluginId::new(PLUGIN_ID).expect("static plugin ID is valid"),
            PluginCapability::new("task.scoped"),
        );
        let host = StaticPluginHost::with_permission_policy(Arc::new(ToolRegistry::new()), policy);
        register_json_path_plugin(&host).expect("plugin registers");
        assert!(host.initialize_all().is_success());
        let request = JsonPathRequest {
            raw_json: "{ users: [{ name: 'Alice' }] }".into(),
            path: "$.users[0].name".into(),
        };
        let execution = host
            .execute_entry(
                PLUGIN_ID,
                ENTRY_ID,
                serde_json::to_vec(&request).expect("request serializes"),
                PluginTaskBudget::new(Duration::from_secs(1), 1024).expect("budget is valid"),
            )
            .expect("entry executes");
        assert_eq!(
            smol::block_on(execution.join()).expect("execution succeeds"),
            br#""Alice""#
        );
    }
}
