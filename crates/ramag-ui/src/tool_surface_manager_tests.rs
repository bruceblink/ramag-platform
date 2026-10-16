//! Headless interaction checks for searchable Home and sidebar tool management.

use std::sync::Arc;

use gpui_kit::component::{Root, h_flex};
use gpui_kit::{
    AppContext as _, Bounds, Context, Entity, IntoElement, Modifiers, ParentElement as _, Pixels,
    Render, Styled, TestAppContext, VisualTestContext, Window, px, size,
};
use ramag_app::{StaticPluginHost, ToolRegistry};
use ramag_domain::{Tool, ToolMeta};

use crate::tool_layout::ToolDragSurface;
use crate::tool_surface_manager::{open_tool_surface_manager, tool_matches};
use crate::{ActivityBar, HomeView};

struct ToolSurfaceTestHost {
    activity_bar: Entity<ActivityBar>,
    home: Entity<HomeView>,
}

impl ToolSurfaceTestHost {
    fn new(registry: Arc<ToolRegistry>, cx: &mut Context<Self>) -> Self {
        let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
        Self {
            activity_bar: cx.new(|cx| ActivityBar::new(registry.clone(), plugin_host, cx)),
            home: cx.new(|cx| HomeView::new(registry, cx)),
        }
    }
}

impl Render for ToolSurfaceTestHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .child(self.activity_bar.clone())
            .child(self.home.clone())
            .children(Root::render_dialog_layer(window, cx))
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

fn registry_with_tools() -> Arc<ToolRegistry> {
    let registry = Arc::new(ToolRegistry::new());
    for id in ["first", "second"] {
        registry.register(Arc::new(TestTool {
            meta: ToolMeta::new(id, format!("Tool {id}"), "A searchable test tool."),
        }));
    }
    registry
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let target = bounds(cx, selector);
    cx.simulate_mouse_move(target.center(), None, Modifiers::default());
    cx.simulate_mouse_down(
        target.center(),
        gpui_kit::MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        target.center(),
        gpui_kit::MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    let result = cx.debug_bounds(selector);
    assert!(result.is_some(), "missing tool manager element: {selector}");
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

/// Search matches a readable name, stable tool ID, or description.
#[test]
fn tool_search_matches_name_id_and_description() {
    let meta = ToolMeta::new("api-test", "API Testing", "Build and send HTTP requests.");
    assert!(tool_matches(&meta, "testing"));
    assert!(tool_matches(&meta, "api-test"));
    assert!(tool_matches(&meta, "http"));
    assert!(!tool_matches(&meta, "kafka"));
}

/// Both surfaces use one manager layout and keep add/remove state independent.
#[gpui_kit::test]
fn home_and_sidebar_manager_updates_entries_without_closing(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_tools();
    let registry_for_view = registry.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let host = cx.new(|cx| ToolSurfaceTestHost::new(registry_for_view, cx));
        Root::new(host, window, cx)
    });
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();

    click(visual, "home-tool-actions");
    assert!(visual.debug_bounds("tool-manager-body").is_some());
    assert!(visual.debug_bounds("tool-manager-row-home-first").is_some());
    click(visual, "tool-manager-action-home-first");
    assert!(visual.debug_bounds("tool-manager-body").is_some());
    assert!(visual.debug_bounds("tool-manager-row-home-first").is_some());
    assert!(visual.debug_bounds("home-tool-first").is_none());
    assert!(visual.debug_bounds("activity-tool-first").is_some());
    assert_eq!(
        registry
            .visibility("first")
            .map(|visibility| (visibility.home, visibility.sidebar)),
        Some((false, true))
    );

    click(visual, "tool-manager-action-home-first");
    assert!(visual.debug_bounds("home-tool-first").is_some());
    assert!(visual.debug_bounds("activity-tool-first").is_some());
    assert_eq!(registry.count(), 2, "移除入口不能卸载工具");

    click(visual, "tool-manager-done");
    assert!(visual.debug_bounds("tool-manager-body").is_none());
    click(visual, "activity-add-menu");
    assert!(visual.debug_bounds("tool-manager-body").is_some());
    click(visual, "tool-manager-action-sidebar-first");
    assert!(visual.debug_bounds("tool-manager-body").is_some());
    assert!(visual.debug_bounds("activity-tool-first").is_none());
    assert!(visual.debug_bounds("home-tool-first").is_some());
    assert_eq!(
        registry
            .visibility("first")
            .map(|visibility| (visibility.home, visibility.sidebar)),
        Some((true, false))
    );
    click(visual, "tool-manager-action-sidebar-first");
    assert!(visual.debug_bounds("activity-tool-first").is_some());
    click(visual, "tool-manager-done");
}

/// The panel remains inside the available viewport in a compact window.
#[gpui_kit::test]
fn tool_manager_search_filters_rows_and_fits_compact_window(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_tools();
    let registry_for_view = registry.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let host = cx.new(|cx| ToolSurfaceTestHost::new(registry_for_view, cx));
        Root::new(host, window, cx)
    });
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    let manager_handle = std::rc::Rc::new(std::cell::RefCell::new(None));
    let manager_out = manager_handle.clone();
    visual.update(|window, app| {
        *manager_out.borrow_mut() = Some(open_tool_surface_manager(
            registry.clone(),
            ToolDragSurface::Home,
            window,
            app,
        ));
    });
    visual.run_until_parked();

    for selector in [
        "tool-manager-body",
        "tool-manager-search",
        "tool-manager-scroll",
        "tool-manager-group-current",
        "tool-manager-group-available",
        "tool-manager-done",
    ] {
        assert_fits(visual, selector, 360.0, 640.0);
    }
    let initial_body_height = bounds(visual, "tool-manager-body").size.height;

    let manager = manager_handle.borrow().clone();
    assert!(manager.is_some(), "manager panel should be created");
    if let Some(manager) = manager {
        visual.update(|window, app| {
            manager.update(app, |manager, cx| {
                manager
                    .search
                    .update(cx, |state, cx| state.set_value("second", window, cx));
            });
        });
    }
    visual.run_until_parked();
    assert!(visual.debug_bounds("tool-manager-row-home-first").is_none());
    assert!(
        visual
            .debug_bounds("tool-manager-row-home-second")
            .is_some()
    );
    let filtered_body_height = bounds(visual, "tool-manager-body").size.height;
    assert!(
        filtered_body_height < initial_body_height,
        "filtered results should reduce empty dialog space: {initial_body_height:?} -> {filtered_body_height:?}"
    );
    assert_fits(visual, "tool-manager-scroll", 360.0, 640.0);
}
