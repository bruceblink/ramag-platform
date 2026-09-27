//! 本机优先协作插件：GPUI 只编排用户确认，数据由应用服务保存到加密 Storage。

#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

mod view;

use std::sync::Arc;

use gpui_kit::{App, AppContext as _, Entity, Window};
use ramag_domain::traits::{ClipboardDriver, Storage, Tool, ToolMeta};

pub const PLUGIN_ID: &str = "ramag.collaboration";
pub const ENTRY_ID: &str = "collaboration";

/// 本机协作入口的稳定 Activity Bar 元数据；网络传输能力不由插件声明。
pub struct CollaborationTool {
    meta: ToolMeta,
}

impl CollaborationTool {
    pub const ID: &'static str = ENTRY_ID;

    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(
                Self::ID,
                "本机协作",
                "选择性保存、预览和撤销本机加密协作草稿",
            )
            .with_icon("users"),
        }
    }
}

impl Default for CollaborationTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for CollaborationTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

/// 创建原生 GPUI 协作入口；Storage 由组合根注入，视图不直接打开数据库文件。
pub fn create_collaboration_view(
    storage: Arc<dyn Storage>,
    clipboard: Option<Arc<dyn ClipboardDriver>>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<CollaborationView> {
    let view = cx.new(|cx| CollaborationView::new(storage, clipboard, window, cx));
    view.update(cx, |view, cx| view.reload(cx));
    view
}

pub use view::CollaborationView;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_declares_local_only_entry() {
        let tool = CollaborationTool::new();
        assert_eq!(tool.meta().id, ENTRY_ID);
        assert!(tool.meta().description.contains("本机"));
    }
}
