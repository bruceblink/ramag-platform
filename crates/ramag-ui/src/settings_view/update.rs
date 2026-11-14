use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder as _, px,
};
use ramag_app::UpdateCheckResult;
use ramag_domain::entities::DownloadProgress;
use ramag_domain::entities::UpdateCancellation;

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
                                "安装更新"
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
            controls = controls.child(render_download_progress(
                progress,
                cancellation.is_cancelled(),
                theme,
            ));
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

mod render;
use self::render::{
    render_download_progress, render_update_identity, render_update_toolbar, update_from_state,
};

#[cfg(test)]
#[path = "update/tests.rs"]
mod tests;
