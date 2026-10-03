//! System Pulse's complete monitor hosted inside Ramag's GPUI application.
//! The source layout is preserved so upstream behavior can be compared directly.

mod assets;
mod controls;
mod dashboard;
mod diagnostics;
#[cfg(test)]
mod fixture;
mod layout;
mod live;
mod meters;
#[cfg(test)]
mod native_tests;
mod panel;
mod panel_context;
mod processes;
mod screen_charts;
mod screen_data;
mod screen_pages;
mod screen_style;
mod screen_summary;
#[cfg(test)]
mod screen_tests;
pub mod screens;
mod settings;
mod storage;
#[cfg(test)]
mod test_support;
pub mod workspace;

pub use screens::ApplicationView as SystemView;

/// Dispatch fixed-operation helpers before GPUI or storage initialization.
/// The bounded argument list includes the executable; ordinary launches retain
/// their original privileges and return None without changing a process.
pub fn system_helper_entry(arguments: impl IntoIterator<Item = std::ffi::OsString>) -> Option<i32> {
    let arguments = arguments.into_iter().take(10).collect::<Vec<_>>();
    ramag_infra_system::windows_thermal::helper_entry(arguments.iter().cloned()).or_else(|| {
        ramag_infra_system::process_control::helper_entry(arguments.into_iter().skip(1))
    })
}

#[cfg(test)]
mod helper_tests {
    use super::system_helper_entry;

    #[test]
    fn helpers_reject_invalid_requests_before_application_startup() {
        assert_eq!(
            system_helper_entry(["ramag", "--other"].map(Into::into)),
            None
        );
        assert_eq!(
            system_helper_entry(["ramag", "--system-pulse-process-action"].map(Into::into)),
            Some(13)
        );
        assert!(
            system_helper_entry(["ramag", "--system-pulse-cpu-temperature-helper"].map(Into::into))
                .is_some_and(|code| code != 0)
        );
    }
}

use gpui_kit::{App, AppContext as _, Entity, Window};
use ramag_domain::traits::{Tool, ToolMeta};

/// 在主窗口中创建系统工具视图；采集工作由视图内部的后台任务执行。
pub fn create_system_view(window: &mut Window, cx: &mut App) -> Entity<SystemView> {
    if let Err(error) = assets::install(cx) {
        eprintln!("{error}");
    }
    cx.new(|cx_inner| SystemView::new(window, cx_inner))
}

/// 系统工具在 Ramag 工具注册表中的元数据。
pub struct SystemTool {
    meta: ToolMeta,
}

impl SystemTool {
    pub const ID: &'static str = "system";

    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(Self::ID, "系统监控", "查看本机性能、磁盘、网络和运行中进程")
                .with_icon("gauge"),
        }
    }
}

impl Default for SystemTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for SystemTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_metadata_exposes_system_entry() {
        let tool = SystemTool::new();
        assert_eq!(tool.meta().id, SystemTool::ID);
        assert_eq!(tool.meta().name, "系统监控");
        assert!(tool.meta().icon.is_some());
    }
}
