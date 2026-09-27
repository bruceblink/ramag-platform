#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;

use gpui_kit::{
    AppContext, Context, Entity, IntoElement, Modifiers, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, div, px, size,
};
use ramag_app::{DataSyncGate, StaticPluginHost, ToolRegistry};
use ramag_domain::{PluginDescriptor, PluginEntryDescriptor, PluginId, Tool, ToolMeta};

use super::{Shell, WindowBoundsPref};
use crate::HomeView;

struct DummyTool {
    meta: ToolMeta,
}

impl Tool for DummyTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

struct ShellHost {
    shell: Entity<Shell>,
}

impl Render for ShellHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.shell.clone())
    }
}

#[test]
fn window_bounds_reject_invalid_values() {
    assert!(
        WindowBoundsPref::parse(r#"{"x":10.0,"y":20.0,"w":1200.0,"h":780.0,"maximized":false}"#)
            .is_ok()
    );
    assert!(
        WindowBoundsPref::parse(r#"{"x":10.0,"y":20.0,"w":-1.0,"h":780.0,"maximized":false}"#)
            .is_err()
    );
    assert!(
        WindowBoundsPref::parse(
            r#"{"x":1000001.0,"y":20.0,"w":1200.0,"h":780.0,"maximized":false}"#
        )
        .is_err()
    );
    assert!(WindowBoundsPref::parse(&" ".repeat(WindowBoundsPref::MAX_PREF_BYTES + 1)).is_err());
}

#[gpui_kit::test]
fn standard_entry_view_is_created_only_after_activation(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = Arc::new(ToolRegistry::new());
    registry
        .register_plugin(
            PluginDescriptor::new(
                PluginId::new("lazy.plugin").expect("测试插件 ID 应有效"),
                "Lazy plugin",
                "lazy.entry",
            )
            .with_entries(vec![PluginEntryDescriptor::new("lazy.entry", "Lazy entry")]),
            Arc::new(DummyTool {
                meta: ToolMeta::new("lazy.entry", "Lazy entry", ""),
            }),
        )
        .expect("测试入口应注册");
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let gate = Arc::new(DataSyncGate::default());
    let mut shell_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let shell = cx.new(|cx| {
            Shell::new(
                registry.clone(),
                plugin_host.clone(),
                gate.clone(),
                window,
                cx,
            )
        });
        shell_entity = Some(shell.clone());
        ShellHost { shell }
    });
    let visual_cx: &mut VisualTestContext = visual_cx;
    visual_cx.simulate_resize(size(px(800.0), px(600.0)));
    visual_cx.run_until_parked();
    assert!(
        visual_cx
            .debug_bounds("plugin-entry-view-lazy.entry")
            .is_none()
    );

    shell_entity
        .expect("Shell 实体应创建")
        .update(visual_cx, |shell, cx| {
            shell.activate_for_test("lazy.entry", cx);
        });
    visual_cx.run_until_parked();
    assert!(
        visual_cx
            .debug_bounds("plugin-entry-view-lazy.entry")
            .is_some()
    );
}

/// 验证剪贴板入口同时出现在首页和活动栏，且首页点击能打开对应工具详情。
#[gpui_kit::test]
fn clipboard_is_visible_in_home_and_sidebar_and_opens_detail(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = Arc::new(ToolRegistry::new());
    registry
        .register_plugin(
            PluginDescriptor::new(
                PluginId::new("clipboard.plugin").expect("剪贴板插件 ID 应有效"),
                "剪贴板",
                "clipboard",
            )
            .with_entries(vec![PluginEntryDescriptor::new("clipboard", "剪贴板")]),
            Arc::new(DummyTool {
                meta: ToolMeta::new("clipboard", "剪贴板", "剪贴历史与快速粘贴"),
            }),
        )
        .expect("剪贴板入口应注册");
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let gate = Arc::new(DataSyncGate::default());
    let mut shell_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let home = cx.new(|cx| HomeView::new(registry.clone(), cx));
        let shell = cx.new(|cx| {
            let mut shell = Shell::new(
                registry.clone(),
                plugin_host.clone(),
                gate.clone(),
                window,
                cx,
            );
            shell.set_home_view(home, window, cx);
            shell
        });
        shell_entity = Some(shell.clone());
        ShellHost { shell }
    });
    let visual_cx: &mut VisualTestContext = visual_cx;
    visual_cx.simulate_resize(size(px(800.0), px(600.0)));
    visual_cx.run_until_parked();

    assert!(
        visual_cx.debug_bounds("activity-tool-clipboard").is_some(),
        "剪贴板入口应显示在侧边栏"
    );
    let card = visual_cx
        .debug_bounds("home-tool-clipboard")
        .expect("首页应显示剪贴板工具卡片");
    visual_cx.simulate_click(card.center(), Modifiers::default());
    visual_cx.run_until_parked();

    let selected = shell_entity
        .expect("Shell 实体应创建")
        .read_with(visual_cx, |shell, _| shell.selected.clone());
    assert_eq!(selected.as_deref(), Some("clipboard"));
    assert!(
        visual_cx
            .debug_bounds("plugin-entry-view-clipboard")
            .is_some(),
        "首页剪贴板卡片点击后应显示工具详情"
    );
}
