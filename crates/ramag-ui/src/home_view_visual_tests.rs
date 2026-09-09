use std::sync::Arc;

use gpui::{
    AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled, TestAppContext,
    Window, div, px, size,
};
use ramag_app::ToolRegistry;
use ramag_domain::{Tool, ToolMeta};

use super::HomeView;

struct HomeTestHost {
    view: Entity<HomeView>,
}

impl Render for HomeTestHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().relative().size_full().child(self.view.clone())
    }
}

struct TestTool {
    meta: ToolMeta,
}

impl Tool for TestTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

fn registry_with_tools(count: usize) -> Arc<ToolRegistry> {
    let registry = Arc::new(ToolRegistry::new());
    for index in 0..count {
        registry.register(Arc::new(TestTool {
            meta: ToolMeta::new(
                format!("home-test-{index}"),
                format!("Home tool {index}"),
                "A responsive home card description",
            ),
        }));
    }
    registry
}

/// Verify that the first screen keeps its branding and tool cards inside a narrow main pane.
#[gpui::test]
fn home_view_scrolls_and_fits_compact_windows(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let registry = registry_with_tools(16);
    let (host_entity, visual_cx) = cx.add_window_view(move |_window, cx| {
        let home = cx.new(|cx| HomeView::new(registry, cx));
        HomeTestHost { view: home }
    });
    let home_entity = host_entity.read_with(visual_cx, |host, _| host.view.clone());

    visual_cx.simulate_resize(size(px(360.0), px(260.0)));
    visual_cx.run_until_parked();

    let Some(home) = visual_cx.debug_bounds("home-view") else {
        return;
    };
    let Some(logo) = visual_cx.debug_bounds("home-logo") else {
        return;
    };
    let Some(grid) = visual_cx.debug_bounds("home-tool-grid") else {
        return;
    };
    let Some(first_card) = visual_cx.debug_bounds("home-tool-home-test-0") else {
        return;
    };

    assert!(
        logo.right() <= home.right(),
        "首页 Logo 不能横向溢出: {logo:?}"
    );
    assert!(
        grid.right() <= home.right(),
        "首页网格不能横向溢出: {grid:?}"
    );
    assert!(
        first_card.right() <= grid.right(),
        "首页卡片不能横向溢出网格: {first_card:?} / {grid:?}"
    );

    let max_offset = home_entity.read_with(visual_cx, |view, _| view.scroll.max_offset());
    assert!(
        max_offset.y > px(0.0),
        "低高度首页应提供纵向滚动范围: {max_offset:?}"
    );
}
