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
            Self::Database => crate::icons::database(),
            Self::VersionControl => crate::icons::git_branch(),
            Self::Ssh => crate::activity_bar::ActivityBar::icon_for_tool("ssh"),
            Self::ObjectStorage => {
                crate::activity_bar::ActivityBar::icon_for_tool("object_storage")
            }
            Self::Plugins => crate::activity_bar::ActivityBar::icon_for_tool("plugin"),
            Self::Update => Icon::new(IconName::Info),
            Self::Clipboard => crate::icons::clipboard(),
            Self::ToolVisibility => crate::icons::toolbox(),
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
            SettingsPage::Database => self.render_database_page(cx),
            SettingsPage::VersionControl => managed_in_module_card("Git 配置", cx.theme()),
            SettingsPage::Ssh => self.render_ssh_page(cx),
            SettingsPage::ObjectStorage => managed_in_module_card("账号与 Bucket", cx.theme()),
            SettingsPage::Plugins => self.plugin_diagnostics.clone().into_any_element(),
            SettingsPage::Update => self.render_update_page(cx),
            SettingsPage::Clipboard => self.render_clipboard_page(cx),
            SettingsPage::ToolVisibility => self.tool_visibility.clone().into_any_element(),
        };

        v_flex()
            .size_full()
            .id("settings-page-scroll")
            .debug_selector(|| "settings-page-scroll".into())
            .overflow_y_scroll()
            .track_scroll(&self.page_scroll)
            .vertical_scrollbar(&self.page_scroll)
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(if page == SettingsPage::System {
                        1180.0
                    } else {
                        820.0
                    }))
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

/// 在工具设置保存期间提供统一的进行中状态，避免页面只显示一行孤立文本。
pub(super) fn settings_saving_notice(
    id: &'static str,
    label: &'static str,
    theme: &gpui_kit::component::Theme,
) -> impl IntoElement {
    let muted = theme.muted_foreground;
    h_flex()
        .id(id)
        .debug_selector(move || id.into())
        .w_full()
        .min_w_0()
        .items_start()
        .gap(px(8.0))
        .p(px(8.0))
        .border_1()
        .border_color(muted.opacity(0.28))
        .rounded(px(6.0))
        .bg(muted.opacity(0.06))
        .child(Icon::new(IconName::Info).small().text_color(muted))
        .child(
            v_flex()
                .min_w_0()
                .gap(px(2.0))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(muted)
                        .child(format!("{label}：正在保存…")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child("保存完成前暂时不能修改此页设置。"),
                ),
        )
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

/// Creates the compact System Pulse-style settings panel used by migrated pages.
/// The panel keeps the host theme and responsive width while giving the section
/// title the bundled display font used by the reference Settings screen.
pub(super) fn pulse_settings_card(
    title: &'static str,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::Div {
    v_flex()
        .w_full()
        .min_w_0()
        .p(px(16.0))
        .gap(px(12.0))
        .border_1()
        .border_color(theme.border.opacity(0.78))
        .rounded(px(6.0))
        .bg(theme.secondary)
        .child(
            div()
                .font_family("Michroma")
                .text_size(px(19.0))
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .text_color(theme.foreground)
                .child(title),
        )
}

fn managed_in_module_card(title: &'static str, theme: &gpui_kit::component::Theme) -> AnyElement {
    pulse_settings_card(title, theme)
        .id("settings-managed-card")
        .debug_selector(|| "settings-managed-card".into())
        .child(
            h_flex()
                .id("settings-managed-card-heading")
                .debug_selector(|| "settings-managed-card-heading".into())
                .gap(px(8.0))
                .items_center()
                .child(
                    Icon::new(IconName::Info)
                        .small()
                        .text_color(theme.muted_foreground),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child("暂无通用设置"),
                ),
        )
        .child(
            div()
                .id("settings-managed-card-message")
                .debug_selector(|| "settings-managed-card-message".into())
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("此模块暂无通用设置。"),
        )
        .child(
            div()
                .id("settings-managed-card-guidance")
                .debug_selector(|| "settings-managed-card-guidance".into())
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("请在对应工具页面完成具体操作。"),
        )
        .into_any_element()
}

#[cfg(test)]
#[path = "pages_tests.rs"]
mod tests;
