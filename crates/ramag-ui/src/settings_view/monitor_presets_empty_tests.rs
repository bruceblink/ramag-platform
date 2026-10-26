//! 系统监控命名预设空状态的 Headless GPUI 布局测试。

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, div, px, size,
};

use super::render_empty_monitor_preset_state;

struct EmptyPresetHost;

impl Render for EmptyPresetHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("monitor-preset-empty-host")
            .debug_selector(|| "monitor-preset-empty-host".into())
            .w_full()
            .child(render_empty_monitor_preset_state(cx.theme()))
    }
}

#[gpui_kit::test]
fn empty_monitor_preset_state_fits_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, cx) = cx.add_window_view(|_, _| EmptyPresetHost);
    let cx: &mut VisualTestContext = cx;

    for width in [360.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(220.0)));
        cx.run_until_parked();

        let Some(host) = cx.debug_bounds("monitor-preset-empty-host") else {
            unreachable!("监控预设空状态宿主应渲染");
        };
        let Some(state) = cx.debug_bounds("monitor-preset-empty-state") else {
            unreachable!("监控预设空状态应渲染");
        };
        let Some(heading) = cx.debug_bounds("monitor-preset-empty-heading") else {
            unreachable!("监控预设空状态标题应渲染");
        };
        let Some(guidance) = cx.debug_bounds("monitor-preset-empty-guidance") else {
            unreachable!("监控预设空状态引导应渲染");
        };

        for child in [state, heading, guidance] {
            assert!(child.origin.x >= host.origin.x);
            assert!(child.origin.y >= host.origin.y);
            assert!(child.origin.x + child.size.width <= host.origin.x + host.size.width);
            assert!(child.origin.y + child.size.height <= host.origin.y + host.size.height);
        }
        assert!(guidance.origin.y > heading.origin.y);
    }
}
