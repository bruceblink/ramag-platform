#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

//! Docker 与 Kubernetes 容器管理工具的静态入口和工作台。

mod view;

use std::sync::Arc;

use gpui::{App, AppContext as _, Entity, Window};
use ramag_app::{ContainerRegistryService, ContainerService};
use ramag_domain::traits::{Tool, ToolMeta};

pub use view::{ContainerSection, ContainerView};

/// 创建容器管理工具视图；连接读取和资源操作在后续切片接入。
pub fn create_container_view(
    service: Arc<ContainerService>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ContainerView> {
    cx.new(|cx| ContainerView::with_service(service, window, cx))
}

pub fn create_container_view_with_registry(
    service: Arc<ContainerService>,
    registry_service: Arc<ContainerRegistryService>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ContainerView> {
    cx.new(|cx| ContainerView::with_services(service, registry_service, window, cx))
}

/// 容器管理工具在 Activity Bar 中显示的注册信息。
pub struct ContainerTool {
    meta: ToolMeta,
}

impl ContainerTool {
    pub const ID: &'static str = "container";

    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(Self::ID, "容器管理", "查看 Docker 与 Kubernetes 资源")
                .with_icon("box"),
        }
    }
}

impl Default for ContainerTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ContainerTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_metadata_exposes_container_entry() {
        let tool = ContainerTool::new();
        assert_eq!(tool.meta().id, ContainerTool::ID);
        assert_eq!(tool.meta().name, "容器管理");
        assert!(tool.meta().icon.is_some());
    }
}
