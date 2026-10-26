use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use ramag_app::{AvailableUpdate, UpdateCheckResult};

use super::{SettingsView, UpdateUiState, pages::pulse_settings_card};

impl SettingsView {
    pub(super) fn sync_update_state(&mut self) {
        if !matches!(self.update_state, UpdateUiState::Idle) {
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

        pulse_settings_card("版本信息", theme)
            .id("settings-about-card")
            .debug_selector(|| "settings-about-card".into())
            .child(render_update_identity(&self.update_state, theme))
            .child(render_update_toolbar(
                format!("当前版本：{current_version}"),
                update,
                accent,
                link_hover,
            ))
            .into_any_element()
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
            div()
                .size(px(34.0))
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(8.0))
                .bg(theme.accent.opacity(0.14))
                .child(Icon::new(IconName::Info).small().text_color(theme.accent)),
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
                        .child("原生开发工作台 · 版本与更新"),
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
    link_hover: gpui_kit::Hsla,
) -> impl IntoElement {
    let mut info = h_flex()
        .id("settings-update-info")
        .debug_selector(|| "settings-update-info".into())
        .flex_1()
        .min_w_0()
        .flex_wrap()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .debug_selector(|| "settings-update-current-version".into())
                .flex_1()
                .min_w_0()
                .text_sm()
                .whitespace_normal()
                .child(current_version),
        );
    if let Some(update) = update {
        let release_url = update.release.release_url.clone();
        info = info.child(
            h_flex()
                .debug_selector(|| "settings-update-available".into())
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(accent)
                        .child("新"),
                )
                .child(
                    div()
                        .id("settings-update-release-page")
                        .debug_selector(|| "settings-update-release-page".into())
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(accent)
                        .whitespace_normal()
                        .cursor_pointer()
                        .hover(move |link| link.text_color(link_hover))
                        .child(format!("新版本：{}", update.release.version))
                        .on_click(move |_: &ClickEvent, _, cx| {
                            cx.open_url(&release_url);
                        }),
                ),
        );
    }

    h_flex()
        .id("settings-update-toolbar")
        .debug_selector(|| "settings-update-toolbar".into())
        .w_full()
        .min_w_0()
        .flex_wrap()
        .items_center()
        .gap(px(12.0))
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

fn update_from_state(state: &UpdateUiState) -> Option<&AvailableUpdate> {
    match state {
        UpdateUiState::Available(update) | UpdateUiState::UnsupportedPlatform(update) => {
            Some(update)
        }
        UpdateUiState::Idle | UpdateUiState::UpToDate => None,
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::ActiveTheme as _;
    use gpui_kit::{
        Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _,
        TestAppContext, VisualTestContext, Window, div, px, size,
    };
    use ramag_app::AvailableUpdate;
    use ramag_domain::entities::ReleaseInfo;

    use super::{UpdateUiState, render_update_identity, render_update_toolbar, update_status_text};

    #[test]
    fn update_status_text_covers_all_update_states() {
        let update = AvailableUpdate {
            release: ReleaseInfo {
                version: "0.8.0".into(),
                tag_name: "v0.8.0".into(),
                release_url: "https://example.com/releases/v0.8.0".into(),
                notes: String::new(),
                published_at: None,
                assets: Vec::new(),
            },
            asset: None,
        };
        assert_eq!(update_status_text(&UpdateUiState::Idle), "尚未检查");
        assert_eq!(update_status_text(&UpdateUiState::UpToDate), "已是最新版本");
        assert_eq!(
            update_status_text(&UpdateUiState::Available(update.clone())),
            "发现新版本"
        );
        assert_eq!(
            update_status_text(&UpdateUiState::UnsupportedPlatform(update)),
            "新版本无安装包"
        );
    }

    struct UpdateIdentityHost {
        state: UpdateUiState,
    }

    impl Render for UpdateIdentityHost {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("settings-update-identity-host")
                .debug_selector(|| "settings-update-identity-host".into())
                .w_full()
                .child(render_update_identity(&self.state, cx.theme()))
        }
    }

    #[gpui_kit::test]
    fn update_identity_stays_inside_supported_widths(cx: &mut TestAppContext) {
        cx.update(gpui_kit::component::init);
        let (_, cx) = cx.add_window_view(|_, _| UpdateIdentityHost {
            state: UpdateUiState::UpToDate,
        });
        let cx: &mut VisualTestContext = cx;
        for width in [360.0, 1024.0, 1440.0] {
            cx.simulate_resize(size(px(width), px(220.0)));
            cx.run_until_parked();

            let Some(host) = cx.debug_bounds("settings-update-identity-host") else {
                unreachable!("关于页身份区测试宿主应渲染");
            };
            let Some(identity) = cx.debug_bounds("settings-update-identity") else {
                unreachable!("关于页身份区应渲染");
            };
            let Some(status) = cx.debug_bounds("settings-update-status") else {
                unreachable!("关于页更新状态徽标应渲染");
            };

            for child in [identity, status] {
                assert!(child.origin.x >= host.origin.x);
                assert!(child.origin.y >= host.origin.y);
                assert!(child.origin.x + child.size.width <= host.origin.x + host.size.width);
                assert!(child.origin.y + child.size.height <= host.origin.y + host.size.height);
            }
        }
    }

    struct UpdateToolbarHost;

    impl Render for UpdateToolbarHost {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = cx.theme();
            div()
                .id("settings-update-toolbar-host")
                .debug_selector(|| "settings-update-toolbar-host".into())
                .w(px(96.0))
                .h(px(600.0))
                .child(render_update_toolbar(
                    "当前版本：0.1.0-这是一个足够长的版本标识".into(),
                    Some(AvailableUpdate {
                        release: ReleaseInfo {
                            version: "0.2.0-这是一个足够长的更新版本标识".into(),
                            tag_name: "v0.2.0".into(),
                            release_url: "https://example.com/releases/v0.2.0".into(),
                            notes: String::new(),
                            published_at: None,
                            assets: Vec::new(),
                        },
                        asset: None,
                    }),
                    theme.accent,
                    theme.link_hover,
                ))
        }
    }

    #[gpui_kit::test]
    fn update_toolbar_keeps_long_info_and_actions_inside_narrow_parent(cx: &mut TestAppContext) {
        cx.update(gpui_kit::component::init);
        let (_, cx) = cx.add_window_view(|_, _| UpdateToolbarHost);
        let cx: &mut VisualTestContext = cx;
        cx.simulate_resize(size(px(240.0), px(180.0)));
        cx.run_until_parked();

        let Some(host) = cx.debug_bounds("settings-update-toolbar-host") else {
            unreachable!("更新工具栏宿主应渲染");
        };
        let Some(toolbar) = cx.debug_bounds("settings-update-toolbar") else {
            unreachable!("更新工具栏应渲染");
        };
        let Some(info) = cx.debug_bounds("settings-update-info") else {
            unreachable!("版本信息组应渲染");
        };
        let Some(actions) = cx.debug_bounds("settings-update-actions") else {
            unreachable!("操作组应渲染");
        };
        let Some(version) = cx.debug_bounds("settings-update-current-version") else {
            unreachable!("当前版本应渲染");
        };
        let Some(release) = cx.debug_bounds("settings-update-release-page") else {
            unreachable!("更新版本链接应渲染");
        };
        let Some(community) = cx.debug_bounds("settings-community") else {
            unreachable!("交流群按钮应渲染");
        };
        let Some(feedback) = cx.debug_bounds("settings-feedback-issue") else {
            unreachable!("反馈问题按钮应渲染");
        };

        for child in [
            toolbar, info, actions, version, release, community, feedback,
        ] {
            assert!(child.origin.x >= host.origin.x);
            assert!(child.origin.y >= host.origin.y);
            assert!(child.origin.x + child.size.width <= host.origin.x + host.size.width);
            assert!(child.origin.y + child.size.height <= host.origin.y + host.size.height);
            assert!(child.origin.x + child.size.width <= toolbar.origin.x + toolbar.size.width);
            assert!(child.origin.y + child.size.height <= toolbar.origin.y + toolbar.size.height);
        }
        assert!(feedback.origin.y > community.origin.y);
    }
}
