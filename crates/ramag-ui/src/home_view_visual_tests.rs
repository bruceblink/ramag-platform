use std::sync::Arc;

use gpui_kit::test::TestQueryExt as _;
use gpui_kit::{
    AppContext as _, Bounds, Context, ElementId, Entity, IntoElement, Modifiers, MouseButton,
    ParentElement, Pixels, Render, Styled, Subscription, TestAppContext, VisualTestContext, Window,
    div, point, px, size,
};
use ramag_app::ToolRegistry;
use ramag_domain::{Tool, ToolMeta};

use super::{HomeEvent, HomeView};

/// Keep the tested view and its open events alive without navigating away from the grid.
pub(super) struct HomeTestHost {
    view: Entity<HomeView>,
    opened: Vec<String>,
    _subscription: Subscription,
}

impl HomeTestHost {
    /// Subscribe to the production card event so hit testing verifies the requested tool ID.
    pub(super) fn new(registry: Arc<ToolRegistry>, cx: &mut Context<Self>) -> Self {
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

pub(super) fn registry_with_catalog_tools() -> Arc<ToolRegistry> {
    let registry = Arc::new(ToolRegistry::new());
    for (id, name, description) in [
        ("dbclient", "PostgreSQL Client", "Inspect the local server"),
        ("api", "HTTP Workbench", "Send request samples"),
        (
            "plugin.customer-tool",
            "Customer Plugin",
            "Browse external records",
        ),
    ] {
        registry.register(Arc::new(TestTool {
            meta: ToolMeta::new(id, name, description),
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

/// 按可见控件的实际边界执行点击，确保测试走真实的命中流程。
pub(super) fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let target = bounds(cx, selector);
    cx.simulate_click(target.center(), Modifiers::default());
    cx.run_until_parked();
}

/// 按辅助功能名称点击弹出菜单项，验证读屏名称也能定位真实选项。
pub(super) fn click_accessible_label(visual: &mut VisualTestContext, label: &str) {
    let target = visual.update(|window, _| {
        let matches = window.find_all_by_label(label);
        assert_eq!(matches.len(), 1, "菜单项名称应唯一且可发现：{label}");
        matches
            .first()
            .map(|element| element.bounds())
            .unwrap_or_default()
    });
    visual.simulate_click(target.center(), Modifiers::default());
    visual.run_until_parked();
}

/// 更新共享显示偏好并等待真实 GPUI 窗口按新状态重绘。
fn set_home_display_preferences(
    visual: &mut VisualTestContext,
    density: crate::ContentDensity,
    text_size: crate::InterfaceTextSize,
) {
    visual.update(|_, app| {
        let mut settings = crate::system_settings(app);
        settings.content_density = density;
        settings.text_size = text_size;
        crate::set_system_settings(settings, app);
    });
    visual.run_until_parked();
}

/// 读取卡片的辅助名称，确认密度收缩没有隐藏完整工具说明。
fn accessible_name(visual: &mut VisualTestContext, id: &'static str) -> Option<String> {
    let id = ElementId::from(id);
    visual.update(|window, _| {
        gpui_kit::base::test_support::find(window, &[], &id)
            .and_then(|snapshot| snapshot.label().map(str::to_owned))
    })
}

/// 使用屏幕实际行边界拖动工具，避免测试依赖固定窗口坐标。
fn drag_home_tool_to_bottom(
    visual: &mut VisualTestContext,
    source: &'static str,
    target: &'static str,
) {
    let source = bounds(visual, source);
    let start = source.center();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    visual.simulate_mouse_move(
        start + point(px(0.0), px(12.0)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    visual.run_until_parked();
    let target = bounds(visual, target);
    let destination = point(target.center().x, target.bottom() - px(4.0));
    visual.simulate_mouse_move(destination, Some(MouseButton::Left), Modifiers::default());
    visual.run_until_parked();
    visual.simulate_mouse_up(destination, MouseButton::Left, Modifiers::default());
    visual.run_until_parked();
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
        for (width, height) in [
            (360.0, 640.0),
            (1024.0, 768.0),
            (1440.0, 900.0),
            (1689.0, 900.0),
            (2560.0, 1392.0),
        ] {
            visual_cx.simulate_resize(size(px(width), px(height)));
            visual_cx.run_until_parked();
            let header = bounds(visual_cx, "home-logo");
            let grid = bounds(visual_cx, "home-tool-grid");
            assert_eq!(header.origin.x, grid.origin.x, "标题与工具网格左对齐");
            assert_eq!(header.size.width, grid.size.width, "标题与工具网格宽度一致");
            let first_card = bounds(visual_cx, "home-tool-home-test-0");
            let fourth_card = bounds(visual_cx, "home-tool-home-test-3");
            if super::grid_columns(width) >= 4 {
                assert_eq!(first_card.origin.y, fourth_card.origin.y);
                assert!(fourth_card.origin.x > first_card.origin.x);
            } else {
                assert!(fourth_card.origin.y > first_card.origin.y);
            }
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
                let card_width = f32::from(card.size.width);
                let card_height = f32::from(card.size.height);
                assert!((card_width / card_height - super::GOLDEN_RATIO).abs() < 0.01);
                let title = bounds(visual_cx, title_selector);
                let description = bounds(visual_cx, description_selector);
                assert!(title.origin.x >= card.origin.x && title.right() <= card.right());
                assert!(title.bottom() <= description.origin.y);
                assert!(description.right() <= card.right());
                assert!(description.bottom() <= card.bottom());
                assert!(
                    description.size.height > px(30.0),
                    "卡片应为工具描述保留至少两行空间: {description:?} / {card:?}"
                );
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
    let moved_width = f32::from(moved.size.width);
    let moved_height = f32::from(moved.size.height);
    assert!((moved_width / moved_height - super::GOLDEN_RATIO).abs() < 0.01);
    assert_eq!(host.read_with(visual, |host, _| host.opened.len()), 1);
}

/// 搜索名称、说明和 ID；无结果操作可同时恢复搜索与分类。
#[gpui_kit::test]
fn home_search_matches_name_description_and_id_and_clears_empty_state(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_catalog_tools();
    let registry_for_view = registry.clone();
    let (_, visual) = cx.add_window_view(move |_, cx| HomeTestHost::new(registry_for_view, cx));
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();

    click(visual, "home-search");
    visual.simulate_keystrokes("postgresql");
    visual.run_until_parked();
    assert!(visual.debug_bounds("home-tool-dbclient").is_some());
    assert!(visual.debug_bounds("home-tool-api").is_none());
    assert!(visual.debug_bounds("home-filter-sort-hint").is_some());
    assert!(visual.debug_bounds("home-tool-manager").is_some());
    let before_filtered_drag = registry.order();
    let matching_tool = bounds(visual, "home-tool-dbclient");
    let drag_start = matching_tool.center();
    let drag_end = drag_start + point(px(0.0), px(48.0));
    visual.simulate_mouse_down(drag_start, MouseButton::Left, Modifiers::default());
    visual.simulate_mouse_move(drag_end, Some(MouseButton::Left), Modifiers::default());
    visual.run_until_parked();
    visual.simulate_mouse_up(drag_end, MouseButton::Left, Modifiers::default());
    visual.run_until_parked();
    assert_eq!(registry.order(), before_filtered_drag);
    click(visual, "home-search-clear");

    click(visual, "home-search");
    visual.simulate_keystrokes("server");
    visual.run_until_parked();
    assert!(visual.debug_bounds("home-tool-dbclient").is_some());
    assert!(visual.debug_bounds("home-tool-api").is_none());
    click(visual, "home-search-clear");

    click(visual, "home-search");
    visual.simulate_keystrokes("DBCLIENT");
    visual.run_until_parked();
    assert!(visual.debug_bounds("home-tool-dbclient").is_some());
    assert!(visual.debug_bounds("home-tool-api").is_none());

    click(visual, "home-search-clear");
    click(visual, "home-category");
    click_accessible_label(visual, "数据");
    click(visual, "home-search");
    visual.simulate_keystrokes("nothing");
    visual.run_until_parked();
    assert!(visual.debug_bounds("home-filter-empty").is_some());
    assert!(visual.debug_bounds("home-tool-dbclient").is_none());
    click(visual, "home-filter-clear");
    assert!(visual.debug_bounds("home-tool-dbclient").is_some());
    assert!(visual.debug_bounds("home-tool-api").is_some());
    assert!(
        visual
            .debug_bounds("home-tool-plugin.customer-tool")
            .is_some()
    );
}

/// 分类把内置工具分组，并确保未知插件仍能从“其他”类别进入。
#[gpui_kit::test]
fn home_category_filters_builtin_and_unknown_tools(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual) =
        cx.add_window_view(move |_, cx| HomeTestHost::new(registry_with_catalog_tools(), cx));
    visual.simulate_resize(size(px(720.0), px(640.0)));
    visual.run_until_parked();

    click(visual, "home-category");
    assert_eq!(
        visual.update(|window, _| window.find_all_by_label("数据").len()),
        1,
        "数据分类应通过辅助功能名称发现"
    );
    click_accessible_label(visual, "数据");
    assert!(visual.debug_bounds("home-tool-dbclient").is_some());
    assert!(visual.debug_bounds("home-tool-api").is_none());
    assert!(
        visual
            .debug_bounds("home-tool-plugin.customer-tool")
            .is_none()
    );

    click(visual, "home-category");
    click_accessible_label(visual, "其他");
    assert!(
        visual
            .debug_bounds("home-tool-plugin.customer-tool")
            .is_some()
    );
    assert!(visual.debug_bounds("home-tool-dbclient").is_none());
}

/// 列表视图保留行拖拽、入口打开和窄窗口双主题的内容边界。
#[gpui_kit::test]
fn home_list_rows_open_and_reorder_without_clipping(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    cx.update(|cx| cx.set_reduce_motion(true));
    let registry = registry_with_tools(3);
    let registry_for_view = registry.clone();
    let (host, visual) = cx.add_window_view(move |_, cx| HomeTestHost::new(registry_for_view, cx));

    for mode in [crate::Mode::Dark, crate::Mode::Light] {
        visual.update(|_, app| crate::apply_theme(mode, app));
        visual.simulate_resize(size(px(360.0), px(640.0)));
        visual.run_until_parked();
        click(visual, "home-layout-list");
        assert!(visual.debug_bounds("home-layout-grid").is_some());
        let home = bounds(visual, "home-view");
        let list = bounds(visual, "home-tool-grid");
        let first = bounds(visual, "home-tool-home-test-0");
        let second = bounds(visual, "home-tool-home-test-1");
        let third = bounds(visual, "home-tool-home-test-2");
        assert_eq!(f32::from(first.size.height), super::HOME_LIST_ROW_HEIGHT);
        assert_eq!(first.origin.x, list.origin.x);
        assert_eq!(first.size.width, list.size.width);
        assert_eq!(
            second.origin.y - first.origin.y,
            px(super::HOME_LIST_ROW_HEIGHT)
        );
        assert_eq!(
            third.origin.y - second.origin.y,
            px(super::HOME_LIST_ROW_HEIGHT)
        );
        for selector in [
            "home-tool-title-home-test-0",
            "home-tool-description-home-test-0",
        ] {
            let content = bounds(visual, selector);
            assert!(
                content.left() >= first.left() && content.right() <= first.right(),
                "{selector} must fit horizontally: {content:?} / {first:?}"
            );
            assert!(
                content.top() + px(4.0) >= first.top()
                    && content.bottom() <= first.bottom() + px(4.0),
                "{selector} must fit vertically: {content:?} / {first:?}"
            );
        }
        assert!(first.right() <= home.right());
        click(visual, "home-layout-grid");
    }

    click(visual, "home-layout-list");
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
}

/// 共享密度立即调整网格卡片；大字号保持可读说明，网格和列表拖拽继续按实际尺寸排序。
#[gpui_kit::test]
fn home_shared_density_updates_grid_and_preserves_drag_geometry(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    cx.update(|cx| cx.set_reduce_motion(true));
    let registry = registry_with_tools(4);
    let registry_for_view = registry.clone();
    let (_, visual) = cx.add_window_view(move |_, cx| HomeTestHost::new(registry_for_view, cx));
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    set_home_display_preferences(
        visual,
        crate::ContentDensity::Standard,
        crate::InterfaceTextSize::Large,
    );

    let standard_card = bounds(visual, "home-tool-home-test-0");
    let standard_icon = bounds(visual, "home-tool-icon-home-test-0");
    assert!((f32::from(standard_card.size.height) - 220.0 / super::GOLDEN_RATIO).abs() < 0.1);
    assert_eq!(
        accessible_name(visual, "home-tool-home-test-0").as_deref(),
        Some(
            "Home tool 0 / 测试较长的跨语言工具名称 — A responsive home card description，用于检查长描述换行和卡片底部边界。"
        )
    );

    set_home_display_preferences(
        visual,
        crate::ContentDensity::Compact,
        crate::InterfaceTextSize::Large,
    );
    let compact_card = bounds(visual, "home-tool-home-test-0");
    let compact_icon = bounds(visual, "home-tool-icon-home-test-0");
    assert!(compact_card.size.height < standard_card.size.height);
    assert!(
        (f32::from(compact_card.size.height) - (93.0 * 18.0 / 16.0)).abs() < 0.2,
        "较大字号应放大紧凑卡片高度以保留标题和正文空间: {compact_card:?}"
    );
    assert_eq!(
        standard_icon.left() - standard_card.left(),
        compact_icon.left() - compact_card.left() + px(3.0),
        "紧凑档应把卡片内边距从 15px 收到 12px"
    );
    for selector in [
        "home-tool-title-home-test-0",
        "home-tool-description-home-test-0",
    ] {
        let content = bounds(visual, selector);
        assert!(content.left() >= compact_card.left());
        assert!(content.right() <= compact_card.right());
        assert!(content.top() + px(4.0) >= compact_card.top());
        assert!(
            content.bottom() <= compact_card.bottom() + px(4.0),
            "{selector} should stay inside the compact card: {content:?} / {compact_card:?}"
        );
    }
    assert_eq!(
        accessible_name(visual, "home-tool-home-test-0").as_deref(),
        Some(
            "Home tool 0 / 测试较长的跨语言工具名称 — A responsive home card description，用于检查长描述换行和卡片底部边界。"
        )
    );

    drag_home_tool_to_bottom(visual, "home-tool-home-test-0", "home-tool-home-test-2");
    assert_eq!(
        registry.order(),
        ["home-test-1", "home-test-2", "home-test-0", "home-test-3"]
    );

    click(visual, "home-layout-list");
    let first_row = bounds(visual, "home-tool-home-test-1");
    let second_row = bounds(visual, "home-tool-home-test-2");
    let last_row = bounds(visual, "home-tool-home-test-3");
    assert_eq!(
        f32::from(first_row.size.height),
        super::HOME_LIST_ROW_HEIGHT
    );
    assert_eq!(
        second_row.origin.y - first_row.origin.y,
        px(super::HOME_LIST_ROW_HEIGHT)
    );
    assert_eq!(
        last_row.origin.y - first_row.origin.y,
        px(super::HOME_LIST_ROW_HEIGHT * 3.0)
    );
    drag_home_tool_to_bottom(visual, "home-tool-home-test-1", "home-tool-home-test-3");
    assert_eq!(
        registry.order(),
        ["home-test-2", "home-test-0", "home-test-3", "home-test-1"]
    );

    set_home_display_preferences(
        visual,
        crate::ContentDensity::Standard,
        crate::InterfaceTextSize::Large,
    );
    assert_eq!(
        f32::from(bounds(visual, "home-tool-home-test-1").size.height),
        super::HOME_LIST_ROW_HEIGHT,
        "共享密度不改变原型规定的列表行高"
    );
    click(visual, "home-layout-grid");
    let standard_card = bounds(visual, "home-tool-home-test-2");
    assert!((f32::from(standard_card.size.height) - 220.0 / super::GOLDEN_RATIO).abs() < 0.1);
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
