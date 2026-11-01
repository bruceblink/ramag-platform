use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Sizable as _, h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder as _, px,
};
use ramag_app::{AvailableUpdate, UpdateCheckResult};
use ramag_domain::entities::{DownloadProgress, UpdateCancellation};

use super::{SettingsView, UpdateUiState, pages::pulse_settings_card};

impl SettingsView {
    pub(super) fn sync_update_state(&mut self) {
        if self.update_checking {
            return;
        }
        let Some(result) = self
            .update_service
            .as_ref()
            .and_then(|service| service.last_result())
        else {
            return;
        };
        self.update_state = match result {
            UpdateCheckResult::UpToDate { .. } => UpdateUiState::UpToDate,
            UpdateCheckResult::Available(update) => UpdateUiState::Available(update),
            UpdateCheckResult::UnsupportedPlatform(update) => {
                UpdateUiState::UnsupportedPlatform(update)
            }
        };
    }

    pub(super) fn render_update_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let accent = theme.accent;
        let link_hover = theme.link_hover;
        let current_version = self
            .update_service
            .as_ref()
            .map_or(env!("CARGO_PKG_VERSION"), |service| {
                service.current_version()
            });
        let update = update_from_state(&self.update_state).cloned();

        v_flex()
            .id("settings-about-layout")
            .debug_selector(|| "settings-about-layout".into())
            .w_full()
            .max_w(px(760.0))
            .mx_auto()
            .gap(px(16.0))
            .child(
                pulse_settings_card("版本信息", theme)
                    .id("settings-about-card")
                    .debug_selector(|| "settings-about-card".into())
                    .child(render_update_identity(&self.update_state, theme))
                    .child(render_update_toolbar(
                        current_version.to_string(),
                        update,
                        accent,
                        theme.foreground,
                        link_hover,
                    ))
                    .child(self.render_update_controls(cx)),
            )
            .into_any_element()
    }

    fn render_update_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let available = matches!(self.update_state, UpdateUiState::Available(_));
        let downloaded = self.update_download_path.is_some();
        let busy = self.update_checking || self.update_download.is_some();
        let progress = *self.update_progress.lock();
        let mut controls = v_flex()
            .id("settings-update-controls")
            .debug_selector(|| "settings-update-controls".into())
            .w_full()
            .min_w_0()
            .gap(px(12.0))
            .pt(px(14.0))
            .border_t_1()
            .border_color(theme.border.opacity(0.72))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(div().text_sm().child("自动检测更新"))
                    .child(
                        div()
                            .id("settings-update-auto-wrapper")
                            .debug_selector(|| "settings-update-auto-wrapper".into())
                            .child(
                                crate::clickable_switch("settings-update-auto")
                                    .checked(self.update_auto_enabled)
                                    .disabled(!self.update_auto_loaded || self.update_auto_saving)
                                    .on_click(cx.listener(|this, _: &bool, _, cx| {
                                        this.toggle_auto_update(cx)
                                    })),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        crate::clickable_button("settings-update-check")
                            .debug_selector(|| "settings-update-check".into())
                            .outline()
                            .small()
                            .label(if self.update_checking {
                                "正在检查"
                            } else {
                                "检查更新"
                            })
                            .disabled(self.update_service.is_none() || busy)
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.check_for_update(cx)
                                }),
                            ),
                    )
                    .when(available && self.update_download.is_none(), |row| {
                        row.child(
                            crate::clickable_button(if downloaded {
                                "settings-update-install"
                            } else {
                                "settings-update-download"
                            })
                            .debug_selector(move || {
                                if downloaded {
                                    "settings-update-install".into()
                                } else {
                                    "settings-update-download".into()
                                }
                            })
                            .outline()
                            .small()
                            .label(if downloaded {
                                "安装安装包"
                            } else {
                                "下载安装包"
                            })
                            .disabled(self.update_checking)
                            .on_click(cx.listener(
                                |this, _: &ClickEvent, _, cx| {
                                    if this.update_download_path.is_some() {
                                        this.install_update(cx);
                                    } else {
                                        this.download_update(cx);
                                    }
                                },
                            )),
                        )
                    })
                    .when(self.update_download.is_some(), |row| {
                        row.child(
                            crate::clickable_button("settings-update-cancel")
                                .debug_selector(|| "settings-update-cancel".into())
                                .outline()
                                .small()
                                .label("取消下载")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if let Some(cancellation) = &this.update_download {
                                        cancellation.cancel();
                                    }
                                    cx.notify();
                                })),
                        )
                    })
                    .when(self.update_download_path.is_some(), |row| {
                        row.child(
                            crate::clickable_button("settings-update-reveal")
                                .debug_selector(|| "settings-update-reveal".into())
                                .outline()
                                .small()
                                .label("打开安装包目录")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if let (Some(service), Some(path)) =
                                        (&this.update_service, &this.update_download_path)
                                        && let Err(error) = service.reveal_download(path)
                                    {
                                        this.update_download_error =
                                            Some(format!("打开目录失败：{error}"));
                                    }
                                    cx.notify();
                                })),
                        )
                    }),
            );
        if let Some(cancellation) = &self.update_download {
            let text = if cancellation.is_cancelled() {
                "正在取消下载".to_string()
            } else if progress.total == 0 {
                "正在准备下载".to_string()
            } else {
                format!(
                    "下载中：{} / {}",
                    ramag_domain::entities::format_bytes(progress.downloaded),
                    ramag_domain::entities::format_bytes(progress.total)
                )
            };
            controls = controls.child(
                div()
                    .id("settings-update-progress")
                    .debug_selector(|| "settings-update-progress".into())
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(text),
            );
        } else if self.update_download_path.is_some() {
            controls = controls.child(
                div()
                    .text_sm()
                    .text_color(theme.success)
                    .child("安装包已下载并通过 SHA-256 校验"),
            );
        }
        if matches!(self.update_state, UpdateUiState::UpToDate) && self.update_error.is_none() {
            controls = controls.child(
                h_flex()
                    .id("settings-update-result")
                    .debug_selector(|| "settings-update-result".into())
                    .items_center()
                    .gap(px(6.0))
                    .text_sm()
                    .text_color(theme.success)
                    .child(Icon::new(IconName::CircleCheck).small())
                    .child("当前已是最新版本"),
            );
        }
        if let Some(error) = self
            .update_error
            .as_ref()
            .or(self.update_download_error.as_ref())
        {
            controls = controls.child(
                div()
                    .id("settings-update-error")
                    .debug_selector(|| "settings-update-error".into())
                    .text_sm()
                    .text_color(theme.danger)
                    .whitespace_normal()
                    .child(error.clone()),
            );
        }
        controls
    }

    fn install_update(&mut self, cx: &mut Context<Self>) {
        let (Some(service), Some(path)) = (
            self.update_service.as_ref(),
            self.update_download_path.as_ref(),
        ) else {
            return;
        };
        self.update_download_error = service
            .install_download(path)
            .err()
            .map(|error| format!("启动安装包失败：{error}"));
        cx.notify();
    }

    fn toggle_auto_update(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.update_service.clone() else {
            return;
        };
        let next = !self.update_auto_enabled;
        self.update_auto_saving = true;
        self.update_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.set_auto_check_enabled(next).await;
            let _ = this.update(cx, |this, cx| {
                this.update_auto_saving = false;
                match result {
                    Ok(()) => this.update_auto_enabled = next,
                    Err(error) => {
                        this.update_error = Some(format!("保存自动检测设置失败：{error}"))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn check_for_update(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.update_service.clone() else {
            return;
        };
        self.update_checking = true;
        self.update_state = UpdateUiState::Idle;
        self.update_error = None;
        self.update_download_error = None;
        self.update_download_path = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.check(true).await;
            let _ = this.update(cx, |this, cx| {
                this.update_checking = false;
                match result {
                    Ok(result) => crate::activity_bar::sync_update_indicator(&result, cx),
                    Err(error) => this.update_error = Some(format!("检查更新失败：{error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn download_update(&mut self, cx: &mut Context<Self>) {
        let (Some(service), Some(update)) = (
            self.update_service.clone(),
            update_from_state(&self.update_state).cloned(),
        ) else {
            return;
        };
        let cancellation = UpdateCancellation::default();
        let progress = Arc::clone(&self.update_progress);
        *progress.lock() = DownloadProgress::default();
        self.update_download = Some(cancellation.clone());
        self.update_download_path = None;
        self.update_download_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                if !this
                    .update(cx, |this, cx| {
                        if this.update_download.is_some() {
                            cx.notify();
                            true
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false)
                {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let result = service
                .download(
                    &update,
                    Arc::new(move |value| *progress.lock() = value),
                    cancellation.clone(),
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                this.update_download = None;
                match result {
                    Ok(path) => this.update_download_path = Some(path),
                    Err(_) if cancellation.is_cancelled() => {
                        this.update_download_error = Some("下载已取消".into())
                    }
                    Err(error) => {
                        this.update_download_error = Some(format!("下载安装包失败：{error}"))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// 将产品身份和更新状态放在版本信息卡的首行，避免版本号和外链动作成为孤立内容。
fn render_update_identity(
    state: &UpdateUiState,
    theme: &gpui_kit::component::Theme,
) -> impl IntoElement {
    let (label, color) = update_status_label(state, theme);
    h_flex()
        .id("settings-update-identity")
        .debug_selector(|| "settings-update-identity".into())
        .w_full()
        .min_w_0()
        .items_center()
        .gap(px(10.0))
        .child(
            h_flex()
                .size(px(34.0))
                .flex_none()
                .items_center()
                .justify_center()
                .child(gpui_kit::img(crate::icons::ramag_brand_icon()).size(px(34.0))),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.0))
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .child("Ramag"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("本地优先的开发者工具平台"),
                ),
        )
        .child(
            h_flex()
                .id("settings-update-status")
                .debug_selector(|| "settings-update-status".into())
                .flex_none()
                .items_center()
                .gap(px(6.0))
                .px(px(8.0))
                .py(px(5.0))
                .rounded(px(999.0))
                .bg(color.opacity(0.12))
                .child(div().size(px(6.0)).rounded(px(3.0)).bg(color))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(color)
                        .child(label),
                ),
        )
}

fn update_status_label<'a>(
    state: &'a UpdateUiState,
    theme: &'a gpui_kit::component::Theme,
) -> (&'static str, gpui_kit::Hsla) {
    let color = match state {
        UpdateUiState::Idle => theme.muted_foreground,
        UpdateUiState::UpToDate => theme.success,
        UpdateUiState::Available(_) => theme.accent,
        UpdateUiState::UnsupportedPlatform(_) => theme.warning,
    };
    (update_status_text(state), color)
}

fn update_status_text(state: &UpdateUiState) -> &'static str {
    match state {
        UpdateUiState::Idle => "尚未检查",
        UpdateUiState::UpToDate => "已是最新版本",
        UpdateUiState::Available(_) => "发现新版本",
        UpdateUiState::UnsupportedPlatform(_) => "新版本无安装包",
    }
}

/// 将版本信息和外链操作分成可收缩的布局组，保证更新提示和按钮在窄容器内换行。
fn render_update_toolbar(
    current_version: String,
    update: Option<AvailableUpdate>,
    accent: gpui_kit::Hsla,
    foreground: gpui_kit::Hsla,
    link_hover: gpui_kit::Hsla,
) -> impl IntoElement {
    let mut info = h_flex()
        .id("settings-update-info")
        .debug_selector(|| "settings-update-info".into())
        .flex_1()
        .min_w_0()
        .flex_wrap()
        .items_end()
        .gap(px(24.0))
        .child(update_version_summary(
            "当前版本",
            current_version,
            foreground,
            Some("settings-update-current-version"),
        ));
    if let Some(update) = update {
        let release_url = update.release.release_url.clone();
        info = info.child(
            update_version_summary(
                "可用版本",
                update.release.version,
                accent,
                Some("settings-update-release-page"),
            )
            .id("settings-update-available")
            .debug_selector(|| "settings-update-available".into())
            .cursor_pointer()
            .hover(move |link| link.text_color(link_hover))
            .on_click(move |_: &ClickEvent, _, cx| {
                cx.open_url(&release_url);
            }),
        );
    }

    h_flex()
        .id("settings-update-toolbar")
        .debug_selector(|| "settings-update-toolbar".into())
        .w_full()
        .min_w_0()
        .flex_wrap()
        .items_end()
        .gap(px(16.0))
        .child(info)
        .child(
            h_flex()
                .id("settings-update-actions")
                .debug_selector(|| "settings-update-actions".into())
                .flex_1()
                .min_w_0()
                .flex_wrap()
                .items_center()
                .justify_end()
                .gap(px(8.0))
                .child(
                    crate::clickable_button("settings-community")
                        .debug_selector(|| "settings-community".into())
                        .outline()
                        .small()
                        .label("交流群")
                        .on_click(|_: &ClickEvent, _, cx| {
                            cx.open_url(crate::COMMUNITY_URL);
                        }),
                )
                .child(
                    crate::clickable_button("settings-feedback-issue")
                        .debug_selector(|| "settings-feedback-issue".into())
                        .outline()
                        .small()
                        .label("反馈问题")
                        .on_click(|_: &ClickEvent, _, cx| {
                            cx.open_url(crate::FEEDBACK_ISSUE_URL);
                        }),
                ),
        )
}

fn update_version_summary(
    label: &'static str,
    value: String,
    value_color: gpui_kit::Hsla,
    debug_selector: Option<&'static str>,
) -> gpui_kit::Div {
    v_flex()
        .min_w_0()
        .gap(px(2.0))
        .child(
            div()
                .text_xs()
                .text_color(value_color.opacity(0.68))
                .child(label),
        )
        .child(
            div()
                .when_some(debug_selector, |value, selector| {
                    value.debug_selector(move || selector.into())
                })
                .text_sm()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(value_color)
                .whitespace_normal()
                .child(value),
        )
}

fn update_from_state(state: &UpdateUiState) -> Option<&AvailableUpdate> {
    match state {
        UpdateUiState::Available(update) | UpdateUiState::UnsupportedPlatform(update) => {
            Some(update)
        }
        UpdateUiState::Idle | UpdateUiState::UpToDate => None,
    }
}

#[cfg(test)]
#[path = "update/tests.rs"]
mod tests;
