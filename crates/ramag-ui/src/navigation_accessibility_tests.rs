//! 检查桌面活动栏和窄屏底部导航的真实按钮辅助名称。

use std::sync::Arc;

use gpui_kit::{ElementId, Modifiers, TestAppContext, VisualTestContext, Window, px, size};
use ramag_app::{DataSyncGate, StaticPluginHost, ToolRegistry};
use ramag_domain::{Tool, ToolMeta};

use crate::shell::Shell;

struct NavigationTestTool {
    meta: ToolMeta,
}

impl Tool for NavigationTestTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

fn accessible_name(window: &Window, id: &'static str) -> String {
    let element_id = ElementId::from(id);
    let Some(snapshot) = gpui_kit::base::test_support::find(window, &[], &element_id) else {
        unreachable!("应找到导航按钮节点：{id}");
    };
    let Some(label) = snapshot.label() else {
        unreachable!("导航按钮应有辅助名称：{id}");
    };
    label.to_owned()
}

fn names(visual: &mut VisualTestContext, ids: [&'static str; 3]) -> [String; 3] {
    visual.update(|window, _| ids.map(|id| accessible_name(window, id)))
}

/// 选中状态只追加到辅助名称；可见名称和未选中项的辅助名称保持原值。
#[gpui_kit::test]
fn shell_navigation_names_announce_current_page_on_desktop_and_compact_bar(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::component::init);
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(NavigationTestTool {
        meta: ToolMeta::new("api", "API", "API tool"),
    }));
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let data_sync_gate = Arc::new(DataSyncGate::default());
    let (_, visual) = cx.add_window_view(move |window, cx| {
        Shell::new(
            registry.clone(),
            plugin_host.clone(),
            data_sync_gate.clone(),
            window,
            cx,
        )
    });
    let visual: &mut VisualTestContext = visual;

    visual.simulate_resize(size(px(1440.0), px(900.0)));
    visual.run_until_parked();
    assert_eq!(
        names(visual, ["home", "tool-api", "settings"]),
        [
            "工具总览，当前页面".to_owned(),
            "API".to_owned(),
            "设置".to_owned(),
        ]
    );

    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    assert_eq!(
        names(
            visual,
            [
                "compact-nav-home",
                "compact-nav-api",
                "compact-nav-settings",
            ],
        ),
        [
            "总览，当前页面".to_owned(),
            "API".to_owned(),
            "设置".to_owned(),
        ]
    );

    let Some(api_button) = visual.debug_bounds("shell-compact-nav-api") else {
        unreachable!("窄屏底部 API 入口应可点击");
    };
    visual.simulate_click(api_button.center(), Modifiers::default());
    visual.run_until_parked();
    assert_eq!(
        names(
            visual,
            [
                "compact-nav-home",
                "compact-nav-api",
                "compact-nav-settings",
            ],
        ),
        [
            "总览".to_owned(),
            "API，当前页面".to_owned(),
            "设置".to_owned(),
        ]
    );

    visual.simulate_resize(size(px(1440.0), px(900.0)));
    visual.run_until_parked();
    assert_eq!(
        names(visual, ["home", "tool-api", "settings"]),
        [
            "工具总览".to_owned(),
            "API，当前页面".to_owned(),
            "设置".to_owned(),
        ]
    );
}

#[gpui_kit::test]
fn shell_theme_toggle_announces_the_theme_it_will_apply(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = Arc::new(ToolRegistry::new());
    let host = Arc::new(StaticPluginHost::new(registry.clone()));
    let gate = Arc::new(DataSyncGate::default());
    let (_, visual) = cx.add_window_view(move |window, cx| {
        Shell::new(registry.clone(), host.clone(), gate.clone(), window, cx)
    });
    visual.update(|_, cx| crate::theme::apply_theme(crate::theme::Mode::Dark, cx));
    visual.run_until_parked();
    assert_eq!(
        visual.update(|window, _| accessible_name(window, "shell-theme-toggle")),
        "切换到亮色主题"
    );

    let Some(toggle) = visual.debug_bounds("shell-theme-toggle") else {
        unreachable!("顶栏应显示主题按钮");
    };
    visual.simulate_click(toggle.center(), Modifiers::default());
    visual.run_until_parked();
    assert_eq!(
        visual.update(|window, _| accessible_name(window, "shell-theme-toggle")),
        "切换到暗色主题"
    );
}
