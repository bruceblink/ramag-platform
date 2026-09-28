use std::sync::Arc;

use gpui_kit::{
    AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement, Pixels, Render, Styled,
    TestAppContext, VisualTestContext, Window, div, px, size,
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
                format!("Home tool {index} / 测试较长的跨语言工具名称"),
                "A responsive home card description，用于检查长描述换行和卡片底部边界。",
            ),
        }));
    }
    registry
}

/// Verify that the first screen keeps its branding and tool cards inside a narrow main pane.
fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    let result = cx.debug_bounds(selector);
    assert!(result.is_some(), "首页元素必须渲染：{selector}");
    result.unwrap_or_default()
}

/// 验证长标题卡片、首页标题和滚动区域在不同窗口中保持统一边界。
#[gpui_kit::test]
fn home_view_scrolls_and_fits_compact_windows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_tools(16);
    let (host_entity, visual_cx) = cx.add_window_view(move |_window, cx| {
        let home = cx.new(|cx| HomeView::new(registry, cx));
        HomeTestHost { view: home }
    });
    let home_entity = host_entity.read_with(visual_cx, |host, _| host.view.clone());

    visual_cx.simulate_resize(size(px(360.0), px(260.0)));
    visual_cx.run_until_parked();

    let home = bounds(visual_cx, "home-view");
    let logo = bounds(visual_cx, "home-logo");
    let grid = bounds(visual_cx, "home-tool-grid");
    let first_card = bounds(visual_cx, "home-tool-home-test-0");

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
    for mode in [crate::Mode::Dark, crate::Mode::Light] {
        visual_cx.update(|_, app| crate::apply_theme(mode, app));
        for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
            visual_cx.simulate_resize(size(px(width), px(height)));
            visual_cx.run_until_parked();
            let header = bounds(visual_cx, "home-logo");
            let grid = bounds(visual_cx, "home-tool-grid");
            assert_eq!(header.origin.x, grid.origin.x, "标题与工具网格左对齐");
            assert_eq!(header.size.width, grid.size.width, "标题与工具网格宽度一致");
            for selector in [
                "home-tool-home-test-0",
                "home-tool-home-test-7",
                "home-tool-home-test-15",
            ] {
                let card = bounds(visual_cx, selector);
                assert!(card.origin.x >= grid.origin.x && card.right() <= grid.right());
                assert_eq!(card.size.height, px(super::TOOL_CARD_HEIGHT));
            }
        }
    }
}
