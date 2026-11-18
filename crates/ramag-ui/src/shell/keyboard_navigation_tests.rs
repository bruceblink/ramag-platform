//! 生产 Shell 抽屉的键盘焦点循环与入口激活验收。

use std::sync::Arc;

use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, Modifiers,
    Render, Styled as _, TestAppContext, VisualTestContext, Window, div, px, size,
};
use ramag_app::{DataSyncGate, StaticPluginHost, ToolRegistry};
use ramag_domain::{Tool, ToolMeta};

use super::super::Shell;

struct KeyboardTool {
    meta: ToolMeta,
}

impl Tool for KeyboardTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

struct KeyboardPage {
    selector: &'static str,
}

impl Render for KeyboardPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let selector = self.selector;
        div().size_full().debug_selector(move || selector.into())
    }
}

fn make_shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.update(gpui_kit::component::init);
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(KeyboardTool {
        meta: ToolMeta::new("api", "API 测试", "隔离的键盘导航测试工具"),
    }));
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let gate = Arc::new(DataSyncGate::default());
    let mut shell_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let api_page = cx.new(|_| KeyboardPage {
            selector: "shell-keyboard-api-page",
        });
        let shell = cx.new(|cx| {
            let mut shell = Shell::new(
                registry.clone(),
                plugin_host.clone(),
                gate.clone(),
                window,
                cx,
            );
            shell.register_tool_view("api", api_page.into());
            shell
        });
        shell_entity = Some(shell.clone());
        Root::new(shell, window, cx)
    });
    (shell_entity.expect("Shell 应创建"), visual)
}

/// 使用真实菜单按钮打开抽屉，再把焦点陷阱设为键盘遍历起点。
fn open_drawer(shell: &Entity<Shell>, visual: &mut VisualTestContext) {
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    let toggle = visual
        .debug_bounds("shell-navigation-toggle")
        .expect("窄屏菜单按钮应渲染");
    visual.simulate_click(toggle.center(), Modifiers::default());
    visual.run_until_parked();
    assert!(visual.debug_bounds("shell-navigation-drawer").is_some());
    assert!(visual.debug_bounds("activity-tool-manager").is_some());
    assert!(
        visual
            .debug_bounds("activity-entry-label-tool-manager")
            .is_some()
    );

    // 测试把实际抽屉句柄作为确定起点，不断言打开回调的帧时序。
    visual.update(|window, cx| {
        shell.update(cx, |shell, cx| shell.focus_navigation_drawer(window, cx));
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
}

/// 读取当前焦点，并确认它仍位于生产 Shell 注册的抽屉焦点陷阱内。
fn focused_in_drawer(visual: &mut VisualTestContext) -> FocusHandle {
    visual.update(|window, cx| {
        let trap = gpui_kit::base::active_focus_trap(window, cx).expect("导航抽屉应注册焦点陷阱");
        let focused = window.focused(cx).expect("键盘焦点应存在");
        assert!(trap.contains(&focused, window), "Tab 焦点不得离开导航抽屉");
        focused
    })
}

/// 按生产抽屉的 Tab 顺序把焦点移到有可见名称的工具管理按钮。
fn tab_to_manager(visual: &mut VisualTestContext) {
    for _ in 0..3 {
        visual.simulate_keystrokes("tab");
        focused_in_drawer(visual);
    }
}

/// 逐个真实 Tab，按按钮快照确认焦点到达 API 后才允许执行键盘激活。
fn tab_to_api(visual: &mut VisualTestContext) {
    let max_steps = visual.update(|window, _| {
        let drawer = window.within("shell-navigation-focus-trap");
        drawer.find_all_by_role(gpui_kit::Role::Button).len() + 1
    });
    let api_exists = visual.update(|window, _| {
        gpui_kit::base::test_support::find(window, &[], &gpui_kit::ElementId::from("tool-api"))
            .is_some()
    });
    assert!(api_exists, "真实 API Button 应能由 test_support::find 查询");

    let mut reached_api = false;
    for _ in 0..max_steps {
        visual.simulate_keystrokes("tab");
        focused_in_drawer(visual);
        let api_focused = visual.update(|window, _| {
            gpui_kit::base::test_support::find(window, &[], &gpui_kit::ElementId::from("tool-api"))
                .is_some_and(|button| {
                    assert_eq!(button.role(), Some(gpui_kit::Role::Button));
                    assert!(matches!(
                        button.label(),
                        Some("API 测试" | "API 测试，当前页面")
                    ));
                    button.focused() == Some(true)
                })
        });
        if api_focused {
            reached_api = true;
            break;
        }
    }

    assert!(
        reached_api,
        "Tab 在抽屉的 {max_steps} 个真实按钮范围内未到达 tool-api"
    );
}

/// 验证抽屉边界处的 Tab 与 Shift+Tab 循环，并确认 Escape 关闭后焦点回到菜单。
#[gpui_kit::test]
fn production_drawer_traps_both_tab_directions_and_escape_returns_focus(cx: &mut TestAppContext) {
    let (shell, visual) = make_shell(cx);
    open_drawer(&shell, visual);

    let drawer_focus = focused_in_drawer(visual);
    visual.simulate_keystrokes("shift-tab");
    let last_focus = focused_in_drawer(visual);
    assert_ne!(last_focus, drawer_focus, "Shift+Tab 应到达抽屉内的末端入口");

    visual.simulate_keystrokes("tab");
    let first_focus = focused_in_drawer(visual);
    assert_ne!(first_focus, last_focus, "Tab 应从末端入口循环到抽屉内部");

    visual.simulate_keystrokes("shift-tab");
    assert_eq!(
        focused_in_drawer(visual),
        last_focus,
        "Shift+Tab 应从首端入口循环回末端入口"
    );

    visual.simulate_keystrokes("escape");
    assert!(visual.debug_bounds("shell-navigation-drawer").is_none());
    let toggle_focus = shell.read_with(visual, |shell, _| shell.navigation_toggle_focus.clone());
    assert!(visual.update(|window, _| toggle_focus.is_focused(window)));
}

#[gpui_kit::test]
fn closing_drawer_before_next_frame_keeps_focus_on_toggle(cx: &mut TestAppContext) {
    let (shell, visual) = make_shell(cx);
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();

    let toggle = visual
        .debug_bounds("shell-navigation-toggle")
        .expect("窄屏菜单按钮应渲染");
    visual.simulate_click(toggle.center(), Modifiers::default());
    visual.run_until_parked();
    assert!(visual.debug_bounds("shell-navigation-drawer").is_some());

    // 在打开回调执行前关闭抽屉，再推进下一帧检查延迟焦点操作。
    let close = visual
        .debug_bounds("shell-navigation-close")
        .expect("打开抽屉后关闭按钮应渲染");
    visual.simulate_click(close.center(), Modifiers::default());
    visual.run_until_parked();
    assert!(visual.debug_bounds("shell-navigation-drawer").is_none());

    visual.update(|window, cx| {
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();

    let toggle_focus = shell.read_with(visual, |shell, _| shell.navigation_toggle_focus.clone());
    assert!(
        visual.update(|window, _| toggle_focus.is_focused(window)),
        "抽屉关闭后，待执行的打开回调不能把焦点移回隐藏抽屉"
    );
}

/// 验证带辅助名称的真实管理工具按钮可由 Enter 和 Space 激活。
#[gpui_kit::test]
fn production_drawer_manager_button_activates_with_enter_and_space(cx: &mut TestAppContext) {
    let (shell, visual) = make_shell(cx);

    open_drawer(&shell, visual);
    tab_to_manager(visual);
    visual.simulate_keystrokes("enter");
    assert!(visual.debug_bounds("tool-manager-body").is_some());
    assert!(visual.debug_bounds("shell-navigation-drawer").is_some());

    visual.simulate_keystrokes("escape");
    assert!(visual.debug_bounds("tool-manager-body").is_none());
    visual.simulate_keystrokes("escape");
    assert!(visual.debug_bounds("shell-navigation-drawer").is_none());

    open_drawer(&shell, visual);
    tab_to_manager(visual);
    visual.simulate_keystrokes("space");
    assert!(visual.debug_bounds("tool-manager-body").is_some());
    assert!(visual.debug_bounds("shell-navigation-drawer").is_some());
}

/// 验证 API Button 可由抽屉内 Tab 到达，并由 Enter 和 Space 激活。
#[gpui_kit::test]
fn production_drawer_api_button_is_tab_reachable_and_activates_with_enter_and_space(
    cx: &mut TestAppContext,
) {
    let (shell, visual) = make_shell(cx);

    open_drawer(&shell, visual);
    tab_to_api(visual);
    visual.update(|window, cx| window.press("enter", cx));
    assert_eq!(
        shell.read_with(visual, |shell, _| shell
            .selected
            .as_deref()
            .map(str::to_owned)),
        Some("api".to_owned()),
        "Enter 应激活当前聚焦的 API Button"
    );
    assert!(visual.debug_bounds("shell-keyboard-api-page").is_some());
    assert!(visual.debug_bounds("shell-navigation-drawer").is_none());
    assert_eq!(
        shell.read_with(visual, |shell, _| shell
            .selected
            .as_deref()
            .map(str::to_owned)),
        Some("api".to_owned())
    );

    open_drawer(&shell, visual);
    tab_to_api(visual);
    visual.update(|window, cx| window.press("space", cx));
    assert_eq!(
        shell.read_with(visual, |shell, _| shell
            .selected
            .as_deref()
            .map(str::to_owned)),
        Some("api".to_owned()),
        "Space 应激活当前聚焦的 API Button"
    );
    assert!(visual.debug_bounds("shell-keyboard-api-page").is_some());
    assert!(visual.debug_bounds("shell-navigation-drawer").is_none());
    assert_eq!(
        shell.read_with(visual, |shell, _| shell
            .selected
            .as_deref()
            .map(str::to_owned)),
        Some("api".to_owned())
    );
}
