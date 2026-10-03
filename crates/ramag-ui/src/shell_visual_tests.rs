//! 主壳层的 headless GPUI 几何验收。

use std::sync::Arc;

use gpui_kit::component::Root;
use gpui_kit::{
    AppContext as _, Bounds, Context, InteractiveElement as _, IntoElement, Modifiers, Pixels,
    Render, Styled as _, TestAppContext, VisualTestContext, Window, div, px, size,
};
use ramag_app::{DataSyncGate, StaticPluginHost, ToolRegistry};
use ramag_domain::{Tool, ToolMeta};

use super::{Shell, WORKBENCH_TOOLBAR_HEIGHT};

fn assert_header_fits(cx: &mut VisualTestContext, width: f32, height: f32) {
    let header = cx.debug_bounds("workbench-shell-header");
    assert!(header.is_some(), "工作区顶部工具栏应渲染");
    let header = header.unwrap_or_default();
    assert_eq!(header.size.height, px(WORKBENCH_TOOLBAR_HEIGHT));
    assert!(header.origin.x >= px(0.0));
    assert!(header.origin.y >= px(0.0));
    assert!(header.right() <= px(width));
    assert!(header.bottom() <= px(height));
}

/// 在紧凑和桌面尺寸下验证共享壳层的顶部工具栏保持稳定高度并不越界。
#[gpui_kit::test]
fn workbench_shell_header_stays_inside_supported_window_sizes(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = Arc::new(ToolRegistry::new());
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let gate = Arc::new(DataSyncGate::default());
    let (_, visual_cx) = cx.add_window_view(move |window, cx| {
        Shell::new(
            registry.clone(),
            plugin_host.clone(),
            gate.clone(),
            window,
            cx,
        )
    });
    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();
        assert_header_fits(visual_cx, width, height);
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

struct TestPage {
    selector: &'static str,
}

impl Render for TestPage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let selector = self.selector;
        div().size_full().debug_selector(move || selector.into())
    }
}

fn bounds(visual: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    let bounds = visual.debug_bounds(selector);
    assert!(bounds.is_some(), "missing shell element: {selector}");
    bounds.unwrap_or_default()
}

fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let center = bounds(visual, selector).center();
    visual.simulate_click(center, Modifiers::default());
    visual.run_until_parked();
}

/// Exercises the real shell navigation and theme action while checking that long
/// page names cannot overlap fixed actions or reduce the content viewport.
#[gpui_kit::test]
fn pulse_shell_keeps_titles_actions_and_navigation_reachable(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(TestTool {
        meta: ToolMeta::new("ssh", "SSH / 长工具名 ".repeat(12), ""),
    }));
    let host = Arc::new(StaticPluginHost::new(registry.clone()));
    let gate = Arc::new(DataSyncGate::default());
    let mut shell_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let tool = cx.new(|_| TestPage {
            selector: "shell-test-tool",
        });
        let settings = cx.new(|_| TestPage {
            selector: "shell-test-settings",
        });
        let shell = cx.new(|cx| {
            let mut shell = Shell::new(registry.clone(), host.clone(), gate.clone(), window, cx);
            shell.register_tool_view("ssh", tool.into());
            shell.set_settings_view(settings.into());
            shell
        });
        shell_entity = Some(shell.clone());
        Root::new(shell, window, cx)
    });
    let Some(shell) = shell_entity else {
        unreachable!("shell was created")
    };
    for mode in [crate::Mode::Light, crate::Mode::Dark] {
        for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
            visual.update(|_, cx| crate::apply_theme(mode, cx));
            visual.simulate_resize(size(px(width), px(height)));
            click(visual, "activity-tool-ssh");
            assert!(visual.debug_bounds("shell-test-tool").is_some());
            let header = bounds(visual, "workbench-shell-header");
            let brand = bounds(visual, "pulse-workbench-brand");
            let title = bounds(visual, "pulse-workbench-title");
            let theme = bounds(visual, "shell-theme-toggle");
            let content = bounds(visual, "workbench-shell-content");
            for element in [brand, title, theme] {
                assert!(element.left() >= header.left() && element.right() <= header.right());
                assert!(element.top() >= header.top() && element.bottom() <= header.bottom());
            }
            assert!(brand.right() <= title.left());
            assert!(title.right() <= theme.left());
            assert!(visual.debug_bounds("shell-tool-settings").is_none());
            assert_eq!(theme.size, size(px(28.0), px(28.0)));
            assert!(content.top() >= header.bottom() && content.bottom() <= px(height));
            click(visual, "shell-theme-toggle");
            let changed = visual.update(|_, cx| crate::current_mode(cx));
            assert_ne!(changed, mode);
            click(visual, "activity-settings");
            assert!(visual.debug_bounds("shell-test-settings").is_some());
            assert!(visual.debug_bounds("shell-tool-settings").is_none());
            click(visual, "activity-home");
            assert!(visual.debug_bounds("shell-test-settings").is_none());
            visual.update(|window, cx| {
                shell.update(cx, |shell, cx| {
                    shell.navigate_to(crate::NavTarget::Tool("missing-tool".into()), window, cx)
                });
            });
            visual.run_until_parked();
            let unavailable = bounds(visual, "pulse-status-notice");
            assert!(unavailable.left() >= content.left() && unavailable.right() <= content.right());
            assert!(unavailable.top() >= content.top() && unavailable.bottom() <= content.bottom());
        }
    }
}
