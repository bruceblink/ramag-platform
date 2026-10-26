//! Rendered checks for independent Home and activity-bar visibility controls.

use std::sync::Arc;

use gpui_kit::component::v_flex;
use gpui_kit::{
    AppContext as _, Bounds, Context, IntoElement, Modifiers, ParentElement, Pixels, Render,
    Styled, TestAppContext, VisualTestContext, Window, px, size,
};
use ramag_app::{StaticPluginHost, ToolRegistry};
use ramag_domain::{Tool, ToolMeta};

use super::ToolVisibilityView;
use crate::activity_bar::ActivityBar;
use crate::home_view::HomeView;

struct TestTool {
    meta: ToolMeta,
}

impl Tool for TestTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

struct ToolSurfacesTestHost {
    activity_bar: gpui_kit::Entity<ActivityBar>,
    home: gpui_kit::Entity<HomeView>,
    visibility: gpui_kit::Entity<ToolVisibilityView>,
}

impl ToolSurfacesTestHost {
    fn new(registry: Arc<ToolRegistry>, cx: &mut Context<Self>) -> Self {
        let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
        Self {
            activity_bar: cx.new(|cx| ActivityBar::new(registry.clone(), plugin_host, cx)),
            home: cx.new(|cx| HomeView::new(registry.clone(), cx)),
            visibility: cx.new(|_| ToolVisibilityView::new(registry)),
        }
    }
}

impl Render for ToolSurfacesTestHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui_kit::component::h_flex()
            .size_full()
            .child(self.activity_bar.clone())
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(self.visibility.clone())
                    .child(self.home.clone()),
            )
    }
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let target = bounds(cx, selector);
    cx.simulate_click(target.center(), Modifiers::default());
    cx.run_until_parked();
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    let result = cx.debug_bounds(selector);
    assert!(result.is_some(), "missing visibility element: {selector}");
    result.unwrap_or_default()
}

fn registry_with_tools() -> Arc<ToolRegistry> {
    let registry = Arc::new(ToolRegistry::new());
    for id in ["first", "second"] {
        registry.register(Arc::new(TestTool {
            meta: ToolMeta::new(id, format!("Tool {id}"), "A visibility test tool."),
        }));
    }
    registry
}

/// A Home toggle must leave the sidebar entry intact, and vice versa.
#[gpui_kit::test]
fn rendered_switches_update_home_and_sidebar_independently(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_tools();
    let registry_for_view = registry.clone();
    let (_, visual) =
        cx.add_window_view(move |_, cx| ToolSurfacesTestHost::new(registry_for_view.clone(), cx));
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();

    assert!(visual.debug_bounds("home-tool-first").is_some());
    assert!(visual.debug_bounds("activity-tool-first").is_some());
    assert!(visual.debug_bounds("activity-settings").is_some());

    click(visual, "settings-tool-home-first");
    assert!(visual.debug_bounds("home-tool-first").is_none());
    assert!(visual.debug_bounds("activity-tool-first").is_some());

    click(visual, "settings-tool-sidebar-first");
    assert!(visual.debug_bounds("activity-tool-first").is_none());
    assert!(visual.debug_bounds("activity-settings").is_some());
    assert_eq!(
        registry
            .visibility("first")
            .map(|visibility| (visibility.home, visibility.sidebar)),
        Some((false, false))
    );

    click(visual, "settings-tool-home-first");
    assert!(visual.debug_bounds("home-tool-first").is_some());
    assert!(visual.debug_bounds("activity-tool-first").is_none());

    click(visual, "settings-tool-sidebar-first");
    assert!(visual.debug_bounds("activity-tool-first").is_some());
}

/// Settings cards and both switches stay inside compact and desktop content widths.
#[gpui_kit::test]
fn visibility_page_fits_compact_and_desktop_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_tools();
    let (_, visual) = cx.add_window_view(move |_, _cx| ToolVisibilityView::new(registry));
    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        visual.simulate_resize(size(px(width), px(height)));
        visual.run_until_parked();
        let page = bounds(visual, "tool-visibility");
        let summary = bounds(visual, "tool-visibility-summary");
        let total = bounds(visual, "tool-visibility-summary-total");
        let home_summary = bounds(visual, "tool-visibility-summary-home");
        let sidebar_summary = bounds(visual, "tool-visibility-summary-sidebar");
        let row = bounds(visual, "settings-tool-entry-first");
        let home = bounds(visual, "settings-tool-home-first");
        let sidebar = bounds(visual, "settings-tool-sidebar-first");
        for metric in [summary, total, home_summary, sidebar_summary] {
            assert!(metric.origin.x >= page.origin.x);
            assert!(metric.right() <= page.right());
            assert!(metric.origin.y >= page.origin.y);
            assert!(metric.bottom() <= page.bottom());
        }
        assert!(row.origin.x >= page.origin.x);
        assert!(row.right() <= page.right());
        assert!(home.origin.x >= row.origin.x && home.right() <= row.right());
        assert!(sidebar.origin.x >= row.origin.x && sidebar.right() <= row.right());
    }
}
