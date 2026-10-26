//! 数据库设置页状态卡的 Headless GPUI 布局测试。

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, px, size,
};

use super::render_database_converter_idle_state;
use crate::settings_view::pages::settings_processing_notice;

struct DatabaseStatusHost;

impl Render for DatabaseStatusHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("settings-db-status-host")
            .debug_selector(|| "settings-db-status-host".into())
            .w_full()
            .gap(px(8.0))
            .child(settings_processing_notice(
                "settings-db-transfer-status",
                "连接配置",
                "导入或导出完成前不能再次操作连接配置。",
                cx.theme(),
            ))
            .child(render_database_converter_idle_state(cx.theme()))
    }
}

#[gpui_kit::test]
fn database_status_cards_fit_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual_cx) = cx.add_window_view(|_, _| DatabaseStatusHost);
    let visual_cx: &mut VisualTestContext = visual_cx;

    for mode in [crate::Mode::Light, crate::Mode::Dark] {
        visual_cx.update(|_, app| crate::apply_theme(mode, app));
        for width in [360.0, 1024.0, 1440.0] {
            visual_cx.simulate_resize(size(px(width), px(300.0)));
            visual_cx.run_until_parked();

            let Some(host) = visual_cx.debug_bounds("settings-db-status-host") else {
                unreachable!("数据库状态测试宿主应渲染");
            };
            for selector in [
                "settings-db-transfer-status",
                "settings-db-id-converter-idle",
            ] {
                let Some(state) = visual_cx.debug_bounds(selector) else {
                    unreachable!("数据库状态卡应渲染: {selector}");
                };
                assert!(state.origin.x >= host.origin.x);
                assert!(state.right() <= host.right());
                assert!(state.origin.y >= host.origin.y);
                assert!(state.bottom() <= host.bottom());
            }

            let Some(idle) = visual_cx.debug_bounds("settings-db-id-converter-idle") else {
                unreachable!("转换测试空状态应渲染");
            };
            let Some(heading) = visual_cx.debug_bounds("settings-db-id-converter-idle-heading")
            else {
                unreachable!("转换测试空状态标题应渲染");
            };
            let Some(guidance) = visual_cx.debug_bounds("settings-db-id-converter-idle-guidance")
            else {
                unreachable!("转换测试空状态说明应渲染");
            };
            assert!(heading.origin.y >= idle.origin.y);
            assert!(guidance.origin.y >= heading.bottom());
        }
    }
}
