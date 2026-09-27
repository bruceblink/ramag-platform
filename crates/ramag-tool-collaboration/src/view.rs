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
use ramag_domain::traits::{ClipboardDriver, CollaborationRelay, Storage};
use ramag_ui::clickable_button;

const ACTOR: &str = "local-user";

/// 本机协作视图状态；正文只在 GPUI 编辑器和应用层服务之间短暂流转。
pub struct CollaborationView {
    service: Arc<CollaborationService>,
    clipboard: Option<Arc<dyn ClipboardDriver>>,
    title: Entity<InputState>,
    relay_endpoint: Entity<InputState>,
    remote_id: Entity<InputState>,
    payload: Entity<EditorState>,
    import_text: Entity<EditorState>,
    shares: Vec<CollaborationShare>,
    selected: Option<CollaborationShareId>,
    export_text: String,
    status: String,
    busy: bool,
    relay_available: bool,
}

impl CollaborationView {
    pub(crate) fn new(
        storage: Arc<dyn Storage>,
        clipboard: Option<Arc<dyn ClipboardDriver>>,
        relay: Option<Arc<dyn CollaborationRelay>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("例如：接口排查说明")
                .default_value("本机协作草稿")
                .validate(|value, _| value.len() <= 256)
        });
        let relay_endpoint = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Relay HTTPS 地址")
                .default_value("https://relay.example")
                .validate(|value, _| value.len() <= 4096)
        });
        let remote_id = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("远端共享包 ID")
                .validate(|value, _| value.len() <= 256)
        });
        let payload = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .placeholder("只输入你明确选择的文档或查询结果预览")
        });
        let import_text = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .placeholder("粘贴用户明确提供的共享包 JSON")
        });
        Self {
            relay_available: relay.is_some(),
            service: Arc::new(match relay {
                Some(relay) => CollaborationService::with_relay(storage, relay),
                None => CollaborationService::new(storage),
            }),
            clipboard: clipboard.clone(),
            title,
            relay_endpoint,
            remote_id,
            payload,
            import_text,
            shares: Vec::new(),
            selected: None,
            export_text: String::new(),
            status: if clipboard.is_some() {
                "仅保存在本机加密存储中".into()
            } else {
                "当前平台没有剪贴板能力；可手动复制导出文本".into()
            },
            busy: false,
        }
    }

    pub(crate) fn reload(&mut self, cx: &mut Context<Self>) {
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

    fn import_manual(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let encoded = self.import_text.read(cx).value().to_string();
        if encoded.trim().is_empty() {
            self.status = "先粘贴共享包 JSON".into();
            cx.notify();
            return;
        }
        self.busy = true;
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.import_manual_export(&encoded, ACTOR).await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(share) => {
                        view.selected = Some(share.id.clone());
                        view.shares.insert(0, share);
                        view.export_text.clear();
                        view.status = "已导入为新的本机草稿；未连接远端".into();
                    }
                    Err(error) => view.status = format_service_error(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn copy_export(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            self.status = "先选择一个本机草稿".into();
            cx.notify();
            return;
        };
        let Some(clipboard) = self.clipboard.clone() else {
            self.status = "当前平台没有剪贴板能力；请手动复制导出文本".into();
            cx.notify();
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service
                .manual_export_json(&id)
                .await
                .map_err(|error| error.to_string())
                .and_then(|text| {
                    clipboard
                        .write_text(&text, None)
                        .map(|()| text)
                        .map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(text) => {
                        view.export_text = text;
                        view.status = "已复制安全导出包；不会自动发送到远端".into();
                    }
                    Err(error) => view.status = format!("复制导出包失败：{error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn publish_remote(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            self.status = "先选择一个本机草稿".into();
            cx.notify();
            return;
        };
        let endpoint = self.relay_endpoint.read(cx).value().trim().to_owned();
        if endpoint.is_empty() {
            self.status = "先填写 Relay HTTPS 地址".into();
            cx.notify();
            return;
        }
        if self.busy {
            return;
        }
        self.busy = true;
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.publish_remote(&id, &endpoint).await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(receipt) => {
                        view.status = format!(
                            "已发送到用户指定 Relay；远端 ID {}，未自动继续同步",
                            receipt.remote_id
                        );
                    }
                    Err(error) => view.status = format_service_error(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn import_remote(&mut self, cx: &mut Context<Self>) {
        let endpoint = self.relay_endpoint.read(cx).value().trim().to_owned();
        let remote_id = self.remote_id.read(cx).value().trim().to_owned();
        if endpoint.is_empty() || remote_id.is_empty() {
            self.status = "填写 Relay 地址和远端共享包 ID 后再读取".into();
            cx.notify();
            return;
        }
        if self.busy {
            return;
        }
        self.busy = true;
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.import_remote(&endpoint, &remote_id, ACTOR).await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(share) => {
                        view.selected = Some(share.id.clone());
                        view.shares.insert(0, share);
                        view.status = "远端包已重新校验并导入为本机草稿".into();
                    }
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

#[path = "view_render.rs"]
mod render;

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
