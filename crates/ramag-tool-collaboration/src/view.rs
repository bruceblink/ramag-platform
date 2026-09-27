//! 本机协作的原生 GPUI 视图；所有导出动作都由用户点击触发。

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _,
    button::ButtonVariants as _,
    h_flex,
    input::{Editor, EditorState, Input, InputState},
    v_flex,
};
use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, IntoElement, ParentElement, Render, Styled,
    Window, div, prelude::*, px,
};
use ramag_app::{CollaborationService, CollaborationServiceError};
use ramag_domain::entities::{CollaborationShare, CollaborationShareId};
use ramag_domain::traits::Storage;
use ramag_ui::clickable_button;

const ACTOR: &str = "local-user";

/// 本机协作视图状态；正文只在 GPUI 编辑器和应用层服务之间短暂流转。
pub struct CollaborationView {
    service: Arc<CollaborationService>,
    title: Entity<InputState>,
    payload: Entity<EditorState>,
    shares: Vec<CollaborationShare>,
    selected: Option<CollaborationShareId>,
    export_text: String,
    status: String,
    busy: bool,
}

impl CollaborationView {
    pub(crate) fn new(
        storage: Arc<dyn Storage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("例如：接口排查说明")
                .default_value("本机协作草稿")
                .validate(|value, _| value.len() <= 256)
        });
        let payload = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .placeholder("只输入你明确选择的文档或查询结果预览")
        });
        Self {
            service: Arc::new(CollaborationService::new(storage)),
            title,
            payload,
            shares: Vec::new(),
            selected: None,
            export_text: String::new(),
            status: "仅保存在本机加密存储中".into(),
            busy: false,
        }
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.list().await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(shares) => {
                        view.selected = view
                            .selected
                            .clone()
                            .filter(|id| shares.iter().any(|share| &share.id == id));
                        view.shares = shares;
                        view.status = "已刷新本机草稿".into();
                    }
                    Err(error) => view.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn create_draft(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let title = self.title.read(cx).value().trim().to_owned();
        let payload = self.payload.read(cx).value().to_string();
        let service = self.service.clone();
        self.busy = true;
        self.status = "正在保存本机加密草稿…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service
                .create_local(
                    title.clone(),
                    vec![ramag_domain::CollaborationArtifact::document(
                        title, payload,
                    )],
                )
                .await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(share) => {
                        view.selected = Some(share.id.clone());
                        view.shares.insert(0, share);
                        view.status = "草稿已保存；不会自动发送到远端".into();
                    }
                    Err(error) => view.status = format_service_error(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn prepare_export(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            self.status = "先选择一个本机草稿".into();
            cx.notify();
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.prepare_manual_export(&id, ACTOR).await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(share) => match share.manual_export_json() {
                        Ok(text) => {
                            view.replace_share(share);
                            view.export_text = text;
                            view.status = "已准备导出文本；仍需用户自行复制或发送".into();
                        }
                        Err(error) => view.status = error.to_string(),
                    },
                    Err(error) => view.status = format_service_error(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn revoke(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            self.status = "先选择一个本机草稿".into();
            cx.notify();
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.revoke(&id, ACTOR).await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(share) => {
                        view.replace_share(share);
                        view.export_text.clear();
                        view.status = "草稿已在本机撤销；历史审计仍保留".into();
                    }
                    Err(error) => view.status = format_service_error(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn replace_share(&mut self, share: CollaborationShare) {
        if let Some(existing) = self.shares.iter_mut().find(|item| item.id == share.id) {
            *existing = share;
        } else {
            self.shares.insert(0, share);
        }
    }

    fn render_share_row(
        &self,
        share: CollaborationShare,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.selected.as_ref() == Some(&share.id);
        let id = share.id.clone();
        let theme = cx.theme().clone();
        div()
            .id(format!("collaboration-share-{index}"))
            .debug_selector(move || format!("collaboration-share-{index}"))
            .w_full()
            .p(px(10.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(if selected { theme.accent } else { theme.border })
            .bg(if selected {
                theme.secondary
            } else {
                theme.background
            })
            .cursor_pointer()
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                view.selected = Some(id.clone());
                view.export_text.clear();
                cx.notify();
            }))
            .child(
                h_flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().text_sm().child(share.title))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{:?} · rev {}", share.state, share.revision)),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!(
                        "{} 个入口 · {}",
                        share.artifacts.len(),
                        share.updated_at
                    )),
            )
    }
}

fn format_service_error(error: CollaborationServiceError) -> String {
    error.to_string()
}

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

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
