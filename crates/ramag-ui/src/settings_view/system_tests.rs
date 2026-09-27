//! 公共设置的真实 GPUI 渲染与点击测试，验证最大字号与设置作用域。

use super::*;
use gpui_kit::component::{Root, scroll::ScrollableElement as _};
use gpui_kit::{Modifiers, MouseButton, Render, TestAppContext, VisualTestContext, px, size};

struct SettingsPanelHost;

impl Render for SettingsPanelHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-panel-host")
            .debug_selector(|| "settings-panel-host".into())
            .size_full()
            .p_4()
            .overflow_y_scrollbar()
            .child(system_settings_panel(
                crate::system_settings(cx),
                current_mode(cx),
                cx.theme(),
            ))
    }
}

/// 用最新渲染边界点击，确保检查的是控件事件而非直接修改模型。
fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx.debug_bounds(selector);
    assert!(bounds.is_some(), "missing control: {selector}");
    let Some(bounds) = bounds else { return };
    let center = bounds.center();
    cx.simulate_mouse_move(center, None, Modifiers::default());
    cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(center, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
}

#[gpui_kit::test]
fn public_settings_fit_both_themes_and_apply_without_resetting_other_values(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::component::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|_| SettingsPanelHost);
        Root::new(view, window, cx)
    });
    for mode in [Mode::Light, Mode::Dark] {
        cx.update(|_, app| {
            crate::set_system_settings(
                SystemSettings {
                    text_size: InterfaceTextSize::Large,
                    ..Default::default()
                },
                app,
            );
            crate::apply_theme(mode, app);
        });
        for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
            cx.simulate_resize(size(px(width), px(height)));
            cx.run_until_parked();
            for selector in [
                "settings-theme-row",
                "settings-text-size-row",
                "settings-scrollbar-row",
                "settings-tray-row",
                "settings-scrollbar-2",
            ] {
                let bounds = cx.debug_bounds(selector);
                assert!(bounds.is_some(), "missing {selector}");
                let bounds = bounds.unwrap_or_default();
                assert!(
                    bounds.left() >= px(0.0) && bounds.right() <= px(width),
                    "{selector}: {bounds:?}"
                );
            }
        }
    }
    click(cx, "settings-text-size-0");
    click(cx, "settings-scrollbar-1");
    click(cx, "settings-theme-0");
    cx.update(|_, app| {
        assert_eq!(
            crate::system_settings(app).text_size,
            InterfaceTextSize::Compact
        );
        assert_eq!(
            crate::system_settings(app).scrollbar_visibility,
            ScrollbarVisibility::Hover
        );
        assert_eq!(current_mode(app), Mode::Light);
        assert_eq!(Theme::global(app).font_size, px(14.0));
        assert_eq!(
            Theme::global(app).scrollbar_mode,
            gpui_kit::component::scroll::ScrollbarMode::Hover
        );
    });
}
