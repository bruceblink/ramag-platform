use std::sync::Arc;

use gpui_kit::{
    AppContext as _, Bounds, Context, Entity, IntoElement, Modifiers, MouseButton, ParentElement,
    Pixels, Render, Styled, Subscription, TestAppContext, VisualTestContext, Window, div, point,
    px, size,
};
use ramag_app::ToolRegistry;
use ramag_domain::{Tool, ToolMeta};

use super::{HomeEvent, HomeView};

/// Keep the tested view and its open events alive without navigating away from the grid.
struct HomeTestHost {
    view: Entity<HomeView>,
    opened: Vec<String>,
    _subscription: Subscription,
}

impl HomeTestHost {
    /// Subscribe to the production card event so hit testing verifies the requested tool ID.
    fn new(registry: Arc<ToolRegistry>, cx: &mut Context<Self>) -> Self {
        let view = cx.new(|cx| HomeView::new(registry, cx));
        let subscription = cx.subscribe(&view, |host, _, event, _| match event {
            HomeEvent::OpenTool(id) => host.opened.push(id.clone()),
        });
        Self {
            view,
            opened: Vec::new(),
            _subscription: subscription,
        }
    }
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
    let (host_entity, visual_cx) =
        cx.add_window_view(move |_window, cx| HomeTestHost::new(registry, cx));
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
            for (selector, title_selector, description_selector) in [
                (
                    "home-tool-home-test-0",
                    "home-tool-title-home-test-0",
                    "home-tool-description-home-test-0",
                ),
                (
                    "home-tool-home-test-7",
                    "home-tool-title-home-test-7",
                    "home-tool-description-home-test-7",
                ),
                (
                    "home-tool-home-test-15",
                    "home-tool-title-home-test-15",
                    "home-tool-description-home-test-15",
                ),
            ] {
                let card = bounds(visual_cx, selector);
                assert!(card.origin.x >= grid.origin.x && card.right() <= grid.right());
                assert_eq!(card.size.height, px(super::TOOL_CARD_HEIGHT));
                let title = bounds(visual_cx, title_selector);
                let description = bounds(visual_cx, description_selector);
                assert!(title.origin.x >= card.origin.x && title.right() <= card.right());
                assert!(title.bottom() <= description.origin.y);
                assert!(description.right() <= card.right());
                assert!(description.bottom() <= card.bottom());
            }
        }
    }
}

/// Exercise headless GPUI hit testing and drag release using the same geometry as the cards.
#[gpui_kit::test]
fn home_cards_open_and_reorder_in_compact_windows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    // Render the completed animation frame so the final slot checks are deterministic.
    cx.update(|cx| cx.set_reduce_motion(true));
    let registry = registry_with_tools(3);
    let registry_for_view = registry.clone();
    let (host, visual) = cx.add_window_view(move |_, cx| HomeTestHost::new(registry_for_view, cx));
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    let first = bounds(visual, "home-tool-home-test-0");
    visual.simulate_click(first.center(), Modifiers::default());
    visual.run_until_parked();
    assert_eq!(
        host.read_with(visual, |host, _| host.opened.clone()),
        ["home-test-0"]
    );

    visual.simulate_mouse_down(first.center(), MouseButton::Left, Modifiers::default());
    visual.simulate_mouse_move(
        first.center() + point(px(0.0), px(12.0)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    visual.run_until_parked();
    let last = bounds(visual, "home-tool-home-test-2");
    let destination = point(last.center().x, last.bottom() - px(4.0));
    visual.simulate_mouse_move(destination, Some(MouseButton::Left), Modifiers::default());
    visual.run_until_parked();
    visual.simulate_mouse_up(destination, MouseButton::Left, Modifiers::default());
    visual.run_until_parked();

    assert_eq!(
        registry.order(),
        ["home-test-1", "home-test-2", "home-test-0"]
    );
    let moved = bounds(visual, "home-tool-home-test-0");
    let middle = bounds(visual, "home-tool-home-test-2");
    assert!(
        moved.origin.y >= middle.bottom(),
        "reordered card should follow the previous last card: moved={moved:?}, middle={middle:?}"
    );
    assert_eq!(moved.size.height, px(super::TOOL_CARD_HEIGHT));
    assert_eq!(host.read_with(visual, |host, _| host.opened.len()), 1);
}

/// Empty registries retain the page heading and a visible, bounded unavailable notice.
#[gpui_kit::test]
fn home_empty_state_fits_all_acceptance_sizes(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual) =
        cx.add_window_view(move |_, cx| HomeTestHost::new(registry_with_tools(0), cx));
    for mode in [crate::Mode::Dark, crate::Mode::Light] {
        visual.update(|_, app| crate::apply_theme(mode, app));
        for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
            visual.simulate_resize(size(px(width), px(height)));
            visual.run_until_parked();
            let page = bounds(visual, "home-view");
            let title = bounds(visual, "home-logo");
            let notice = bounds(visual, "pulse-status-notice");
            assert!(notice.origin.x >= page.origin.x && notice.right() <= page.right());
            assert!(notice.origin.y >= title.bottom() && notice.bottom() <= page.bottom());
        }
    }
}
