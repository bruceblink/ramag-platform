//! 最近项目选择器空状态的 Headless GPUI 布局测试。

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, div, px, size,
};

use super::render_empty_section_state;

struct EmptySectionHost {
    favorite_section: bool,
    query_empty: bool,
}

impl Render for EmptySectionHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("recent-picker-empty-host")
            .debug_selector(|| "recent-picker-empty-host".into())
            .w_full()
            .child(render_empty_section_state(
                self.favorite_section,
                self.query_empty,
                cx.theme(),
            ))
    }
}

#[gpui_kit::test]
fn recent_picker_empty_states_fit_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (host, cx) = cx.add_window_view(|_, _| EmptySectionHost {
        favorite_section: false,
        query_empty: false,
    });
    let cx: &mut VisualTestContext = cx;

    for (favorite_section, query_empty, section) in [
        (true, true, "favorites"),
        (false, true, "all"),
        (false, false, "all"),
    ] {
        let (state_selector, heading_selector, guidance_selector) = if section == "favorites" {
            (
                "recent-picker-empty-state-favorites",
                "recent-picker-empty-heading-favorites",
                "recent-picker-empty-guidance-favorites",
            )
        } else {
            (
                "recent-picker-empty-state-all",
                "recent-picker-empty-heading-all",
                "recent-picker-empty-guidance-all",
            )
        };
        host.update(cx, |host, cx| {
            host.favorite_section = favorite_section;
            host.query_empty = query_empty;
            cx.notify();
        });
        for width in [360.0, 640.0, 1024.0, 1440.0] {
            cx.simulate_resize(size(px(width), px(220.0)));
            cx.run_until_parked();

            let Some(host_bounds) = cx.debug_bounds("recent-picker-empty-host") else {
                unreachable!("最近项目空状态宿主应渲染");
            };
            let Some(state) = cx.debug_bounds(state_selector) else {
                unreachable!("最近项目空状态应渲染");
            };
            let Some(heading) = cx.debug_bounds(heading_selector) else {
                unreachable!("最近项目空状态标题应渲染");
            };
            let Some(guidance) = cx.debug_bounds(guidance_selector) else {
                unreachable!("最近项目空状态引导应渲染");
            };

            for child in [state, heading, guidance] {
                assert!(child.origin.x >= host_bounds.origin.x);
                assert!(child.origin.y >= host_bounds.origin.y);
                assert!(
                    child.origin.x + child.size.width
                        <= host_bounds.origin.x + host_bounds.size.width
                );
                assert!(
                    child.origin.y + child.size.height
                        <= host_bounds.origin.y + host_bounds.size.height
                );
            }
            assert!(guidance.origin.y > heading.origin.y);
        }
    }
}
