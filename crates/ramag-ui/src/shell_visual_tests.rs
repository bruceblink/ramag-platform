//! 主壳层的 headless GPUI 几何验收。

use std::sync::Arc;

use gpui_kit::component::Root;
use gpui_kit::{
    AppContext as _, Bounds, Context, InteractiveElement as _, IntoElement, Modifiers, Pixels,
    Render, Styled as _, TestAppContext, VisualTestContext, Window, div, px, size,
};
use ramag_app::{DataSyncGate, StaticPluginHost, ToolRegistry};
use ramag_domain::{Tool, ToolMeta};

use super::Shell;
use crate::shell::{SHELL_HEADER_HEIGHT, SHELL_NARROW_BREAKPOINT};

fn assert_header_fits(cx: &mut VisualTestContext, width: f32, height: f32) {
    let header = cx.debug_bounds("workbench-shell-header");
    assert!(header.is_some(), "工作区顶部工具栏应渲染");
    let header = header.unwrap_or_default();
    assert_eq!(header.size.height, px(SHELL_HEADER_HEIGHT));
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
    for (width, height) in [
        (360.0, 640.0),
        (759.0, 640.0),
        (760.0, 640.0),
        (761.0, 640.0),
        (1024.0, 768.0),
        (1440.0, 900.0),
    ] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();
        assert_header_fits(visual_cx, width, height);
        let header = bounds(visual_cx, "workbench-shell-header");
        let theme = bounds(visual_cx, "shell-theme-toggle");
        let expected_padding = if width <= SHELL_NARROW_BREAKPOINT {
            16.0
        } else if width <= 1100.0 {
            23.0
        } else {
            26.0
        };
        assert_eq!(header.right() - theme.right(), px(expected_padding));
        if width <= SHELL_NARROW_BREAKPOINT {
            assert!(visual_cx.debug_bounds("shell-navigation-toggle").is_some());
            let toggle = bounds(visual_cx, "shell-navigation-toggle");
            assert_eq!(toggle.left() - header.left(), px(expected_padding));
            assert!(visual_cx.debug_bounds("shell-compact-navigation").is_some());
            assert!(
                visual_cx
                    .debug_bounds("workbench-shell-workspace")
                    .is_none()
            );
            assert!(
                visual_cx
                    .debug_bounds("workbench-shell-separator")
                    .is_none()
            );
            assert!(visual_cx.debug_bounds("activity-bar").is_none());
        } else {
            assert!(visual_cx.debug_bounds("shell-navigation-toggle").is_none());
            assert!(
                visual_cx
                    .debug_bounds("workbench-shell-workspace")
                    .is_some()
            );
            let workspace = bounds(visual_cx, "workbench-shell-workspace");
            assert_eq!(workspace.left() - header.left(), px(expected_padding));
            assert!(
                visual_cx
                    .debug_bounds("workbench-shell-separator")
                    .is_some()
            );
            let sidebar = bounds(visual_cx, "activity-bar");
            let header = bounds(visual_cx, "workbench-shell-header");
            assert_eq!(sidebar.top(), px(0.0));
            assert_eq!(sidebar.bottom(), px(height));
            assert_eq!(header.top(), sidebar.top());
            assert_eq!(header.left(), sidebar.right());
            assert_eq!(header.right(), px(width));
        }
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

fn click_sidebar(visual: &mut VisualTestContext, width: f32, selector: &'static str) {
    if width <= SHELL_NARROW_BREAKPOINT && visual.debug_bounds("activity-bar").is_none() {
        click(visual, "shell-navigation-toggle");
    }
    click(visual, selector);
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
    registry.register(Arc::new(TestTool {
        meta: ToolMeta::new("dbclient", "数据库客户端", ""),
    }));
    let host = Arc::new(StaticPluginHost::new(registry.clone()));
    let gate = Arc::new(DataSyncGate::default());
    let mut shell_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let tool = cx.new(|_| TestPage {
            selector: "shell-test-tool",
        });
        let dbclient = cx.new(|_| TestPage {
            selector: "shell-test-dbclient",
        });
        let settings = cx.new(|_| TestPage {
            selector: "shell-test-settings",
        });
        let shell = cx.new(|cx| {
            let mut shell = Shell::new(registry.clone(), host.clone(), gate.clone(), window, cx);
            shell.register_tool_view("ssh", tool.into());
            shell.register_tool_view("dbclient", dbclient.into());
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
            click_sidebar(visual, width, "activity-tool-ssh");
            assert!(visual.debug_bounds("shell-test-tool").is_some());
            if width <= SHELL_NARROW_BREAKPOINT {
                click(visual, "shell-navigation-toggle");
            }
            let header = bounds(visual, "workbench-shell-header");
            let title = bounds(visual, "workbench-shell-page");
            let theme = bounds(visual, "shell-theme-toggle");
            let content = bounds(visual, "workbench-shell-content");
            for element in [title, theme] {
                assert!(element.left() >= header.left() && element.right() <= header.right());
                assert!(element.top() >= header.top() && element.bottom() <= header.bottom());
            }
            if width > SHELL_NARROW_BREAKPOINT {
                let workspace = bounds(visual, "workbench-shell-workspace");
                let separator = bounds(visual, "workbench-shell-separator");
                assert!(workspace.right() <= separator.left());
                assert!(separator.right() <= title.left());
            } else {
                assert!(visual.debug_bounds("workbench-shell-workspace").is_none());
                assert!(visual.debug_bounds("workbench-shell-separator").is_none());
            }
            assert!(title.right() <= theme.left());
            assert!(
                visual
                    .debug_bounds("activity-selected-entry-button-tool-ssh")
                    .is_some(),
                "工具导航应同步高亮当前 SSH 页面"
            );
            assert!(visual.debug_bounds("shell-tool-settings").is_none());
            assert_eq!(theme.size, size(px(28.0), px(28.0)));
            assert!(content.top() >= header.bottom() && content.bottom() <= px(height));

            click_sidebar(visual, width, "activity-tool-dbclient");
            assert!(visual.debug_bounds("shell-test-dbclient").is_some());
            if width > SHELL_NARROW_BREAKPOINT {
                let sidebar = bounds(visual, "activity-bar");
                let header = bounds(visual, "workbench-shell-header");
                assert_eq!(header.left(), sidebar.right());
                assert_eq!(header.right(), px(width));
            }
            if width <= SHELL_NARROW_BREAKPOINT {
                click(visual, "shell-navigation-toggle");
            }
            assert!(
                visual
                    .debug_bounds("activity-selected-entry-button-tool-dbclient")
                    .is_some(),
                "点击数据库入口后，侧栏高亮应同步切换"
            );
            assert!(
                visual
                    .debug_bounds("activity-selected-entry-button-tool-ssh")
                    .is_none()
            );
            click_sidebar(visual, width, "activity-tool-ssh");

            click(visual, "shell-theme-toggle");
            let changed = visual.update(|_, cx| crate::current_mode(cx));
            assert_ne!(changed, mode);
            click_sidebar(visual, width, "activity-settings");
            assert!(visual.debug_bounds("shell-test-settings").is_some());
            assert!(visual.debug_bounds("shell-tool-settings").is_none());
            click_sidebar(visual, width, "activity-home");
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

/// 验证窄屏侧栏不占主内容宽度，并支持 Escape、点击遮罩和注册工具快捷入口。
#[gpui_kit::test]
fn compact_shell_navigation_keeps_content_and_closes_on_escape_or_backdrop(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::component::init);
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(TestTool {
        meta: ToolMeta::new("api", "API 测试", ""),
    }));
    registry.register(Arc::new(TestTool {
        meta: ToolMeta::new("dbclient", "数据库客户端", ""),
    }));
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let gate = Arc::new(DataSyncGate::default());
    let (_, visual) = cx.add_window_view(|window, cx| {
        let api = cx.new(|_| TestPage {
            selector: "shell-test-api",
        });
        let database = cx.new(|_| TestPage {
            selector: "shell-test-database",
        });
        let shell = cx.new(|cx| {
            let mut shell = Shell::new(
                registry.clone(),
                plugin_host.clone(),
                gate.clone(),
                window,
                cx,
            );
            shell.register_tool_view("api", api.into());
            shell.register_tool_view("dbclient", database.into());
            shell
        });
        Root::new(shell, window, cx)
    });
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();

    assert!(visual.debug_bounds("activity-bar").is_none());
    assert!(visual.debug_bounds("shell-navigation-toggle").is_some());
    for selector in [
        "shell-compact-navigation",
        "shell-compact-nav-selected-home",
        "shell-compact-nav-api",
        "shell-compact-nav-database",
        "shell-compact-nav-settings",
    ] {
        assert!(
            visual.debug_bounds(selector).is_some(),
            "missing {selector}"
        );
    }
    let content_before = bounds(visual, "workbench-shell-content");
    assert_eq!(content_before.origin.x, px(0.0));
    assert_eq!(content_before.size.width, px(360.0));

    click(visual, "shell-navigation-toggle");
    // 打开回调在下一帧转移焦点；推进真实回调而不是在测试中直接设置焦点。
    visual.update(|window, cx| {
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
    assert!(
        visual.update(|window, cx| { gpui_kit::base::active_focus_trap(window, cx).is_some() })
    );
    visual.simulate_keystrokes("tab tab");
    visual.run_until_parked();
    assert!(
        visual.update(|window, cx| { gpui_kit::base::active_focus_trap(window, cx).is_some() })
    );
    let drawer = bounds(visual, "shell-navigation-drawer");
    assert_eq!(drawer.origin, gpui_kit::point(px(14.0), px(16.0)));
    assert_eq!(drawer.size, size(px(332.0), px(608.0)));
    assert!(visual.debug_bounds("activity-tool-api").is_some());
    assert!(visual.debug_bounds("activity-tool-dbclient").is_some());
    assert_eq!(bounds(visual, "workbench-shell-content"), content_before);
    click(visual, "activity-tool-manager");
    assert!(visual.debug_bounds("tool-manager-body").is_some());
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    assert!(visual.debug_bounds("tool-manager-body").is_none());
    assert!(visual.debug_bounds("activity-bar").is_some());
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    assert!(visual.debug_bounds("activity-bar").is_none());

    click(visual, "shell-navigation-toggle");
    visual.update(|window, cx| {
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    });
    click(visual, "shell-navigation-close");
    assert!(visual.debug_bounds("activity-bar").is_none());

    click(visual, "shell-navigation-toggle");
    visual.simulate_resize(size(px(761.0), px(640.0)));
    visual.run_until_parked();
    assert!(visual.debug_bounds("activity-bar").is_some());
    assert!(visual.debug_bounds("shell-navigation-drawer").is_none());
    visual.simulate_resize(size(px(759.0), px(640.0)));
    visual.run_until_parked();
    assert!(visual.debug_bounds("activity-bar").is_none());
    assert!(visual.debug_bounds("shell-navigation-drawer").is_none());

    let theme_before = visual.update(|_, cx| crate::current_mode(cx));
    let theme_position = bounds(visual, "shell-theme-toggle").center();
    click(visual, "shell-navigation-toggle");
    assert!(visual.debug_bounds("activity-bar").is_some());
    // 抽屉打开时，顶栏应与主内容一样被遮罩，不能触发主题按钮。
    visual.simulate_click(theme_position, Modifiers::default());
    visual.run_until_parked();
    assert_eq!(visual.update(|_, cx| crate::current_mode(cx)), theme_before);
    visual.simulate_click(gpui_kit::point(px(5.0), px(5.0)), Modifiers::default());
    visual.run_until_parked();
    assert!(visual.debug_bounds("activity-bar").is_none());

    click(visual, "shell-navigation-toggle");
    visual.simulate_click(gpui_kit::point(px(5.0), px(300.0)), Modifiers::default());
    visual.run_until_parked();
    assert!(visual.debug_bounds("activity-bar").is_none());

    click(visual, "shell-compact-nav-api");
    assert!(visual.debug_bounds("shell-test-api").is_some());
    assert!(
        visual
            .debug_bounds("shell-compact-nav-selected-api")
            .is_some()
    );
    assert!(visual.debug_bounds("activity-bar").is_none());

    click(visual, "shell-compact-nav-database");
    assert!(visual.debug_bounds("shell-test-database").is_some());
    assert!(
        visual
            .debug_bounds("shell-compact-nav-selected-database")
            .is_some()
    );
}

/// 验证首页仍显示的工具不会绕过侧栏独立显隐设置进入底部快捷栏。
#[gpui_kit::test]
fn compact_shortcuts_respect_sidebar_visibility(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(TestTool {
        meta: ToolMeta::new("api", "API 测试", ""),
    }));
    assert!(registry.set_sidebar_visible("api", false));
    assert!(
        registry
            .visibility("api")
            .is_some_and(|visibility| visibility.home)
    );
    assert!(registry.is_visible("api"));
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let gate = Arc::new(DataSyncGate::default());
    let (_, visual) = cx.add_window_view(|window, cx| {
        let shell = cx.new(|cx| {
            Shell::new(
                registry.clone(),
                plugin_host.clone(),
                gate.clone(),
                window,
                cx,
            )
        });
        Root::new(shell, window, cx)
    });
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    assert!(visual.debug_bounds("shell-compact-nav-api").is_none());
    click(visual, "shell-navigation-toggle");
    click(visual, "activity-tool-manager");
    assert!(visual.debug_bounds("activity-tool-api").is_none());
    for expected_sidebar_visibility in [true, false] {
        let control = bounds(visual, "tool-manager-sidebar-control-api");
        let switch = gpui_kit::point(
            control.origin.x + control.size.width * 0.82,
            control.center().y,
        );
        visual.simulate_click(switch, Modifiers::default());
        visual.run_until_parked();
        assert_eq!(
            registry
                .visibility("api")
                .is_some_and(|visibility| visibility.sidebar),
            expected_sidebar_visibility
        );
        assert_eq!(
            visual.debug_bounds("activity-tool-api").is_some(),
            expected_sidebar_visibility
        );
        assert_eq!(
            visual.debug_bounds("shell-compact-nav-api").is_some(),
            expected_sidebar_visibility
        );
    }
}

#[path = "shell_navigation_overflow_tests.rs"]
mod navigation_overflow_tests;
