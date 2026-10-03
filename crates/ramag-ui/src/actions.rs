use gpui_kit::Action;
use schemars::JsonSchema;
use serde::Deserialize;

/// 主修饰键+W 关 Tab。各 Tool Session 先消费，没消费则冒泡到 main.rs 关窗
#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, JsonSchema, Action)]
#[action(namespace = ramag)]
pub struct CloseTab;

/// 主修饰键+P 打开当前工具的最近项目列表。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, JsonSchema, Action)]
#[action(namespace = ramag)]
pub struct OpenRecentItems;

/// 从首页工具卡片打开对应的工具入口。
#[derive(Clone, PartialEq, Eq, Debug, Deserialize, JsonSchema, Action)]
#[action(namespace = ramag)]
pub struct OpenTool {
    pub tool_id: String,
}

/// 从工具栏打开该工具设置；工具 ID 只用于查找已注册设置页，不执行外部代码。
#[derive(Clone, PartialEq, Eq, Debug, Deserialize, JsonSchema, Action)]
#[action(namespace = ramag)]
pub struct OpenToolSettings {
    pub tool_id: String,
}

/// Opens Ramag's application-level System settings page from embedded tools.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, JsonSchema, Action)]
#[action(namespace = ramag)]
pub struct OpenSystemSettings;
