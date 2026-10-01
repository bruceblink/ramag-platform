//! 系统监控专属设置页；设置不再占用工作区工具栏。

use gpui_kit::component::{ActiveTheme, v_flex};
use gpui_kit::{AnyElement, Context, IntoElement, ParentElement, Styled, px};

use super::{
    SettingsView,
    pages::settings_card,
    system::{choice_button, setting_row},
};

impl SettingsView {
    pub(super) fn render_monitor_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = crate::monitor_settings(cx);
        let choices = crate::MonitorRefreshRate::ALL
            .into_iter()
            .enumerate()
            .map(|(index, rate)| {
                choice_button(
                    format!("settings-monitor-rate-{index}"),
                    rate.label(),
                    current.refresh_rate == rate,
                    move |_, _, cx| {
                        crate::save_monitor_settings(
                            crate::MonitorSettings { refresh_rate: rate },
                            cx,
                        );
                    },
                )
            })
            .collect();
        v_flex()
            .w_full()
            .gap(px(16.0))
            .child(
                settings_card("采样与刷新", cx.theme().border).child(setting_row(
                    "settings-monitor-rate-row",
                    "刷新频率",
                    "仅影响本机系统监控；立即应用并在下次启动时恢复。",
                    choices,
                    cx.theme(),
                )),
            )
            .child(self.monitor_preset_manager.clone())
            .into_any_element()
    }
}
