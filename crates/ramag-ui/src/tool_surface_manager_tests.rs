//! Headless interaction checks for the searchable tool placement manager.

use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};

use gpui_kit::component::{Root, Sizable as _, h_flex};
use gpui_kit::{
    AppContext as _, Bounds, Context, Entity, InteractiveElement as _, IntoElement, Modifiers,
    ParentElement as _, Pixels, Point, Render, Styled as _, Subscription, TestAppContext,
    VisualTestContext, Window, px, size,
};
use ramag_app::{StaticPluginHost, ToolRegistry};
use ramag_domain::{Tool, ToolMeta};

use crate::tool_layout::ToolDragSurface;
use crate::tool_surface_manager::{create_tool_surface_manager, tool_surface_manager_popover};
use crate::{ActivityBar, HomeEvent, HomeView, NavEvent, NavTarget};

struct ToolSurfaceTestHost {
    activity_bar: Entity<ActivityBar>,
    home: Entity<HomeView>,
    opened_home_tools: Vec<String>,
    sidebar_navigation: Vec<NavTarget>,
    _subscriptions: Vec<Subscription>,
}

impl ToolSurfaceTestHost {
    fn new(registry: Arc<ToolRegistry>, cx: &mut Context<Self>) -> Self {
        let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
        let activity_bar = cx.new(|cx| ActivityBar::new(registry.clone(), plugin_host, cx));
        let home = cx.new(|cx| HomeView::new(registry, cx));
        let subscriptions = vec![
            cx.subscribe(&home, |host, _, event, _| {
                let HomeEvent::OpenTool(id) = event;
                host.opened_home_tools.push(id.clone());
            }),
            cx.subscribe(&activity_bar, |host, _, event, _| {
                let NavEvent::Navigate(target) = event;
                host.sidebar_navigation.push(target.clone());
            }),
        ];
        Self {
            activity_bar,
            home,
            opened_home_tools: Vec::new(),
            sidebar_navigation: Vec::new(),
            _subscriptions: subscriptions,
        }
    }
}

impl Render for ToolSurfaceTestHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .child(self.activity_bar.clone())
            .child(self.home.clone())
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

fn registry_with_hidden_tools() -> Arc<ToolRegistry> {
    let registry = Arc::new(ToolRegistry::new());
    for id in ["first", "second"] {
        registry.register(Arc::new(TestTool {
            meta: ToolMeta::new(id, format!("Tool {id}"), "A searchable test tool."),
        }));
        registry.set_home_visible(id, false);
        registry.set_sidebar_visible(id, false);
    }
    registry
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let target = bounds(cx, selector);
    simulate_click(cx, target.center());
}

fn click_surface_control(cx: &mut VisualTestContext, selector: &'static str) {
    let target = bounds(cx, selector);
    let mut point = target.center();
    point.x = target.origin.x + target.size.width * 0.82;
    simulate_click(cx, point);
}

fn simulate_click(cx: &mut VisualTestContext, point: Point<Pixels>) {
    cx.simulate_mouse_move(point, None, Modifiers::default());
    cx.simulate_mouse_down(point, gpui_kit::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(point, gpui_kit::MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    let result = cx.debug_bounds(selector);
    assert!(
        result.is_some(),
        "missing tool placement control: {selector}"
    );
    result.unwrap_or_default()
}

fn assert_fits(cx: &mut VisualTestContext, selector: &'static str, width: f32, height: f32) {
    let item = bounds(cx, selector);
    assert!(
        item.origin.x >= px(0.0)
            && item.origin.y >= px(0.0)
            && item.right() <= px(width)
            && item.bottom() <= px(height),
        "{selector} must fit {width}x{height}: {item:?}"
    );
}

/// One searchable catalog row exposes the current state for both surfaces.
#[gpui_kit::test]
fn manager_search_lists_tools_once_with_both_surface_controls(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_hidden_tools();
    registry.set_home_visible("first", true);
    registry.set_sidebar_visible("second", true);
    let registry_for_manager = registry.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let manager =
            create_tool_surface_manager(registry_for_manager, ToolDragSurface::Home, window, cx);
        Root::new(cx.new(|_| ManagerTestHost { manager }), window, cx)
    });
    visual.simulate_resize(size(px(700.0), px(520.0)));
    visual.run_until_parked();
    click(visual, "manager-test-trigger");

    assert!(visual.debug_bounds("tool-manager-title").is_some());
    for selector in [
        "tool-manager-row-first",
        "tool-manager-home-control-first",
        "tool-manager-sidebar-control-first",
        "tool-manager-row-second",
        "tool-manager-home-control-second",
        "tool-manager-sidebar-control-second",
    ] {
        assert!(
            visual.debug_bounds(selector).is_some(),
            "missing {selector}"
        );
    }
    assert!(
        visual
            .debug_bounds("tool-manager-added-section-home")
            .is_none(),
        "the catalog must not split the same tool across added and available groups"
    );

    click(visual, "tool-manager-search");
    visual.simulate_keystrokes("second");
    visual.run_until_parked();
    assert!(visual.debug_bounds("tool-manager-row-first").is_none());
    assert!(visual.debug_bounds("tool-manager-row-second").is_some());
    assert!(
        visual
            .debug_bounds("tool-manager-home-control-second")
            .is_some()
    );
    assert!(
        visual
            .debug_bounds("tool-manager-sidebar-control-second")
            .is_some()
    );
}

/// The sidebar manager and its switches stay inside a compact window.
#[gpui_kit::test]
fn sidebar_manager_fits_compact_window(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_hidden_tools();
    let registry_for_view = registry.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let host = cx.new(|cx| ToolSurfaceTestHost::new(registry_for_view, cx));
        Root::new(host, window, cx)
    });
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();

    click(visual, "activity-tool-manager");
    for selector in [
        "tool-manager-body",
        "tool-manager-search",
        "tool-manager-scroll",
        "tool-manager-home-control-first",
        "tool-manager-sidebar-control-first",
    ] {
        assert_fits(visual, selector, 360.0, 640.0);
    }
}

#[gpui_kit::test]
fn manager_reports_when_no_tools_are_registered(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = Arc::new(ToolRegistry::new());
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let manager = create_tool_surface_manager(registry, ToolDragSurface::Home, window, cx);
        Root::new(cx.new(|_| ManagerTestHost { manager }), window, cx)
    });
    visual.simulate_resize(size(px(700.0), px(520.0)));
    visual.run_until_parked();
    click(visual, "manager-test-trigger");

    let empty = bounds(visual, "tool-manager-empty");
    let heading = bounds(visual, "tool-manager-empty-heading");
    let guidance = bounds(visual, "tool-manager-empty-guidance");
    assert!(heading.origin.y >= empty.origin.y);
    assert!(guidance.origin.y > heading.origin.y);
    assert!(guidance.right() <= empty.right());
}

#[gpui_kit::test]
fn manager_reports_when_search_has_no_matches(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_hidden_tools();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let manager = create_tool_surface_manager(registry, ToolDragSurface::Home, window, cx);
        Root::new(cx.new(|_| ManagerTestHost { manager }), window, cx)
    });
    visual.simulate_resize(size(px(700.0), px(520.0)));
    visual.run_until_parked();
    click(visual, "manager-test-trigger");
    click(visual, "tool-manager-search");
    visual.simulate_keystrokes("unknown");
    visual.run_until_parked();

    let empty = bounds(visual, "tool-manager-empty");
    assert!(visual.debug_bounds("tool-manager-empty-heading").is_some());
    assert!(visual.debug_bounds("tool-manager-empty-guidance").is_some());
    assert!(empty.right() <= px(700.0));
}

struct ManagerTestHost {
    manager: Entity<super::ToolSurfaceManager>,
}

impl Render for ManagerTestHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        h_flex().size_full().child(tool_surface_manager_popover(
            self.manager.clone(),
            ToolDragSurface::Home,
            crate::clickable_button("manager-test-trigger")
                .debug_selector(|| "manager-test-trigger".into())
                .small()
                .label("管理工具"),
        ))
    }
}

/// Toggling Home and sidebar controls changes only that tool's selected surface.
#[gpui_kit::test]
fn home_manager_toggles_both_surfaces_without_uninstalling_tools(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_hidden_tools();
    let registry_for_view = registry.clone();
    let host_handle = Rc::new(RefCell::new(None));
    let host_out = host_handle.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let host = cx.new(|cx| ToolSurfaceTestHost::new(registry_for_view, cx));
        *host_out.borrow_mut() = Some(host.clone());
        Root::new(host, window, cx)
    });
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();

    click(visual, "home-tool-manager");
    assert!(visual.debug_bounds("tool-manager-body").is_some());
    assert!(visual.debug_bounds("tool-manager-row-first").is_some());
    assert_eq!(registry.count(), 2);

    click_surface_control(visual, "tool-manager-home-control-first");
    assert!(visual.debug_bounds("home-tool-first").is_some());
    assert!(visual.debug_bounds("activity-tool-first").is_none());
    assert_eq!(
        registry
            .visibility("first")
            .map(|visibility| (visibility.home, visibility.sidebar)),
        Some((true, false))
    );

    click_surface_control(visual, "tool-manager-sidebar-control-first");
    assert!(visual.debug_bounds("home-tool-first").is_some());
    assert!(visual.debug_bounds("activity-tool-first").is_some());
    assert_eq!(
        registry
            .visibility("first")
            .map(|visibility| (visibility.home, visibility.sidebar)),
        Some((true, true))
    );

    click_surface_control(visual, "tool-manager-home-control-first");
    assert!(visual.debug_bounds("home-tool-first").is_none());
    assert!(visual.debug_bounds("activity-tool-first").is_some());
    assert_eq!(
        registry
            .visibility("first")
            .map(|visibility| (visibility.home, visibility.sidebar)),
        Some((false, true))
    );
    assert!(visual.debug_bounds("tool-manager-body").is_some());

    let host = host_handle.borrow().clone();
    assert!(host.is_some(), "test host should remain available");
    let host = host.unwrap_or_else(|| unreachable!());
    let opened_home_tools = host.read_with(visual, |host, _| host.opened_home_tools.clone());
    assert!(opened_home_tools.is_empty(), "入口管理不能打开工具卡片");
    assert_eq!(registry.count(), 2, "隐藏入口不能卸载工具");
}

/// The sidebar trigger opens the same all-tools catalog and leaves Home unchanged.
#[gpui_kit::test]
fn sidebar_manager_changes_only_the_sidebar_preference(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_hidden_tools();
    let registry_for_view = registry.clone();
    let host_handle = Rc::new(RefCell::new(None));
    let host_out = host_handle.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let host = cx.new(|cx| ToolSurfaceTestHost::new(registry_for_view, cx));
        *host_out.borrow_mut() = Some(host.clone());
        Root::new(host, window, cx)
    });
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();

    click(visual, "activity-tool-manager");
    assert!(visual.debug_bounds("tool-manager-title").is_some());
    assert!(
        visual
            .debug_bounds("tool-manager-home-control-first")
            .is_some()
    );
    assert!(
        visual
            .debug_bounds("tool-manager-sidebar-control-first")
            .is_some()
    );

    click_surface_control(visual, "tool-manager-sidebar-control-first");
    assert!(visual.debug_bounds("activity-tool-first").is_some());
    assert!(visual.debug_bounds("home-tool-first").is_none());
    click_surface_control(visual, "tool-manager-sidebar-control-first");
    assert!(visual.debug_bounds("activity-tool-first").is_none());
    assert!(visual.debug_bounds("home-tool-first").is_none());
    assert_eq!(
        registry
            .visibility("first")
            .map(|visibility| (visibility.home, visibility.sidebar)),
        Some((false, false))
    );

    let host = host_handle.borrow().clone();
    assert!(host.is_some(), "test host should remain available");
    let host = host.unwrap_or_else(|| unreachable!());
    let navigation = host.read_with(visual, |host, _| host.sidebar_navigation.clone());
    assert!(navigation.is_empty(), "入口管理不能导航到工具");
}

/// Both surface controls stay visible when the manager opens at a narrow Home width.
#[gpui_kit::test]
fn manager_fits_compact_home_window(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_hidden_tools();
    registry.set_home_visible("first", true);
    registry.set_home_visible("second", true);
    let registry_for_manager = registry.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let manager =
            create_tool_surface_manager(registry_for_manager, ToolDragSurface::Home, window, cx);
        Root::new(cx.new(|_| ManagerTestHost { manager }), window, cx)
    });
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    click(visual, "manager-test-trigger");
    for selector in [
        "tool-manager-body",
        "tool-manager-row-first",
        "tool-manager-home-control-first",
        "tool-manager-sidebar-control-first",
    ] {
        assert_fits(visual, selector, 360.0, 640.0);
    }
    assert!(visual.debug_bounds("tool-manager-row-second").is_some());
}
