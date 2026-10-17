use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _, button::ButtonVariants as _, h_flex,
    scroll::ScrollableElement as _, v_flex,
};
use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Role, SharedString, Styled, Window, div, img, prelude::*, px,
};
use ramag_domain::entities::{CloudProvider, ObjectStorageAccount};

use super::account_sort::{
    AccountSortColumn, account_sort_description, account_sort_direction, next_account_sort,
    sort_accounts,
};
use super::model::ObjectStorageView;

const CONTENT_MAX_W: f32 = 1080.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AccountRowDensity {
    Full,
    Medium,
    Narrow,
}

fn account_row_density(width: f32) -> AccountRowDensity {
    if width < 600.0 {
        AccountRowDensity::Narrow
    } else if width < 900.0 {
        AccountRowDensity::Medium
    } else {
        AccountRowDensity::Full
    }
}

impl ObjectStorageView {
    pub(super) fn render_accounts(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let query = self
            .account_search
            .read(cx)
            .value()
            .to_string()
            .to_lowercase();
        let mut visible = self
            .accounts
            .iter()
            .filter(|account| {
                query.is_empty()
                    || account.name.to_lowercase().contains(&query)
                    || account
                        .provider
                        .display_name()
                        .to_lowercase()
                        .contains(&query)
            })
            .cloned()
            .collect::<Vec<_>>();
        if let Some(sort) = self.account_sort {
            sort_accounts(&mut visible, sort);
        }
        let density = account_row_density(f32::from(window.viewport_size().width));
        let show_manual_count = density == AccountRowDensity::Full;

        let header_inner = ramag_ui::pulse_ui::pulse_home_toolbar(cx)
            .debug_selector(|| "object-account-toolbar".into())
            .items_center()
            .child(
                div().flex_1().min_w_0().child(
                    div()
                        .debug_selector(|| "object-account-search-input".into())
                        .max_w(px(360.0))
                        .child(
                            ramag_ui::cleanable_input(
                                &self.account_search,
                                "object-account-search-clear",
                                false,
                                cx,
                            )
                            .small()
                            .id("object-account-search-input")
                            .prefix(Icon::new(IconName::Search).small().text_color(muted)),
                        ),
                ),
            )
            .child(
                ramag_ui::clickable_button("object-new-account")
                    .debug_selector(|| "object-new-account".into())
                    .primary()
                    .small()
                    .icon(IconName::Plus)
                    .label("新建账号")
                    .tooltip("新建")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.show_new_account(window, cx);
                    })),
            );
        let header = ramag_ui::pulse_ui::pulse_page_title(
            "对象存储",
            Some(format!(
                "{} 个账号 · Bucket 与对象管理",
                self.accounts.len()
            )),
            cx,
        )
        .max_w(px(CONTENT_MAX_W))
        .id("object-account-page-title")
        .debug_selector(|| "object-account-page-title".into());

        let body = if self.loading && self.accounts.is_empty() {
            centered_message("加载中…", muted).into_any_element()
        } else if self.accounts.is_empty() {
            h_flex()
                .debug_selector(|| "object-account-empty-body".into())
                .size_full()
                .items_center()
                .justify_center()
                .px(px(16.0))
                .child(
                    ramag_ui::pulse_ui::pulse_panel(cx)
                        .id("object-account-empty-state")
                        .debug_selector(|| "object-account-empty-state".into())
                        .max_w(px(480.0))
                        .border_0()
                        .min_h(px(220.0))
                        .items_center()
                        .justify_center()
                        .gap(px(10.0))
                        .p(px(24.0))
                        .text_center()
                        .child(
                            Icon::new(IconName::HardDrive)
                                .large()
                                .text_color(ramag_ui::pulse_ui::pulse_palette(cx).muted),
                        )
                        .child(
                            ramag_ui::pulse_ui::pulse_display_heading("暂无对象存储账号", 16.0, cx)
                                .debug_selector(|| "object-account-empty-title".into()),
                        )
                        .child(
                            div()
                                .debug_selector(|| "object-account-empty-description".into())
                                .text_sm()
                                .text_color(muted)
                                .child("添加 COS、OSS 或 S3 兼容存储账号后，可查看 Bucket 并管理对象。"),
                        )
                        .child(
                            ramag_ui::clickable_button("empty-add-object-account")
                                .debug_selector(|| "object-account-empty-cta".into())
                                .primary()
                                .icon(IconName::Plus)
                                .label("新建账号")
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.show_new_account(window, cx);
                                })),
                        ),
                )
                .into_any_element()
        } else if visible.is_empty() {
            centered_message("暂无匹配", muted).into_any_element()
        } else {
            let rows = v_flex().w_full().children(
                visible
                    .into_iter()
                    .enumerate()
                    .map(|(index, account)| self.render_account_row(index, account, density, cx)),
            );
            let table = v_flex()
                .size_full()
                .min_h_0()
                .child(self.render_account_table_header(show_manual_count, cx))
                .child(
                    div()
                        .id("object-account-list-scroll")
                        .debug_selector(|| "object-account-list-scroll".into())
                        .w_full()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scrollbar()
                        .child(rows),
                );
            table.into_any_element()
        };

        ramag_ui::pulse_ui::pulse_home_frame(cx)
            .child(header)
            .child(
                ramag_ui::pulse_ui::pulse_home_panel(cx)
                    .id("object-account-list-panel")
                    .debug_selector(|| "object-account-list-panel".into())
                    .child(header_inner)
                    .child(
                        div()
                            .debug_selector(|| "object-account-list-body".into())
                            .flex_1()
                            .min_h_0()
                            .overflow_hidden()
                            .child(body),
                    ),
            )
            .into_any_element()
    }

    fn render_account_table_header(
        &self,
        show_manual_count: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !show_manual_count {
            return div()
                .id("object-account-table-header")
                .debug_selector(|| "object-account-table-header".into())
                .h(px(0.0))
                .flex_none()
                .into_any_element();
        }

        let theme = cx.theme();
        h_flex()
            .id("object-account-table-header")
            .debug_selector(|| "object-account-table-header".into())
            .w_full()
            .flex_none()
            .items_center()
            .gap(px(8.0))
            .px(px(14.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(div().w(px(24.0)).flex_none())
            .child(self.render_account_sort_header(
                AccountSortColumn::Name,
                "账号",
                None,
                true,
                false,
                cx,
            ))
            .child(self.render_account_sort_header(
                AccountSortColumn::Provider,
                "服务商",
                Some(120.0),
                false,
                true,
                cx,
            ))
            .child(self.render_account_sort_header(
                AccountSortColumn::ReadOnly,
                "状态",
                Some(56.0),
                false,
                true,
                cx,
            ))
            .child(self.render_account_sort_header(
                AccountSortColumn::BucketCount,
                "Bucket",
                Some(140.0),
                false,
                false,
                cx,
            ))
            .child(
                div()
                    .id("object-account-table-header-actions")
                    .debug_selector(|| "object-account-table-header-actions".into())
                    .w(px(72.0))
                    .flex_none()
                    .text_right()
                    .child("操作"),
            )
            .into_any_element()
    }

    fn render_account_sort_header(
        &self,
        column: AccountSortColumn,
        label: &'static str,
        width: Option<f32>,
        flexible: bool,
        centered: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sort = self.account_sort;
        let description = account_sort_description(sort, column, label);
        let direction = account_sort_direction(sort, column);
        let foreground = cx.theme().foreground;
        let selector = format!("object-account-table-sort-{}", column.key());
        let debug_selector = selector.clone();
        let mut header = h_flex()
            .id(SharedString::from(selector))
            .debug_selector(move || debug_selector.clone())
            .role(Role::Button)
            .aria_label(description)
            .items_center()
            .min_w_0()
            .cursor_pointer()
            .hover(move |header| header.text_color(foreground))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.account_sort = Some(next_account_sort(this.account_sort, column));
                cx.notify();
            }))
            .child(
                ramag_ui::sortable_header_content(
                    div()
                        .when(!centered, |label| label.flex_1())
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(label),
                    direction,
                    cx.theme().muted_foreground,
                    foreground,
                )
                .when(centered, |content| content.justify_center()),
            );
        if let Some(width) = width {
            header = header.w(px(width)).flex_none();
        }
        if flexible {
            header = header.flex_1();
        }
        if centered {
            header = header.justify_center();
        }
        header.into_any_element()
    }

    fn render_account_row(
        &self,
        index: usize,
        account: ObjectStorageAccount,
        density: AccountRowDensity,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = account.id.clone();
        let edit_id = id.clone();
        let delete_id = id.clone();
        let account_key = id.to_string();
        let edit_selector = format!("object-account-row-edit-{account_key}");
        let edit_debug_selector = edit_selector.clone();
        let delete_selector = format!("object-account-row-delete-{account_key}");
        let delete_debug_selector = delete_selector.clone();
        let row_selector = format!("object-account-row-{account_key}");
        let name_selector = format!("object-account-row-name-{account_key}");
        let actions_selector = format!("object-account-row-actions-{account_key}");
        let primary_selector = format!("object-account-row-primary-{account_key}");
        let metadata_selector = format!("object-account-row-metadata-{account_key}");
        let selected = self.selected_account_id.as_ref() == Some(&id);
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let accent = cx.theme().accent;
        let danger = cx.theme().danger;
        let narrow = density == AccountRowDensity::Narrow;
        let mut provider_bg = accent;
        provider_bg.a = 0.12;
        let mut production_bg = danger;
        production_bg.a = 0.15;
        let provider_id = match account.provider {
            CloudProvider::TencentCos => "tencent-cos",
            CloudProvider::AliyunOss => "aliyun-oss",
            CloudProvider::S3Compatible => "s3-compatible",
        };
        let provider_icon = ramag_ui::icons::object_storage_brand_icon(provider_id);

        let row = if narrow { v_flex() } else { h_flex() };
        let mut row = row
            .id(SharedString::from(format!("object-account-{id}")))
            .debug_selector(move || row_selector.clone())
            .w_full()
            .min_w_0()
            .h(px(if narrow { 74.0 } else { 56.0 }))
            .items_center()
            .gap(px(if narrow { 4.0 } else { 8.0 }))
            .px(px(14.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(border)
            .cursor_pointer()
            .when(selected, |row| {
                let mut selected_bg = accent;
                selected_bg.a = 0.06;
                row.bg(selected_bg)
            })
            .hover(|row| row.bg(cx.theme().muted))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.select_account(id.clone(), window, cx);
            }));

        let provider_label = account.provider.display_name();
        let account_name = account.name.clone();
        let name_tooltip = account.name.clone();
        let name = div()
            .id(SharedString::from(name_selector.clone()))
            .debug_selector(move || name_selector.clone())
            .flex_1()
            .min_w_0()
            .text_sm()
            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(name_tooltip.clone()).build(window, cx)
            })
            .child(account_name);
        let icon = div()
            .id(SharedString::from(format!(
                "object-account-provider-icon-{index}"
            )))
            .debug_selector(move || format!("object-account-provider-icon-{index}"))
            .flex_none()
            .w(px(24.0))
            .flex()
            .justify_center()
            .when_some(provider_icon, |slot, icon| {
                slot.child(img(icon).size(px(18.0)))
            });
        let provider = div()
            .debug_selector(move || format!("object-account-provider-{index}"))
            .flex_none()
            .w(px(120.0))
            .flex()
            .justify_center()
            .child(
                div()
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .text_xs()
                    .text_color(accent)
                    .bg(provider_bg)
                    .child(provider_label),
            );
        let read_only = div()
            .debug_selector(move || format!("object-account-read-only-{index}"))
            .flex_none()
            .w(px(56.0))
            .flex()
            .justify_center()
            .when(account.read_only, |slot| {
                slot.child(
                    div()
                        .px(px(6.0))
                        .py(px(1.0))
                        .rounded(px(4.0))
                        .text_xs()
                        .text_color(danger)
                        .bg(production_bg)
                        .child(ramag_ui::PRODUCTION_BADGE_LABEL),
                )
            });
        let actions = h_flex()
            .id(SharedString::from(actions_selector.clone()))
            .debug_selector(move || actions_selector.clone())
            .flex_none()
            .w(px(72.0))
            .justify_end()
            .gap(px(4.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                ramag_ui::clickable_button(SharedString::from(format!(
                    "edit-object-account-{edit_id}"
                )))
                .debug_selector(move || edit_debug_selector.clone())
                .ghost()
                .small()
                .icon(ramag_ui::icons::pencil())
                .tooltip("编辑")
                .on_click(cx.listener(
                    move |this, _: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        this.selected_account_id = Some(edit_id.clone());
                        this.show_edit_account(window, cx);
                    },
                )),
            )
            .child(
                ramag_ui::clickable_button(SharedString::from(format!(
                    "delete-object-account-{delete_id}"
                )))
                .debug_selector(move || delete_debug_selector.clone())
                .ghost()
                .small()
                .icon(ramag_ui::icons::trash())
                .tooltip("删除")
                .on_click(cx.listener(
                    move |this, _: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        this.selected_account_id = Some(delete_id.clone());
                        this.request_delete_account(window, cx);
                    },
                )),
            );

        if narrow {
            row = row
                .child(
                    h_flex()
                        .id(SharedString::from(primary_selector.clone()))
                        .debug_selector(move || primary_selector.clone())
                        .w_full()
                        .min_w_0()
                        .gap(px(8.0))
                        .items_center()
                        .child(icon)
                        .child(name)
                        .child(actions),
                )
                .child(
                    h_flex()
                        .id(SharedString::from(metadata_selector.clone()))
                        .debug_selector(move || metadata_selector.clone())
                        .w_full()
                        .min_w_0()
                        .h(px(24.0))
                        .pl(px(32.0))
                        .gap(px(8.0))
                        .items_center()
                        .child(provider)
                        .child(read_only),
                );
        } else {
            row = row
                .child(icon)
                .child(name)
                .child(provider)
                .child(read_only)
                .when(density == AccountRowDensity::Full, |row| {
                    row.child(
                        div()
                            .debug_selector(move || format!("object-account-bucket-count-{index}"))
                            .flex_none()
                            .w(px(140.0))
                            .text_xs()
                            .text_color(muted)
                            .child(format!("{} 个 Bucket", account.manual_buckets.len())),
                    )
                })
                .child(actions);
        }

        row.into_any_element()
    }
}

fn centered_message(message: &'static str, color: gpui_kit::Hsla) -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .child(div().text_sm().text_color(color).child(message))
}
