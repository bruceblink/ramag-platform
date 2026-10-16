//! Headless interaction checks for compact add pickers and entry actions.

use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};

use gpui_kit::component::{Root, Sizable as _, h_flex};
use gpui_kit::{
    AppContext as _, Bounds, Context, Entity, InteractiveElement as _, IntoElement, Modifiers,
    ParentElement as _, Pixels, Render, Styled as _, Subscription, TestAppContext,
    VisualTestContext, Window, px, size,
};
use ramag_app::{StaticPluginHost, ToolRegistry};
use ramag_domain::{Tool, ToolMeta};

use crate::tool_layout::ToolDragSurface;
use crate::tool_surface_manager::{create_tool_surface_picker, tool_picker_popover};
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
    assert!(result.is_some(), "missing tool entry element: {selector}");
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

/// Search results contain only tools that are not already shown on this surface.
#[gpui_kit::test]
fn picker_search_matches_name_id_description_and_current_surface(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_hidden_tools();
    registry.set_home_visible("first", true);
    let registry_for_picker = registry.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let picker =
            create_tool_surface_picker(registry_for_picker, ToolDragSurface::Home, window, cx);
        Root::new(cx.new(|_| PickerTestHost { picker }), window, cx)
    });
    visual.simulate_resize(size(px(700.0), px(520.0)));
    visual.run_until_parked();
    click(visual, "picker-test-trigger");
    assert!(visual.debug_bounds("tool-picker-row-home-first").is_none());
    assert!(visual.debug_bounds("tool-picker-row-home-second").is_some());

    click(visual, "tool-picker-search");
    visual.simulate_keystrokes("second");
    visual.run_until_parked();
    assert!(visual.debug_bounds("tool-picker-row-home-first").is_none());
    assert!(visual.debug_bounds("tool-picker-row-home-second").is_some());
}

struct PickerTestHost {
    picker: Entity<super::ToolSurfacePicker>,
}

impl Render for PickerTestHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        h_flex().size_full().child(tool_picker_popover(
            self.picker.clone(),
            ToolDragSurface::Home,
            crate::clickable_button("picker-test-trigger")
                .debug_selector(|| "picker-test-trigger".into())
                .small()
                .label("添加工具"),
        ))
    }
}

/// Add from either surface in one click and remove from the entry's own actions.
#[gpui_kit::test]
fn quick_add_and_entry_removal_keep_surfaces_independent(cx: &mut TestAppContext) {
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

    click(visual, "home-tool-actions");
    assert!(visual.debug_bounds("tool-picker-body").is_some());
    click(visual, "tool-picker-row-home-first");
    assert!(visual.debug_bounds("tool-picker-body").is_some());
    assert!(visual.debug_bounds("home-tool-first").is_some());
    assert!(visual.debug_bounds("tool-picker-row-home-first").is_none());
    assert!(visual.debug_bounds("activity-tool-first").is_none());
    assert_eq!(registry.count(), 2, "添加入口不能重复注册工具");

    click(visual, "home-tool-actions");
    click(visual, "tool-entry-actions-home-first");
    assert!(
        visual
            .debug_bounds("tool-entry-remove-home-first")
            .is_some()
    );
    click(visual, "tool-entry-remove-home-first");
    assert!(visual.debug_bounds("home-tool-first").is_none());
    assert!(visual.debug_bounds("tool-picker-body").is_none());
    assert_eq!(
        registry
            .visibility("first")
            .map(|visibility| (visibility.home, visibility.sidebar)),
        Some((false, false))
    );
    let host = host_handle.borrow().clone();
    assert!(host.is_some(), "test host should remain available");
    let host = host.unwrap_or_else(|| unreachable!());
    let opened_home_tools = host.read_with(visual, |host, _| host.opened_home_tools.clone());
    assert!(opened_home_tools.is_empty(), "更多操作不能打开工具卡片");

    click(visual, "home-tool-actions");
    assert!(visual.debug_bounds("tool-picker-row-home-first").is_some());
    click(visual, "home-tool-actions");

    click(visual, "activity-add-menu");
    assert!(visual.debug_bounds("tool-picker-body").is_some());
    click(visual, "tool-picker-row-sidebar-first");
    assert!(visual.debug_bounds("activity-tool-first").is_some());
    assert!(visual.debug_bounds("home-tool-first").is_none());
    assert_eq!(
        registry
            .visibility("first")
            .map(|visibility| (visibility.home, visibility.sidebar)),
        Some((false, true))
    );

    click(visual, "activity-add-menu");
    click(visual, "tool-entry-actions-sidebar-first");
    assert!(
        visual
            .debug_bounds("tool-entry-remove-sidebar-first")
            .is_some()
    );
    click(visual, "tool-entry-remove-sidebar-first");
    assert!(visual.debug_bounds("activity-tool-first").is_none());
    assert_eq!(
        registry
            .visibility("first")
            .map(|visibility| (visibility.home, visibility.sidebar)),
        Some((false, false))
    );
    let navigation = host.read_with(visual, |host, _| host.sidebar_navigation.clone());
    assert!(navigation.is_empty(), "更多操作不能导航到工具");

    click(visual, "activity-add-menu");
    assert!(
        visual
            .debug_bounds("tool-picker-row-sidebar-first")
            .is_some()
    );
    click(visual, "tool-picker-row-sidebar-first");
    assert!(visual.debug_bounds("activity-tool-first").is_some());
}

/// The anchored picker stays inside a compact viewport and has a useful empty state.
#[gpui_kit::test]
fn picker_fits_compact_window_and_reports_no_available_tools(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_hidden_tools();
    registry.set_home_visible("first", true);
    registry.set_home_visible("second", true);
    let registry_for_picker = registry.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let picker =
            create_tool_surface_picker(registry_for_picker, ToolDragSurface::Home, window, cx);
        Root::new(cx.new(|_| PickerTestHost { picker }), window, cx)
    });
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    click(visual, "picker-test-trigger");
    for selector in [
        "tool-picker-body",
        "tool-picker-search",
        "tool-picker-scroll",
    ] {
        assert_fits(visual, selector, 360.0, 640.0);
    }
    assert!(visual.debug_bounds("tool-picker-row-home-first").is_none());
    assert!(visual.debug_bounds("tool-picker-row-home-second").is_none());
}
