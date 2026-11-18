//! 活动栏的 headless 布局验收，覆盖动态工具列表的低高度窗口边界。
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;

use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, Modifiers, ParentElement, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, div, px, size,
};
use ramag_app::{StaticPluginHost, ToolRegistry};
use ramag_domain::{PluginDescriptor, PluginId, Tool, ToolMeta};

use crate::tool_layout::{ToolDragGlobal, ToolDragSurface, ToolDropSide, ToolDropTarget};

use super::ActivityBar;

struct ActivityBarHost {
    bar: Entity<ActivityBar>,
}

impl Render for ActivityBarHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.bar.clone())
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

fn registry_with_tools(count: usize) -> Arc<ToolRegistry> {
    // Build only metadata-backed tools so the test isolates activity-bar layout.
    let registry = Arc::new(ToolRegistry::new());
    for index in 0..count {
        registry.register(Arc::new(TestTool {
            meta: ToolMeta::new(
                format!("activity-test-{index}"),
                format!("Test tool {index}"),
                "",
            ),
        }));
    }
    registry
}

fn host_with_registration_failure(registry: Arc<ToolRegistry>) -> Arc<StaticPluginHost> {
    let host = Arc::new(StaticPluginHost::new(registry));
    let tool = Arc::new(TestTool {
        meta: ToolMeta::new("actual-entry", "Actual entry", ""),
    });
    let descriptor = PluginDescriptor::new(
        PluginId::new("broken-plugin").expect("测试插件 ID 应有效"),
        "失败插件",
        "declared-entry",
    );
    host.register_plugin(Arc::new(ramag_app::StaticPluginAdapter::new(
        descriptor, tool,
    )))
    .expect_err("入口 ID 不一致应被拒绝");
    host
}

fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let bounds = visual.debug_bounds(selector);
    assert!(bounds.is_some(), "missing activity element: {selector}");
    visual.simulate_click(bounds.unwrap_or_default().center(), Modifiers::default());
    visual.run_until_parked();
}

/// 真实活动栏视图：工具项超出可视高度时，滚动区收缩而固定入口保持可见。
#[gpui_kit::test]
fn activity_bar_keeps_fixed_actions_visible_when_tool_list_overflows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_tools(16);
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let (_, cx) = cx
        .add_window_view(move |_, cx| ActivityBar::new(registry.clone(), plugin_host.clone(), cx));
    let cx: &mut VisualTestContext = cx;
    cx.simulate_resize(size(px(360.0), px(220.0)));
    cx.run_until_parked();

    let bar = cx.debug_bounds("activity-bar");
    assert!(bar.is_some(), "活动栏应渲染");
    let bar = bar.unwrap_or_default();
    assert_eq!(bar.size.width, px(332.0));
    let brand = cx
        .debug_bounds("activity-brand")
        .expect("短窗口标题行应渲染");
    let title = cx
        .debug_bounds("activity-navigation-title")
        .expect("窄屏抽屉应显示“全部工具”标题");
    assert!(title.origin.x >= brand.origin.x && title.right() <= brand.right());
    assert!(cx.debug_bounds("activity-brand-mark").is_none());
    assert!(cx.debug_bounds("activity-brand-name").is_none());
    assert_eq!(brand.origin.y, px(8.0));
    let scroll = cx.debug_bounds("activity-tool-scroll");
    assert!(scroll.is_some(), "工具列表滚动区应渲染");
    let scroll = scroll.unwrap_or_default();
    let manager = cx.debug_bounds("activity-tool-manager");
    assert!(manager.is_some(), "工具管理入口应渲染");
    let manager = manager.unwrap_or_default();
    let shortcuts = cx.debug_bounds("activity-shortcuts");
    assert!(shortcuts.is_some(), "快捷键入口应渲染");
    let shortcuts = shortcuts.unwrap_or_default();
    let settings = cx.debug_bounds("activity-settings");
    assert!(settings.is_some(), "设置入口应渲染");
    let settings = settings.unwrap_or_default();
    let last_tool = cx.debug_bounds("activity-tool-activity-test-15");
    assert!(last_tool.is_some(), "工具列表末项应渲染");
    let last_tool = last_tool.unwrap_or_default();

    // 短窗口的三个固定入口都保持 32px 高，并完整留在可见侧栏内。
    for fixed_action in [manager, shortcuts, settings] {
        assert_eq!(fixed_action.size.height, px(32.0));
        assert!(fixed_action.size.width > px(0.0));
        assert!(fixed_action.origin.y >= bar.origin.y);
        assert!(fixed_action.bottom() <= bar.bottom());
    }
    assert!(scroll.origin.y >= bar.origin.y);
    assert!(scroll.bottom() <= bar.bottom());
    assert!(last_tool.bottom() > scroll.bottom());
    assert_eq!(
        cx.debug_bounds("activity-command-section")
            .unwrap()
            .bottom(),
        bar.bottom() - px(8.0)
    );
    assert!(cx.debug_bounds("activity-sidebar-footer").is_none());
}

/// 短窗口中的完整侧栏仍保留滚动工具区和三个固定命令入口。
#[gpui_kit::test]
fn activity_bar_footer_keeps_short_height_tools_and_actions_reachable(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_tools(16);
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let (_, visual) = cx
        .add_window_view(move |_, cx| ActivityBar::new(registry.clone(), plugin_host.clone(), cx));
    visual.simulate_resize(size(px(1101.0), px(220.0)));
    visual.run_until_parked();

    let bar = visual
        .debug_bounds("activity-bar")
        .expect("短窗口活动栏应渲染");
    let footer = visual
        .debug_bounds("activity-sidebar-footer")
        .expect("完整文字侧栏应显示页脚");
    assert_eq!(footer.bottom(), bar.bottom() - px(8.0));

    let command_section = visual
        .debug_bounds("activity-command-section")
        .expect("固定命令区应渲染");
    assert!(command_section.bottom() <= footer.origin.y);
    for selector in [
        "activity-tool-manager",
        "activity-shortcuts",
        "activity-settings",
    ] {
        let action = visual.debug_bounds(selector);
        assert!(action.is_some(), "固定命令入口应可见：{selector}");
        let action = action.unwrap_or_default();
        assert!(action.origin.y >= bar.origin.y);
        assert!(action.bottom() <= command_section.bottom());
    }

    let scroll = visual
        .debug_bounds("activity-tool-scroll")
        .expect("工具滚动区应保留");
    assert!(scroll.size.height > px(0.0));
    assert!(scroll.bottom() <= command_section.origin.y);
    let last_tool = visual
        .debug_bounds("activity-tool-activity-test-15")
        .expect("工具列表末项应渲染");
    assert!(last_tool.bottom() > scroll.bottom());
}

/// API 与数据库工作区只在 761–1100px 使用图标栏；更窄时抽屉显示完整文字。
#[gpui_kit::test]
fn activity_bar_compact_workbench_layout_obeys_prototype_width_boundaries(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(TestTool {
        meta: ToolMeta::new("api", "API 测试", ""),
    }));
    registry.register(Arc::new(TestTool {
        meta: ToolMeta::new("dbclient", "数据库客户端", ""),
    }));
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let mut bar_entity: Option<Entity<ActivityBar>> = None;
    let (_, visual) = cx.add_window_view(|_, cx| {
        let bar = cx.new(|cx| ActivityBar::new(registry.clone(), plugin_host.clone(), cx));
        bar_entity = Some(bar.clone());
        ActivityBarHost { bar }
    });
    let bar = bar_entity.expect("活动栏实体应创建");
    assert_eq!(super::COMPACT_BAR_WIDTH, 62.0);
    assert_eq!(super::COMPACT_CONTENT_WIDTH, 54.0);
    for (width, expected_width) in [
        (759.0, 410.0),
        (760.0, 410.0),
        (761.0, super::COMPACT_BAR_WIDTH),
        (1099.0, super::COMPACT_BAR_WIDTH),
        (1100.0, super::COMPACT_BAR_WIDTH),
        (1101.0, super::BAR_WIDTH),
    ] {
        visual.simulate_resize(size(px(width), px(640.0)));
        visual.update(|_, cx| {
            bar.update(cx, |bar, cx| {
                bar.set_selected(crate::NavTarget::Tool("api".into()), cx);
            });
            cx.set_global(ToolDragGlobal {
                dragged_id: Some("api".into()),
                source_index: 0,
                target: Some(ToolDropTarget {
                    surface: ToolDragSurface::ActivityBar,
                    index: 1,
                    side: ToolDropSide::Bottom,
                }),
                revision: 1,
            });
        });
        visual.run_until_parked();
        let bounds = visual
            .debug_bounds("activity-bar")
            .expect("活动栏应保持可见");
        assert_eq!(bounds.size.width, px(expected_width), "width {width}");
        let brand = visual
            .debug_bounds("activity-brand")
            .expect("品牌或标题行应渲染");
        assert_eq!(brand.origin.y, px(18.0), "brand top at width {width}");
        assert_eq!(brand.size.height, px(32.0), "brand row at width {width}");
        let narrow_drawer = width <= crate::shell::SHELL_NARROW_BREAKPOINT;
        assert_eq!(
            visual.debug_bounds("activity-navigation-title").is_some(),
            narrow_drawer,
            "navigation title visibility at width {width}"
        );
        assert_eq!(
            visual.debug_bounds("activity-brand-mark").is_some(),
            !narrow_drawer,
            "brand mark visibility at width {width}"
        );
        assert_eq!(
            visual.debug_bounds("activity-brand-name").is_some(),
            !narrow_drawer && expected_width != super::COMPACT_BAR_WIDTH,
            "brand label visibility at width {width}"
        );
        let complete_sidebar = !narrow_drawer && expected_width != super::COMPACT_BAR_WIDTH;
        let footer = visual.debug_bounds("activity-sidebar-footer");
        assert_eq!(
            footer.is_some(),
            complete_sidebar,
            "footer visibility at width {width}"
        );
        if let Some(footer) = footer {
            assert_eq!(footer.bottom(), bounds.bottom() - px(12.0));
        }
        if !narrow_drawer {
            assert_eq!(
                visual.debug_bounds("activity-brand-mark").unwrap().size,
                size(px(28.0), px(28.0))
            );
        }
        assert_eq!(
            visual.debug_bounds("activity-home").unwrap().size.height,
            px(super::ACTIVITY_ITEM_HEIGHT)
        );
        assert_eq!(
            visual
                .debug_bounds("activity-tool-api")
                .unwrap()
                .size
                .height,
            px(super::ACTIVITY_ITEM_HEIGHT)
        );
        let command_section = visual
            .debug_bounds("activity-command-section")
            .expect("底部命令区应渲染");
        if let Some(footer) = visual.debug_bounds("activity-sidebar-footer") {
            assert!(command_section.bottom() <= footer.origin.y);
        } else {
            assert_eq!(command_section.bottom(), bounds.bottom() - px(12.0));
        }
        let manager = visual
            .debug_bounds("activity-tool-manager")
            .expect("管理工具入口应位于底部命令区");
        let shortcuts = visual
            .debug_bounds("activity-shortcuts")
            .expect("快捷键入口应位于底部命令区");
        let settings = visual
            .debug_bounds("activity-settings")
            .expect("设置入口应位于底部命令区");
        if let Some(footer) = visual.debug_bounds("activity-sidebar-footer") {
            assert!(settings.bottom() <= footer.origin.y);
        }
        let command_height = if expected_width == super::COMPACT_BAR_WIDTH {
            32.0
        } else {
            super::ACTIVITY_ITEM_HEIGHT
        };
        // 几何断言同时覆盖桌面文字导航和紧凑图标导航的入口尺寸。
        for action in [manager, shortcuts, settings] {
            assert_eq!(action.size.height, px(command_height));
            assert!(action.size.width > px(0.0));
            assert!(action.origin.y >= bounds.origin.y);
            assert!(action.bottom() <= bounds.bottom());
        }
        assert!(manager.origin.y >= command_section.origin.y + px(15.0));
        assert!(visual.debug_bounds("activity-tool-api").is_some());
        let api_button = visual
            .debug_bounds("activity-selected-entry-button-tool-api")
            .expect("选中的 API 按钮应使用选中态选择器");
        assert!(
            visual
                .debug_bounds("activity-entry-button-tool-dbclient")
                .is_some(),
            "未选中的数据库入口应保留普通选择器"
        );
        assert!(
            visual
                .debug_bounds("activity-selected-marker-tool-api")
                .is_none(),
            "选中入口不应再绘制独立竖线"
        );
        let api_icon = visual
            .debug_bounds("activity-entry-icon-tool-api")
            .expect("API 图标应渲染");
        let manager_button = visual
            .debug_bounds("activity-tool-manager")
            .expect("管理工具按钮应渲染");
        let manager_icon = visual
            .debug_bounds("activity-entry-icon-tool-manager")
            .expect("管理工具图标应渲染");
        if expected_width == super::COMPACT_BAR_WIDTH {
            assert!(
                (f32::from(api_icon.center().x - bounds.center().x)).abs() <= 0.5,
                "紧凑栏中的 API 图标应水平居中"
            );
            assert!(
                visual
                    .debug_bounds("activity-entry-label-tool-api")
                    .is_none()
            );
            assert!(
                (f32::from(manager_icon.center().x - bounds.center().x)).abs() <= 0.5,
                "紧凑栏中的管理工具图标应水平居中"
            );
            assert!(
                visual
                    .debug_bounds("activity-entry-label-tool-manager")
                    .is_none()
            );
        } else {
            let label = visual
                .debug_bounds("activity-entry-label-tool-api")
                .expect("桌面导航应显示文字标签");
            let manager_label = visual
                .debug_bounds("activity-entry-label-tool-manager")
                .expect("桌面管理工具入口应显示文字标签");
            let icon_left_in_button = f32::from(api_icon.origin.x - api_button.origin.x);
            let manager_icon_left_in_button =
                f32::from(manager_icon.origin.x - manager_button.origin.x);
            assert!(
                (icon_left_in_button - 10.0).abs() <= 1.0,
                "桌面导航图标应从按钮左侧内边距开始：button={api_button:?}, icon={api_icon:?}"
            );
            assert_eq!(manager_button.size.width, api_button.size.width);
            assert!(
                (manager_icon_left_in_button - 10.0).abs() <= 1.0,
                "管理工具图标应与其他导航入口左对齐：button={manager_button:?}, icon={manager_icon:?}"
            );
            assert_eq!(manager_icon.origin.x, api_icon.origin.x);
            assert_eq!(manager_label.origin.x, label.origin.x);
            assert!(
                label.origin.x >= api_icon.right(),
                "桌面导航标签应位于图标右侧：icon={api_icon:?}, label={label:?}"
            );
            assert!(
                label.origin.x < api_button.center().x,
                "桌面导航标签应从按钮左侧开始，而不是整体居中：button={api_button:?}, label={label:?}"
            );
        }
        let expected_content_width = if expected_width == super::COMPACT_BAR_WIDTH {
            super::COMPACT_CONTENT_WIDTH
        } else {
            expected_width - 20.0
        };
        let indicator = visual
            .debug_bounds("activity-tool-drop-indicator")
            .expect("拖动时应显示插入线");
        assert_eq!(indicator.size.width, px(expected_content_width));
        assert_eq!(indicator.size.height, px(2.0));
        let first_tool = visual
            .debug_bounds("activity-tool-api")
            .expect("工具列表首项应渲染");
        assert_eq!(
            f32::from(indicator.origin.y - first_tool.origin.y),
            2.0 * (super::ACTIVITY_ITEM_HEIGHT + crate::tool_layout::ACTIVITY_ITEM_GAP)
                - crate::tool_layout::ACTIVITY_ITEM_GAP / 2.0
        );
    }

    visual.simulate_resize(size(px(1024.0), px(640.0)));
    visual.update(|_, cx| {
        bar.update(cx, |bar, cx| {
            bar.set_selected(crate::NavTarget::Home, cx);
        });
    });
    visual.run_until_parked();
    assert!(
        visual.debug_bounds("activity-sidebar-footer").is_some(),
        "1024px 首页保留完整文字侧栏，应显示页脚"
    );
}

/// 确认 API 导航在桌面、紧凑栏和抽屉布局中都显示图标。
#[gpui_kit::test]
fn api_navigation_icon_renders_across_prototype_breakpoints(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(TestTool {
        meta: ToolMeta::new("api", "API 测试", ""),
    }));
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let mut bar_entity: Option<Entity<ActivityBar>> = None;
    let (_, visual) = cx.add_window_view(|_, cx| {
        let bar = cx.new(|cx| ActivityBar::new(registry.clone(), plugin_host.clone(), cx));
        bar_entity = Some(bar.clone());
        ActivityBarHost { bar }
    });
    let Some(bar) = bar_entity else {
        unreachable!("活动栏实体应创建")
    };

    for width in [1440.0, 900.0, 760.0, 360.0] {
        visual.simulate_resize(size(px(width), px(640.0)));
        visual.update(|_, cx| {
            bar.update(cx, |bar, cx| {
                bar.set_selected(crate::NavTarget::Tool("api".into()), cx);
            });
        });
        visual.run_until_parked();

        assert!(visual.debug_bounds("activity-tool-api").is_some());
        let icon = visual
            .debug_bounds("activity-entry-icon-tool-api")
            .expect("API 导航入口应绘制图标");
        assert_eq!(icon.size, size(px(16.0), px(16.0)));
        assert_eq!(
            visual.debug_bounds("activity-navigation-title").is_some(),
            width <= crate::shell::SHELL_NARROW_BREAKPOINT,
            "抽屉标题断点不匹配：width={width}"
        );
    }
}

#[gpui_kit::test]
fn activity_bar_shows_plugin_failure_badge(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_tools(1);
    let plugin_host = host_with_registration_failure(registry.clone());
    let (_, visual_cx) = cx
        .add_window_view(move |_, cx| ActivityBar::new(registry.clone(), plugin_host.clone(), cx));
    visual_cx.simulate_resize(size(px(360.0), px(520.0)));
    visual_cx.run_until_parked();

    let settings = visual_cx
        .debug_bounds("activity-settings")
        .expect("设置入口应渲染");
    let badge = visual_cx
        .debug_bounds("activity-settings-badge")
        .expect("插件故障时设置入口应显示角标");
    assert!(badge.origin.x >= settings.origin.x);
    assert!(badge.right() <= settings.right());
    assert!(badge.origin.y >= settings.origin.y);
    assert!(badge.bottom() <= settings.bottom());
}

/// 验证 196px 侧栏打开管理弹层后，弹层和开关仍留在窄窗口视口内。
#[gpui_kit::test]
fn activity_bar_manager_popover_fits_narrow_window(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = registry_with_tools(3);
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let (_, visual) = cx
        .add_window_view(move |_, cx| ActivityBar::new(registry.clone(), plugin_host.clone(), cx));
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();

    click(visual, "activity-tool-manager");
    for selector in [
        "tool-manager-body",
        "tool-manager-search",
        "tool-manager-scroll",
        "tool-manager-home-control-activity-test-0",
        "tool-manager-sidebar-control-activity-test-0",
    ] {
        let bounds = visual.debug_bounds(selector);
        assert!(bounds.is_some(), "missing manager element: {selector}");
        let bounds = bounds.unwrap_or_default();
        assert!(bounds.left() >= px(0.0), "left edge of {selector}");
        assert!(bounds.right() <= px(360.0), "right edge of {selector}");
        assert!(bounds.top() >= px(0.0), "top edge of {selector}");
        assert!(bounds.bottom() <= px(640.0), "bottom edge of {selector}");
    }
}
