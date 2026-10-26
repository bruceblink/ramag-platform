//! 设置页辅助页面的 headless 布局测试。

#![allow(clippy::expect_used)]

use super::*;
use gpui_kit::component::Root;
use gpui_kit::{Render, TestAppContext, px, size};

#[derive(Default)]
struct ManagedCardTestHost;

impl Render for ManagedCardTestHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("settings-managed-card-host")
            .debug_selector(|| "settings-managed-card-host".into())
            .size_full()
            .p(px(16.0))
            .child(super::managed_in_module_card("Git 配置", cx.theme()))
    }
}

struct SettingsNavigationTestHost {
    selected_page: SettingsPage,
    scroll: gpui_kit::ScrollHandle,
}

impl Render for SettingsNavigationTestHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let compact = settings_is_compact(window);
        let theme = cx.theme();
        let style = SettingsNavigationStyle {
            active: theme.list_active,
            foreground: theme.foreground,
            muted_foreground: theme.muted_foreground,
            hover: theme.list_hover,
            accent: theme.accent,
        };
        let children = SettingsPage::ALL
            .into_iter()
            .map(|page| {
                settings_navigation_item(
                    page,
                    page == self.selected_page,
                    false,
                    compact,
                    style,
                    |_, _, _| {},
                )
                .into_any_element()
            })
            .collect();
        // Mirror the Shell content wrapper so page changes are measured against
        // the same definite height as the production settings view.
        v_flex()
            .size_full()
            .child(div().h(px(44.0)).flex_none())
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_w_0()
                    .min_h_0()
                    .items_stretch()
                    .child(super::super::render_settings_layout(
                        compact,
                        settings_navigation_shell(
                            compact,
                            theme.sidebar,
                            theme.border,
                            children,
                            &self.scroll,
                        ),
                        div()
                            .id("settings-test-page")
                            .debug_selector(|| "settings-test-page".into())
                            .w_full()
                            .h(if self.selected_page == SettingsPage::Database {
                                px(900.0)
                            } else {
                                px(180.0)
                            }),
                    )),
            )
    }
}

#[gpui_kit::test]
fn settings_navigation_switches_to_scrollable_strip_on_compact_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual_cx) = cx.add_window_view(|_, _| SettingsNavigationTestHost {
        selected_page: SettingsPage::System,
        scroll: gpui_kit::ScrollHandle::new(),
    });

    for (width, height) in [(360.0, 520.0), (1024.0, 520.0), (1440.0, 520.0)] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();

        let navigation = visual_cx
            .debug_bounds("settings-navigation")
            .expect("设置导航应参与布局");
        let root = visual_cx
            .debug_bounds("settings-root")
            .expect("设置根布局应参与布局");
        let content = visual_cx
            .debug_bounds("settings-content")
            .expect("设置内容区应参与布局");
        let title = visual_cx
            .debug_bounds("settings-navigation-title")
            .expect("设置导航标题应渲染");
        let system = visual_cx
            .debug_bounds("settings-page-system")
            .expect("系统设置入口应渲染");
        let database = visual_cx
            .debug_bounds("settings-page-database")
            .expect("数据库入口应渲染");
        let update = visual_cx
            .debug_bounds("settings-page-update")
            .expect("关于入口应渲染");
        let plugins = visual_cx
            .debug_bounds("settings-page-plugins")
            .expect("插件入口应渲染");
        let tool_visibility = visual_cx
            .debug_bounds("settings-page-tool-visibility")
            .expect("工具显示入口应渲染");

        assert!(navigation.origin.x >= px(0.0));
        assert!(navigation.right() <= px(width));
        assert!(root.origin.x >= px(0.0));
        assert!(root.right() <= px(width));
        assert!(root.bottom() <= px(height));
        assert_eq!(navigation.origin.y, root.origin.y);
        assert!(content.origin.x >= root.origin.x);
        assert!(content.right() <= root.right());
        assert!(content.bottom() <= root.bottom());
        assert!(title.origin.x >= navigation.origin.x);
        assert!(system.origin.x >= navigation.origin.x);
        assert!(system.origin.y >= navigation.origin.y);
        if width < 900.0 {
            assert!(navigation.size.height <= px(66.0));
            assert!(content.origin.y >= navigation.bottom());
            assert!(system.size.width >= px(SETTINGS_COMPACT_NAV_ITEM_WIDTH));
            assert!(database.size.width >= px(SETTINGS_COMPACT_NAV_ITEM_WIDTH));
            assert!(system.right() <= navigation.right());
            assert!(system.origin.x > title.origin.x);
            assert!(plugins.size.width >= px(SETTINGS_COMPACT_NAV_ITEM_WIDTH));
            assert!(tool_visibility.size.width >= px(SETTINGS_COMPACT_NAV_ITEM_WIDTH));
        } else {
            assert!(navigation.size.width <= px(220.0));
            assert_eq!(navigation.bottom(), root.bottom());
            assert!(content.origin.x >= navigation.right());
            assert!(system.right() <= navigation.right());
            assert!(plugins.right() <= navigation.right());
            assert!(tool_visibility.right() <= navigation.right());
            assert!(update.right() <= navigation.right());
            assert!(update.origin.y > system.origin.y);
        }
    }
}

#[gpui_kit::test]
fn managed_settings_card_keeps_empty_state_hierarchy(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|_| ManagedCardTestHost);
        Root::new(view, window, cx)
    });

    for mode in [crate::Mode::Light, crate::Mode::Dark] {
        visual_cx.update(|_, app| crate::apply_theme(mode, app));
        for width in [360.0, 1024.0, 1440.0] {
            visual_cx.simulate_resize(size(px(width), px(420.0)));
            visual_cx.run_until_parked();

            let host = visual_cx
                .debug_bounds("settings-managed-card-host")
                .expect("设置空配置测试容器应渲染");
            let card = visual_cx
                .debug_bounds("settings-managed-card")
                .expect("设置空配置卡应渲染");
            let heading = visual_cx
                .debug_bounds("settings-managed-card-heading")
                .expect("设置空配置标题行应渲染");
            let message = visual_cx
                .debug_bounds("settings-managed-card-message")
                .expect("设置空配置说明应渲染");
            let guidance = visual_cx
                .debug_bounds("settings-managed-card-guidance")
                .expect("设置空配置引导应渲染");

            assert!(card.origin.x >= host.origin.x);
            assert!(card.origin.y >= host.origin.y);
            assert!(card.right() <= host.right());
            assert!(card.bottom() <= host.bottom());
            assert!(heading.origin.y >= card.origin.y);
            assert!(message.origin.y >= heading.bottom());
            assert!(guidance.origin.y >= message.bottom());
        }
    }
}

/// 内容较少时滚动范围必须为零；窗口变矮时保持行高并允许滚到最后一项。
#[gpui_kit::test]
fn settings_navigation_only_scrolls_when_content_overflows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (host, cx) = cx.add_window_view(|_, _| SettingsNavigationTestHost {
        selected_page: SettingsPage::System,
        scroll: gpui_kit::ScrollHandle::new(),
    });
    for (width, height) in [(1440.0, 900.0), (1024.0, 768.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        cx.run_until_parked();
        let offset = host.read_with(cx, |host, _| host.scroll.max_offset());
        assert_eq!(offset.y, px(0.0), "完整导航无需滚动，不应出现伪滚动条");
    }
    cx.simulate_resize(size(px(1024.0), px(280.0)));
    cx.run_until_parked();
    let max_offset = host.read_with(cx, |host, _| host.scroll.max_offset());
    assert!(max_offset.y > px(0.0), "低窗口需要真实纵向滚动");
    host.update(cx, |host, cx| {
        host.scroll
            .set_offset(gpui_kit::point(px(0.0), -max_offset.y));
        cx.notify();
    });
    cx.run_until_parked();
    let last = cx
        .debug_bounds("settings-page-update")
        .expect("最后一项存在");
    let nav = cx.debug_bounds("settings-navigation").expect("导航存在");
    assert_eq!(last.size.height, px(38.0), "低窗口不能压扁导航项");
    assert!(last.bottom() <= nav.bottom(), "滚动后最后一项可访问");
    cx.simulate_resize(size(px(1440.0), px(900.0)));
    cx.run_until_parked();
    assert_eq!(
        host.read_with(cx, |host, _| host.scroll.max_offset().y),
        px(0.0)
    );
}

/// 切换到数据库客户端后，导航列仍与设置根布局保持顶部和底部对齐。
#[gpui_kit::test]
fn settings_navigation_stays_aligned_when_database_page_is_selected(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (host, visual_cx) = cx.add_window_view(|_, _| SettingsNavigationTestHost {
        selected_page: SettingsPage::System,
        scroll: gpui_kit::ScrollHandle::new(),
    });
    visual_cx.simulate_resize(size(px(1024.0), px(520.0)));
    visual_cx.run_until_parked();

    let initial_root = visual_cx
        .debug_bounds("settings-root")
        .expect("设置根布局应渲染");
    let initial_navigation = visual_cx
        .debug_bounds("settings-navigation")
        .expect("设置导航应渲染");

    host.update(visual_cx, |host, cx| {
        host.selected_page = SettingsPage::Database;
        cx.notify();
    });
    visual_cx.run_until_parked();

    let root = visual_cx
        .debug_bounds("settings-root")
        .expect("设置根布局应渲染");
    let navigation = visual_cx
        .debug_bounds("settings-navigation")
        .expect("设置导航应渲染");
    let database = visual_cx
        .debug_bounds("settings-page-database")
        .expect("数据库客户端入口应渲染");

    assert_eq!(navigation.origin.y, root.origin.y);
    assert_eq!(navigation.bottom(), root.bottom());
    assert_eq!(root.size.height, initial_root.size.height);
    assert_eq!(navigation.size.height, initial_navigation.size.height);
    assert!(database.origin.y >= navigation.origin.y);
    assert!(database.bottom() <= navigation.bottom());
}
