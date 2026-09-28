use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _, h_flex, scroll::ScrollableElement as _, v_flex,
};
use gpui_kit::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, hsla, prelude::*, px,
};

use super::{SETTINGS_COMPACT_NAV_ITEM_WIDTH, SettingsPage, SettingsView, settings_is_compact};

impl SettingsPage {
    fn icon(self) -> Icon {
        match self {
            Self::System => crate::icons::settings(),
            Self::Monitor => crate::icons::gauge(),
            Self::Database => crate::icons::database(),
            Self::VersionControl => crate::icons::git_branch(),
            Self::Ssh => crate::activity_bar::ActivityBar::icon_for_tool("ssh"),
            Self::ObjectStorage => {
                crate::activity_bar::ActivityBar::icon_for_tool("object_storage")
            }
            Self::Plugins => crate::activity_bar::ActivityBar::icon_for_tool("plugin"),
            Self::Update => Icon::new(IconName::Info),
            Self::Clipboard => crate::icons::clipboard(),
        }
    }
}

impl SettingsView {
    pub(super) fn render_navigation(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let compact = settings_is_compact(window);
        let theme = cx.theme();
        let style = SettingsNavigationStyle {
            active: theme.list_active,
            foreground: theme.foreground,
            muted_foreground: theme.muted_foreground,
            hover: theme.list_hover,
            accent: theme.accent,
        };
        let mut children = Vec::new();
        let update_available =
            cx.read_global::<crate::activity_bar::UpdateIndicatorGlobal, _>(|state, _| {
                state.available
            });

        for &page in SettingsPage::ALL
            .iter()
            .filter(|&&page| page != SettingsPage::Clipboard || self.clipboard_service.is_some())
        {
            let selected = self.selected_page == page;
            children.push(
                settings_navigation_item(
                    page,
                    selected,
                    update_available && page == SettingsPage::Update,
                    compact,
                    style,
                    cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if this.selected_page != page {
                            if this
                                .selected_page
                                .clears_database_test_when_switching_to(page)
                            {
                                this.clear_database_converter_test(window, cx);
                            }
                            this.selected_page = page;
                            this.page_scroll
                                .set_offset(gpui_kit::point(px(0.0), px(0.0)));
                            cx.notify();
                        }
                    }),
                )
                .into_any_element(),
            );
        }
        settings_navigation_shell(
            compact,
            theme.sidebar,
            theme.border,
            children,
            &self.navigation_scroll,
        )
    }

    pub(super) fn render_selected_page(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let compact = settings_is_compact(window);
        let page = self.selected_page;
        let content = match page {
            SettingsPage::System => self.render_system_page(cx),
            SettingsPage::Monitor => self.render_monitor_page(cx),
            SettingsPage::Database => self.render_database_page(cx),
            SettingsPage::VersionControl => managed_in_module_card("Git 配置", cx),
            SettingsPage::Ssh => self.render_ssh_page(cx),
            SettingsPage::ObjectStorage => managed_in_module_card("账号与 Bucket", cx),
            SettingsPage::Plugins => self.plugin_diagnostics.clone().into_any_element(),
            SettingsPage::Update => self.render_update_page(cx),
            SettingsPage::Clipboard => self.render_clipboard_page(cx),
        };

        v_flex()
            .size_full()
            .id("settings-page-scroll")
            .overflow_y_scroll()
            .track_scroll(&self.page_scroll)
            .vertical_scrollbar(&self.page_scroll)
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(820.0))
                    .mx_auto()
                    .p(px(if compact { 16.0 } else { 28.0 }))
                    .gap(px(if compact { 16.0 } else { 24.0 }))
                    .child(
                        v_flex()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .child(page.title()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(page.description()),
                            ),
                    )
                    .child(content),
            )
            .into_any_element()
    }
}

#[derive(Clone, Copy)]
struct SettingsNavigationStyle {
    active: gpui_kit::Hsla,
    foreground: gpui_kit::Hsla,
    muted_foreground: gpui_kit::Hsla,
    hover: gpui_kit::Hsla,
    accent: gpui_kit::Hsla,
}

fn settings_navigation_shell(
    compact: bool,
    sidebar: gpui_kit::Hsla,
    border: gpui_kit::Hsla,
    children: Vec<AnyElement>,
    scroll: &gpui_kit::ScrollHandle,
) -> impl IntoElement {
    let title = div()
        .id("settings-navigation-title")
        .debug_selector(|| "settings-navigation-title".into())
        .when(compact, |title| title.w(px(56.0)).flex_none())
        .when(!compact, |title| {
            title.px(px(10.0)).pt(px(6.0)).pb(px(14.0))
        })
        .text_lg()
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .child("设置");
    if compact {
        h_flex()
            .id("settings-navigation")
            .debug_selector(|| "settings-navigation".into())
            .w_full()
            .h(px(66.0))
            .flex_none()
            .items_start()
            .relative()
            .overflow_x_scroll()
            .track_scroll(scroll)
            .horizontal_scrollbar(scroll)
            .bg(sidebar)
            .border_b_1()
            .border_color(border)
            // 内边距属于内容，滚动层与覆盖滑块始终共用无内边距的视口。
            .child(
                h_flex()
                    .flex_none()
                    .items_center()
                    .gap(px(4.0))
                    .p(px(8.0))
                    .child(title)
                    .children(children),
            )
            .into_any_element()
    } else {
        v_flex()
            .id("settings-navigation")
            .debug_selector(|| "settings-navigation".into())
            .w(px(220.0))
            // Desktop navigation owns the root's full cross-axis height; avoid a
            // percentage height that can resolve against page content after a switch.
            .self_stretch()
            .flex_none()
            .relative()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(scroll)
            .vertical_scrollbar(scroll)
            .bg(sidebar)
            .border_r_1()
            .border_color(border)
            .child(
                v_flex()
                    .w_full()
                    .flex_none()
                    .p(px(16.0))
                    .gap(px(4.0))
                    .child(title)
                    .children(children),
            )
            .into_any_element()
    }
}

fn settings_navigation_item(
    page: SettingsPage,
    selected: bool,
    show_update_badge: bool,
    compact: bool,
    style: SettingsNavigationStyle,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
) -> impl IntoElement {
    let debug_selector = format!("settings-page-{}", page.id());
    let background = if selected {
        style.active
    } else {
        hsla(0.0, 0.0, 0.0, 0.0)
    };
    let foreground = if selected {
        style.foreground
    } else {
        style.muted_foreground
    };

    h_flex()
        .id(SharedString::from(format!("settings-page-{}", page.id())))
        .debug_selector(move || debug_selector.clone())
        .w_full()
        .h(px(38.0))
        .flex_none()
        .when(compact, |item| {
            item.w(px(SETTINGS_COMPACT_NAV_ITEM_WIDTH)).flex_none()
        })
        .px(px(10.0))
        .gap(px(9.0))
        .items_center()
        .rounded(px(6.0))
        .bg(background)
        .text_color(foreground)
        .cursor_pointer()
        .hover(move |item| item.bg(if selected { style.active } else { style.hover }))
        .on_click(on_click)
        .child(page.icon().small())
        .child(div().text_sm().child(page.title()))
        .when(show_update_badge, |item| {
            item.child(
                div()
                    .ml_auto()
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(style.accent)
                    .child("新"),
            )
        })
}

pub(super) fn settings_card(title: &'static str, border: gpui_kit::Hsla) -> gpui_kit::Div {
    v_flex()
        .w_full()
        .p(px(16.0))
        .gap(px(12.0))
        .border_1()
        .border_color(border)
        .rounded(px(8.0))
        .child(
            div()
                .text_base()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .child(title),
        )
}

fn managed_in_module_card(title: &'static str, cx: &Context<SettingsView>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    settings_card(title, cx.theme().border)
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child("此模块暂无通用设置。"),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use gpui_kit::{Render, TestAppContext, size};

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
            } else {
                assert!(navigation.size.width <= px(220.0));
                assert_eq!(navigation.bottom(), root.bottom());
                assert!(content.origin.x >= navigation.right());
                assert!(system.right() <= navigation.right());
                assert!(plugins.right() <= navigation.right());
                assert!(update.right() <= navigation.right());
                assert!(update.origin.y > system.origin.y);
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
}
