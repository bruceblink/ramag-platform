use super::*;

impl Render for CollaborationView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let selected = self
            .selected
            .as_ref()
            .and_then(|id| self.shares.iter().find(|share| &share.id == id));
        let mut rows = v_flex().gap(px(8.0));
        for (index, share) in self.shares.iter().cloned().enumerate() {
            rows = rows.child(self.render_share_row(share, index, cx));
        }
        v_flex()
            .id("collaboration-view")
            .debug_selector(|| "collaboration-view".into())
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(24.0))
            .gap(px(14.0))
            .child(
                v_flex()
                    .gap(px(5.0))
                    .child(div().text_lg().child("本机协作"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("原生 GPUI 入口 · 加密草稿 · 用户确认后才准备导出"),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(8.0))
                    .p(px(12.0))
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(8.0))
                    .child(div().text_sm().child("用户确认的 Relay 交接"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("点击发送会把当前已校验的非敏感导出包发送到下方地址；不会后台同步。"),
                    )
                    .child(Input::new(&self.relay_endpoint).w_full())
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap(px(8.0))
                            .child(
                                clickable_button("collaboration-publish")
                                    .debug_selector(|| "collaboration-publish".into())
                                    .primary()
                                    .small()
                                    .label("发送到 Relay")
                                    .disabled(self.busy || !self.relay_available)
                                    .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                                        view.publish_remote(cx);
                                    })),
                            )
                            .child(Input::new(&self.remote_id).flex_1().min_w_0())
                            .child(
                                clickable_button("collaboration-fetch")
                                    .debug_selector(|| "collaboration-fetch".into())
                                    .ghost()
                                    .small()
                                    .label("读取并导入")
                                    .disabled(self.busy || !self.relay_available)
                                    .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                                        view.import_remote(cx);
                                    })),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(8.0))
                    .p(px(12.0))
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(8.0))
                    .child(div().text_sm().child("导入已确认的共享包"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("导入会生成新的本机草稿，不会覆盖现有记录。"),
                    )
                    .child(Editor::new(&self.import_text).h(px(120.0)))
                    .child(
                        h_flex().justify_end().child(
                            clickable_button("collaboration-import")
                                .debug_selector(|| "collaboration-import".into())
                                .ghost()
                                .small()
                                .label("导入为本机草稿")
                                .disabled(self.busy)
                                .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                                    view.import_manual(cx);
                                })),
                        ),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(8.0))
                    .p(px(12.0))
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(8.0))
                    .child(div().text_sm().child("新建本机草稿"))
                    .child(Input::new(&self.title).w_full())
                    .child(Editor::new(&self.payload).h(px(130.0)))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap(px(8.0))
                            .child(
                                clickable_button("collaboration-refresh")
                                    .debug_selector(|| "collaboration-refresh".into())
                                    .ghost()
                                    .small()
                                    .label("刷新")
                                    .disabled(self.busy)
                                    .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                                        view.reload(cx);
                                    })),
                            )
                            .child(
                                clickable_button("collaboration-create")
                                    .debug_selector(|| "collaboration-create".into())
                                    .primary()
                                    .small()
                                    .label(if self.busy { "处理中…" } else { "保存本机草稿" })
                                    .disabled(self.busy)
                                    .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                                        view.create_draft(cx);
                                    })),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(div().text_sm().child("本机草稿"))
                    .child(rows),
            )
            .when_some(selected, |view, share| {
                view.child(
                    v_flex()
                        .gap(px(8.0))
                        .p(px(12.0))
                        .border_1()
                        .border_color(theme.border)
                        .rounded(px(8.0))
                        .child(div().text_sm().child("用户确认操作"))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("敏感内容不能进入导出包；撤销只改变本机状态。"),
                        )
                        .child(
                            h_flex()
                                .flex_wrap()
                                .gap(px(8.0))
                                .child(
                                    clickable_button("collaboration-export")
                                        .debug_selector(|| "collaboration-export".into())
                                        .primary()
                                        .small()
                                        .label("准备导出文本")
                                        .disabled(self.busy || share.state == ramag_domain::CollaborationShareState::Revoked)
                                        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                                            view.prepare_export(cx);
                                        })),
                                )
                                .child(
                                    clickable_button("collaboration-copy")
                                        .debug_selector(|| "collaboration-copy".into())
                                        .ghost()
                                        .small()
                                        .label("复制导出包")
                                        .disabled(
                                            self.busy
                                                || self.clipboard.is_none()
                                                || share.state
                                                    == ramag_domain::CollaborationShareState::Revoked,
                                        )
                                        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                                            view.copy_export(cx);
                                        })),
                                )
                                .child(
                                    clickable_button("collaboration-revoke")
                                        .debug_selector(|| "collaboration-revoke".into())
                                        .danger()
                                        .small()
                                        .label("撤销本机草稿")
                                        .disabled(self.busy || share.state == ramag_domain::CollaborationShareState::Revoked)
                                        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                                            view.revoke(cx);
                                        })),
                                ),
                        )
                        .when(!self.export_text.is_empty(), |panel| {
                            panel.child(
                                div()
                                    .id("collaboration-export-text")
                                    .debug_selector(|| "collaboration-export-text".into())
                                    .max_h(px(220.0))
                                    .overflow_y_scroll()
                                    .p(px(10.0))
                                    .bg(theme.secondary)
                                    .border_1()
                                    .border_color(theme.border)
                                    .rounded(px(6.0))
                                    .text_xs()
                                    .child(self.export_text.clone()),
                            )
                        }),
                )
            })
            .child(
                div()
                    .id("collaboration-status")
                    .debug_selector(|| "collaboration-status".into())
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(self.status.clone()),
            )
    }
}
