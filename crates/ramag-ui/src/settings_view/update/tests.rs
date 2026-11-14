use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, div, px, size,
};
use ramag_app::AvailableUpdate;
use ramag_domain::entities::{DownloadProgress, ReleaseInfo};

use super::UpdateUiState;
use super::render::{
    download_progress_ratio, render_download_progress, render_update_identity,
    render_update_toolbar, update_status_text,
};

#[test]
fn download_progress_ratio_handles_unknown_and_out_of_range_sizes() {
    assert_eq!(download_progress_ratio(DownloadProgress::default()), 0.0);
    assert_eq!(
        download_progress_ratio(DownloadProgress {
            downloaded: 25,
            total: 100,
        }),
        0.25
    );
    assert_eq!(
        download_progress_ratio(DownloadProgress {
            downloaded: 200,
            total: 100,
        }),
        1.0
    );
}

struct DownloadProgressHost {
    progress: DownloadProgress,
}

impl Render for DownloadProgressHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-update-progress-host")
            .debug_selector(|| "settings-update-progress-host".into())
            .w_full()
            .child(render_download_progress(self.progress, false, cx.theme()))
    }
}

#[gpui_kit::test]
fn download_progress_bar_stays_inside_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, cx) = cx.add_window_view(|_, _| DownloadProgressHost {
        progress: DownloadProgress {
            downloaded: 25,
            total: 100,
        },
    });
    let cx: &mut VisualTestContext = cx;
    for width in [360.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(220.0)));
        cx.run_until_parked();

        let host = cx.debug_bounds("settings-update-progress-host");
        assert!(host.is_some(), "下载进度测试宿主应渲染");
        let host = host.unwrap_or_default();
        let track = cx.debug_bounds("settings-update-progress-track");
        assert!(track.is_some(), "已知总大小时应渲染进度轨道");
        let track = track.unwrap_or_default();
        let fill = cx.debug_bounds("settings-update-progress-fill");
        assert!(fill.is_some(), "已知总大小时应渲染进度填充");
        let fill = fill.unwrap_or_default();
        for child in [track, fill] {
            assert!(child.origin.x >= host.origin.x);
            assert!(child.origin.y >= host.origin.y);
            assert!(child.right() <= host.right());
            assert!(child.bottom() <= host.bottom());
        }
        assert!(fill.size.width <= track.size.width);
    }
}

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
                theme.foreground,
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
