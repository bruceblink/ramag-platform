//! 剪贴板设置页摘要的 Headless GPUI 布局测试。

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, div, px, size,
};
use ramag_domain::entities::ClipboardSettings;

use super::render_clipboard_summary;
use crate::settings_view::pages::{
    settings_error_notice, settings_info_notice, settings_processing_notice, settings_saving_notice,
};

struct ClipboardSummaryHost {
    settings: ClipboardSettings,
}

impl Render for ClipboardSummaryHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-clip-summary-host")
            .debug_selector(|| "settings-clip-summary-host".into())
            .w_full()
            .child(render_clipboard_summary(&self.settings, cx))
    }
}

#[gpui_kit::test]
fn clipboard_summary_fits_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, cx) = cx.add_window_view(|_, _| ClipboardSummaryHost {
        settings: ClipboardSettings {
            enabled: true,
            capture_images: false,
            alternate_hotkey: true,
            ..ClipboardSettings::default()
        },
    });
    let cx: &mut VisualTestContext = cx;

    for width in [360.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(360.0)));
        cx.run_until_parked();

        let Some(host) = cx.debug_bounds("settings-clip-summary-host") else {
            unreachable!("剪贴板摘要测试宿主应渲染");
        };
        let Some(summary) = cx.debug_bounds("settings-clip-summary") else {
            unreachable!("剪贴板摘要卡应渲染");
        };
        let Some(metrics) = cx.debug_bounds("settings-clip-summary-metrics") else {
            unreachable!("剪贴板摘要指标区应渲染");
        };

        for child in [summary, metrics] {
            assert!(child.origin.x >= host.origin.x);
            assert!(child.origin.y >= host.origin.y);
            assert!(child.origin.x + child.size.width <= host.origin.x + host.size.width);
            assert!(child.origin.y + child.size.height <= host.origin.y + host.size.height);
        }

        for selector in [
            "settings-clip-summary-enabled",
            "settings-clip-summary-hotkey",
            "settings-clip-summary-images",
        ] {
            let Some(metric) = cx.debug_bounds(selector) else {
                unreachable!("剪贴板摘要指标应渲染: {selector}");
            };
            assert!(metric.origin.x >= metrics.origin.x);
            assert!(metric.origin.y >= metrics.origin.y);
            assert!(metric.origin.x + metric.size.width <= metrics.origin.x + metrics.size.width);
            assert!(metric.origin.y + metric.size.height <= metrics.origin.y + metrics.size.height);
        }
    }
}

struct SavingNoticeHost;

impl Render for SavingNoticeHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-saving-notice-host")
            .debug_selector(|| "settings-saving-notice-host".into())
            .w_full()
            .child(settings_saving_notice(
                "settings-clipboard-save-status",
                "剪贴板设置",
                cx.theme(),
            ))
    }
}

#[gpui_kit::test]
fn saving_notice_fits_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, cx) = cx.add_window_view(|_, _| SavingNoticeHost);
    let cx: &mut VisualTestContext = cx;

    for width in [360.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(180.0)));
        cx.run_until_parked();

        let Some(host) = cx.debug_bounds("settings-saving-notice-host") else {
            unreachable!("设置保存状态测试宿主应渲染");
        };
        let Some(notice) = cx.debug_bounds("settings-clipboard-save-status") else {
            unreachable!("设置保存状态卡应渲染");
        };
        assert!(notice.origin.x >= host.origin.x);
        assert!(notice.right() <= host.right());
        assert!(notice.origin.y >= host.origin.y);
        assert!(notice.bottom() <= host.bottom());
    }
}

struct ClipboardFeedbackHost;

impl Render for ClipboardFeedbackHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("settings-clipboard-feedback-host")
            .debug_selector(|| "settings-clipboard-feedback-host".into())
            .w_full()
            .gap(px(8.0))
            .child(settings_info_notice(
                "settings-clipboard-platform-status",
                "当前平台不可用",
                "当前平台未启用剪贴板工具。",
                cx.theme(),
            ))
            .child(settings_error_notice(
                "settings-clipboard-degraded-status",
                "设置读取异常",
                "采集已自动暂停；重新保存任一设置可尝试修复。",
                cx.theme(),
            ))
            .child(settings_processing_notice(
                "settings-clipboard-clear-status",
                "剪贴历史",
                "清空完成前不能再次操作历史数据。",
                cx.theme(),
            ))
    }
}

#[gpui_kit::test]
fn clipboard_feedback_notices_fit_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual_cx) = cx.add_window_view(|_, _| ClipboardFeedbackHost);
    let visual_cx: &mut VisualTestContext = visual_cx;

    for mode in [crate::Mode::Light, crate::Mode::Dark] {
        visual_cx.update(|_, app| crate::apply_theme(mode, app));
        for width in [360.0, 1024.0, 1440.0] {
            visual_cx.simulate_resize(size(px(width), px(300.0)));
            visual_cx.run_until_parked();

            let Some(host) = visual_cx.debug_bounds("settings-clipboard-feedback-host") else {
                unreachable!("剪贴板状态测试宿主应渲染");
            };
            for selector in [
                "settings-clipboard-platform-status",
                "settings-clipboard-degraded-status",
                "settings-clipboard-clear-status",
            ] {
                let Some(notice) = visual_cx.debug_bounds(selector) else {
                    unreachable!("剪贴板状态卡应渲染: {selector}");
                };
                assert!(notice.origin.x >= host.origin.x);
                assert!(notice.right() <= host.right());
                assert!(notice.origin.y >= host.origin.y);
                assert!(notice.bottom() <= host.bottom());
            }
        }
    }
}
