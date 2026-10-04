use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _, button::ButtonVariants as _, h_flex,
    scroll::ScrollableElement as _, v_flex,
};
use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, Styled, Window, div, img, prelude::*, px,
};
use ramag_domain::entities::{CloudProvider, ObjectStorageAccount};

use super::model::ObjectStorageView;

const CONTENT_MAX_W: f32 = 1080.0;

impl ObjectStorageView {
    pub(super) fn render_accounts(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let query = self
            .account_search
            .read(cx)
            .value()
            .to_string()
            .to_lowercase();
        let visible = self
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
        let show_manual_count = f32::from(window.viewport_size().width) >= 900.0;

        let header_inner = ramag_ui::responsive_toolbar()
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
                    .outline()
                    .small()
                    .icon(IconName::Plus)
                    .tooltip("新建")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.show_new_account(window, cx);
                    })),
            );
        let header = v_flex()
            .debug_selector(|| "object-account-header".into())
            .w_full()
            .border_b_1()
            .border_color(border)
            .child(
                h_flex().w_full().justify_center().px(px(16.0)).child(
                    v_flex()
                        .w_full()
                        .max_w(px(CONTENT_MAX_W))
                        .gap(px(10.0))
                        .pt(px(16.0))
                        .pb(px(12.0))
                        .child(
                            ramag_ui::pulse_ui::pulse_page_title(
                                "对象存储",
                                Some("账号与 Bucket 管理"),
                                cx,
                            )
                            .id("object-account-page-title")
                            .debug_selector(|| "object-account-page-title".into()),
                        )
                        .child(header_inner),
                ),
            );

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
                                .child("添加 COS 或 OSS 账号后即可查看 Bucket 并管理对象。"),
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
            let rows = v_flex()
                .w_full()
                .children(visible.into_iter().enumerate().map(|(index, account)| {
                    self.render_account_row(index, account, show_manual_count, cx)
                }));
            let table = v_flex()
                .w_full()
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
            v_flex()
                .size_full()
                .child(
                    h_flex()
                        .size_full()
                        .items_stretch()
                        .justify_center()
                        .px(px(16.0))
                        .py(px(10.0))
                        .child(
                            ramag_ui::pulse_ui::pulse_panel(cx)
                                .id("object-account-list-panel")
                                .debug_selector(|| "object-account-list-panel".into())
                                .max_w(px(CONTENT_MAX_W))
                                .h_full()
                                .min_h_0()
                                .p_0()
                                .overflow_hidden()
                                .child(table),
                        ),
                )
                .into_any_element()
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(header)
            .child(div().flex_1().min_h_0().child(body))
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
            .child(
                div()
                    .id("object-account-table-header-name")
                    .debug_selector(|| "object-account-table-header-name".into())
                    .flex_1()
                    .min_w_0()
                    .child("账号"),
            )
            .child(
                div()
                    .id("object-account-table-header-provider")
                    .debug_selector(|| "object-account-table-header-provider".into())
                    .w(px(120.0))
                    .flex_none()
                    .text_center()
                    .child("服务商"),
            )
            .child(
                div()
                    .id("object-account-table-header-status")
                    .debug_selector(|| "object-account-table-header-status".into())
                    .w(px(56.0))
                    .flex_none()
                    .text_center()
                    .child("状态"),
            )
            .child(
                div()
                    .id("object-account-table-header-buckets")
                    .debug_selector(|| "object-account-table-header-buckets".into())
                    .w(px(140.0))
                    .flex_none()
                    .child("Bucket"),
            )
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

    fn render_account_row(
        &self,
        index: usize,
        account: ObjectStorageAccount,
        show_manual_count: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = account.id.clone();
        let edit_id = id.clone();
        let delete_id = id.clone();
        let selected = self.selected_account_id.as_ref() == Some(&id);
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let accent = cx.theme().accent;
        let danger = cx.theme().danger;
        let mut provider_bg = accent;
        provider_bg.a = 0.12;
        let mut production_bg = danger;
        production_bg.a = 0.15;
        let provider_id = match account.provider {
            CloudProvider::TencentCos => "tencent-cos",
            CloudProvider::AliyunOss => "aliyun-oss",
        };
        let provider_icon = ramag_ui::icons::object_storage_brand_icon(provider_id);

        h_flex()
            .id(SharedString::from(format!("object-account-{index}-{id}")))
            .debug_selector(move || format!("object-account-row-{index}"))
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_center()
            .gap(px(8.0))
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
            }))
            .child(
                div()
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
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(account.name),
            )
            .child(
                div()
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
                            .child(account.provider.display_name()),
                    ),
            )
            .child(
                div()
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
                    }),
            )
            .when(show_manual_count, |row| {
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
            .child(
                h_flex()
                    .debug_selector(move || format!("object-account-actions-{index}"))
                    .flex_none()
                    .w(px(72.0))
                    .justify_end()
                    .gap(px(4.0))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        ramag_ui::clickable_button(SharedString::from(format!(
                            "edit-object-account-{edit_id}"
                        )))
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
                    ),
            )
            .into_any_element()
    }
}

fn centered_message(message: &'static str, color: gpui_kit::Hsla) -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .child(div().text_sm().text_color(color).child(message))
}
