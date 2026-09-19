#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

//! HTTP 与 gRPC API 测试工作台。

mod view;

use std::sync::Arc;

use gpui::{App, AppContext as _, Entity, Window};
use ramag_app::ApiService;
use ramag_domain::traits::{Tool, ToolMeta};

pub use view::ApiView;

/// 创建 API 工作台视图；协议执行由 `ApiService` 负责，视图只编辑请求和展示快照。
pub fn create_api_view(
    service: Arc<ApiService>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ApiView> {
    cx.new(|cx| ApiView::with_service(service, window, cx))
}

/// API 工作台在 Activity Bar 中显示的注册信息。
pub struct ApiTool {
    meta: ToolMeta,
}

impl ApiTool {
    pub const ID: &'static str = "api";

    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(Self::ID, "API 测试", "发送 HTTP 与 gRPC 请求").with_icon("globe"),
        }
    }
}

impl Default for ApiTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ApiTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_metadata_exposes_api_entry() {
        let tool = ApiTool::new();
        assert_eq!(tool.meta().id, ApiTool::ID);
        assert_eq!(tool.meta().name, "API 测试");
        assert!(tool.meta().icon.is_some());
    }
}
