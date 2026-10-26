//! 监控预设反馈状态卡的 Headless GPUI 布局测试。

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, px, size,
};

use crate::settings_view::pages::{settings_error_notice, settings_success_notice};

struct MonitorPresetStatusHost;

impl Render for MonitorPresetStatusHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("monitor-preset-status-host")
            .debug_selector(|| "monitor-preset-status-host".into())
            .w_full()
            .gap(px(8.0))
            .child(settings_error_notice(
                "monitor-preset-load-error",
                "本地预设库无法读取",
                "原数据已保留；命名预设编辑已停用。",
                cx.theme(),
            ))
            .child(settings_error_notice(
                "monitor-preset-notice-error",
                "监控预设操作失败",
                "预设未保存，请检查名称和本地存储状态。",
                cx.theme(),
            ))
            .child(settings_success_notice(
                "monitor-preset-notice-success",
                "监控预设已更新",
                "已保存当前监控状态，可继续应用或重命名。",
                cx.theme(),
            ))
    }
}

#[gpui_kit::test]
fn monitor_preset_feedback_notices_fit_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual_cx) = cx.add_window_view(|_, _| MonitorPresetStatusHost);
    let visual_cx: &mut VisualTestContext = visual_cx;

    for mode in [crate::Mode::Light, crate::Mode::Dark] {
        visual_cx.update(|_, app| crate::apply_theme(mode, app));
        for width in [360.0, 1024.0, 1440.0] {
            visual_cx.simulate_resize(size(px(width), px(300.0)));
            visual_cx.run_until_parked();

            let Some(host) = visual_cx.debug_bounds("monitor-preset-status-host") else {
                unreachable!("监控预设状态测试宿主应渲染");
            };
            for selector in [
                "monitor-preset-load-error",
                "monitor-preset-notice-error",
                "monitor-preset-notice-success",
            ] {
                let Some(notice) = visual_cx.debug_bounds(selector) else {
                    unreachable!("监控预设状态卡应渲染: {selector}");
                };
                assert!(notice.origin.x >= host.origin.x);
                assert!(notice.right() <= host.right());
                assert!(notice.origin.y >= host.origin.y);
                assert!(notice.bottom() <= host.bottom());
            }
        }
    }
}
