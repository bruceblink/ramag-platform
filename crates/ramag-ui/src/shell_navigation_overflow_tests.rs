//! 短窗口工具导航抽屉的溢出滚动与固定命令入口验收。

use std::sync::Arc;

use gpui_kit::component::Root;
use gpui_kit::{
    AppContext as _, ScrollDelta, ScrollEvent, TestAppContext, TouchPhase, point, px, size,
};
use ramag_app::{DataSyncGate, StaticPluginHost, ToolRegistry};
use ramag_domain::ToolMeta;

use super::{Shell, TestPage, TestTool, bounds, click};

/// 在短窗口中用真实滚轮滚到末项，并验证所有固定入口仍可见且末项能导航。
#[gpui_kit::test]
fn compact_navigation_scrolls_overflow_tools_and_keeps_actions_reachable(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    cx.update(|app| app.set_reduce_motion(true));
    let registry = Arc::new(ToolRegistry::new());
    for index in 0..24 {
        registry.register(Arc::new(TestTool {
            meta: ToolMeta::new(
                format!("overflow-{index}"),
                format!("隔离滚动工具 {index}"),
                "",
            ),
        }));
    }
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let gate = Arc::new(DataSyncGate::default());
    let (_, visual) = cx.add_window_view(|window, cx| {
        let last_tool = cx.new(|_| TestPage {
            selector: "shell-overflow-last-tool-page",
        });
        let settings = cx.new(|_| TestPage {
            selector: "shell-overflow-settings-page",
        });
        let shell = cx.new(|cx| {
            let mut shell = Shell::new(
                registry.clone(),
                plugin_host.clone(),
                gate.clone(),
                window,
                cx,
            );
            shell.register_tool_view("overflow-23", last_tool.into());
            shell.set_settings_view(settings.into());
            shell
        });
        Root::new(shell, window, cx)
    });
    visual.simulate_resize(size(px(360.0), px(240.0)));
    visual.run_until_parked();

    click(visual, "shell-navigation-toggle");
    // 等待抽屉完成焦点转移和实际布局，再测量滚动区与固定入口。
    visual.update(|window, cx| {
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
    let mut drawer = bounds(visual, "shell-navigation-drawer");
    assert_eq!(drawer.origin, point(px(14.0), px(16.0)));
    assert_eq!(drawer.size, size(px(332.0), px(208.0)));
    let title = bounds(visual, "activity-navigation-title");
    assert!(title.left() >= drawer.left() && title.right() <= drawer.right());
    assert!(title.top() >= drawer.top() && title.bottom() <= drawer.bottom());
    assert!(visual.debug_bounds("activity-brand-mark").is_none());
    assert!(visual.debug_bounds("activity-brand-name").is_none());
    for selector in [
        "shell-navigation-close",
        "activity-tool-manager",
        "activity-shortcuts",
        "activity-settings",
    ] {
        let action = bounds(visual, selector);
        assert!(action.size.width > px(0.0) && action.size.height > px(0.0));
        assert!(action.left() >= drawer.left(), "{selector}: {action:?}");
        assert!(action.top() >= drawer.top(), "{selector}: {action:?}");
        assert!(action.right() <= drawer.right(), "{selector}: {action:?}");
        assert!(action.bottom() <= drawer.bottom(), "{selector}: {action:?}");
    }

    let manager = bounds(visual, "activity-tool-manager");
    let shortcuts = bounds(visual, "activity-shortcuts");
    let settings = bounds(visual, "activity-settings");
    for action in [manager, shortcuts, settings] {
        assert_eq!(action.size, size(px(32.0), px(32.0)));
    }
    assert_eq!(manager.center().y, shortcuts.center().y);
    assert_eq!(shortcuts.center().y, settings.center().y);
    assert!(manager.right() < shortcuts.left());
    assert!(shortcuts.right() < settings.left());
    for selector in [
        "activity-entry-label-tool-manager",
        "activity-entry-label-shortcuts",
        "activity-entry-label-settings",
    ] {
        assert!(
            visual.debug_bounds(selector).is_none(),
            "短窗口命令区应只显示图标：{selector}"
        );
    }

    click(visual, "activity-tool-manager");
    assert!(visual.debug_bounds("tool-manager-body").is_some());
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    assert!(visual.debug_bounds("tool-manager-body").is_none());
    assert!(visual.debug_bounds("shell-navigation-drawer").is_some());
    click(visual, "activity-shortcuts");
    assert!(visual.debug_bounds("shortcuts-body").is_some());
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    assert!(visual.debug_bounds("shortcuts-body").is_none());
    assert!(visual.debug_bounds("shell-navigation-drawer").is_some());
    click(visual, "activity-settings");
    assert!(
        visual
            .debug_bounds("shell-overflow-settings-page")
            .is_some()
    );
    assert!(visual.debug_bounds("shell-navigation-drawer").is_none());
    click(visual, "shell-navigation-toggle");
    visual.update(|window, cx| {
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
    drawer = bounds(visual, "shell-navigation-drawer");

    let scroll = bounds(visual, "activity-tool-scroll");
    let last_tool = bounds(visual, "activity-tool-overflow-23");
    assert!(scroll.size.height > px(0.0));
    assert!(
        !scroll.contains(&last_tool.center()),
        "末项初始应位于短视口之外：last={last_tool:?}, scroll={scroll:?}"
    );
    visual.simulate_event(ScrollEvent {
        position: scroll.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-10000.0))),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    visual.run_until_parked();
    let last_tool = bounds(visual, "activity-tool-overflow-23");
    assert!(
        scroll.contains(&last_tool.center()),
        "滚轮后末项应进入滚动视口：last={last_tool:?}, scroll={scroll:?}"
    );

    for selector in [
        "shell-navigation-close",
        "activity-tool-manager",
        "activity-shortcuts",
        "activity-settings",
    ] {
        let action = bounds(visual, selector);
        assert!(action.left() >= drawer.left() && action.right() <= drawer.right());
        assert!(action.top() >= drawer.top() && action.bottom() <= drawer.bottom());
    }
    click(visual, "activity-tool-overflow-23");
    assert!(
        visual
            .debug_bounds("shell-overflow-last-tool-page")
            .is_some()
    );
    assert!(visual.debug_bounds("shell-navigation-drawer").is_none());
}

/// 验证 760px 仍使用抽屉，扩到 761px 后桌面侧栏可立即导航。
#[gpui_kit::test]
fn navigation_switches_from_760px_drawer_to_761px_desktop_sidebar(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    cx.update(|app| app.set_reduce_motion(true));
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(TestTool {
        meta: ToolMeta::new("api", "API 测试", ""),
    }));
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let gate = Arc::new(DataSyncGate::default());
    let (_, visual) = cx.add_window_view(|window, cx| {
        let api = cx.new(|_| TestPage {
            selector: "shell-breakpoint-api-page",
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
            shell
        });
        Root::new(shell, window, cx)
    });
    visual.simulate_resize(size(px(760.0), px(640.0)));
    visual.run_until_parked();
    assert!(visual.debug_bounds("activity-bar").is_none());
    click(visual, "shell-navigation-toggle");
    visual.update(|window, cx| {
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
    let drawer = bounds(visual, "shell-navigation-drawer");
    assert_eq!(drawer.size, size(px(410.0), px(608.0)));
    assert!(visual.debug_bounds("activity-bar").is_some());

    visual.simulate_resize(size(px(761.0), px(640.0)));
    visual.run_until_parked();
    assert!(visual.debug_bounds("shell-navigation-drawer").is_none());
    assert!(visual.debug_bounds("activity-bar").is_some());
    click(visual, "activity-tool-api");
    assert!(visual.debug_bounds("shell-breakpoint-api-page").is_some());
    assert!(
        visual
            .debug_bounds("activity-selected-entry-button-tool-api")
            .is_some()
    );
}
