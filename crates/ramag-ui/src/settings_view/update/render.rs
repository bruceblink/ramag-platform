use super::*;

use gpui_kit::relative;
use ramag_app::AvailableUpdate;
use ramag_domain::entities::DownloadProgress;

/// 将产品身份和更新状态放在版本信息卡的首行，避免版本号和外链动作成为孤立内容。
pub(super) fn render_update_identity(
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

pub(super) fn update_status_text(state: &UpdateUiState) -> &'static str {
    match state {
        UpdateUiState::Idle => "尚未检查",
        UpdateUiState::UpToDate => "已是最新版本",
        UpdateUiState::Available(_) => "发现新版本",
        UpdateUiState::UnsupportedPlatform(_) => "新版本无安装包",
    }
}

/// 将版本信息和外链操作分成可收缩的布局组，保证更新提示和按钮在窄容器内换行。
pub(super) fn render_update_toolbar(
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

/// 展示下载状态和有界进度，未知总大小时只保留准备状态文本。
pub(super) fn render_download_progress(
    progress: DownloadProgress,
    cancelled: bool,
    theme: &gpui_kit::component::Theme,
) -> impl IntoElement {
    let text = if cancelled {
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

    v_flex()
        .id("settings-update-progress")
        .debug_selector(|| "settings-update-progress".into())
        .w_full()
        .gap(px(6.0))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(text),
                )
                .when(progress.total > 0, |row| {
                    row.child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!(
                                "{}%",
                                (download_progress_ratio(progress) * 100.0).round() as u8
                            )),
                    )
                }),
        )
        .when(progress.total > 0, |row| {
            row.child(
                div()
                    .id("settings-update-progress-track")
                    .debug_selector(|| "settings-update-progress-track".into())
                    .relative()
                    .w_full()
                    .h(px(6.0))
                    .rounded(px(3.0))
                    .bg(theme.border.opacity(0.38))
                    .child(
                        div()
                            .id("settings-update-progress-fill")
                            .debug_selector(|| "settings-update-progress-fill".into())
                            .h_full()
                            .rounded(px(3.0))
                            .bg(theme.accent)
                            .w(relative(download_progress_ratio(progress))),
                    ),
            )
        })
}

/// 把下载进度限制在 `0..=1`，避免驱动的边界值使进度条越过轨道。
pub(super) fn download_progress_ratio(progress: DownloadProgress) -> f32 {
    if progress.total == 0 {
        return 0.0;
    }
    (progress.downloaded as f64 / progress.total as f64).clamp(0.0, 1.0) as f32
}

pub(super) fn update_from_state(state: &UpdateUiState) -> Option<&AvailableUpdate> {
    match state {
        UpdateUiState::Available(update) | UpdateUiState::UnsupportedPlatform(update) => {
            Some(update)
        }
        UpdateUiState::Idle | UpdateUiState::UpToDate => None,
    }
}
