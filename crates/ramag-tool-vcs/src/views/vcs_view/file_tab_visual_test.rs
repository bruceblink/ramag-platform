//! 文件标签来源的布局与像素截图测试。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::super::super::helpers::{FileContentSnapshot, FileTab, FileTabSource};
use super::{VcsView, add_vcs_window, inject_diff_session};
use gpui_kit::{TestAppContext, px, size};
use ramag_domain::entities::FileChangeKind;

fn inject_file_tab_source_session(v: &mut VcsView) {
    inject_diff_session(v);
    v.file_tabs.extend([
        FileTab {
            path: "src/lib.rs".into(),
            source: FileTabSource::ProjectFiles,
            cached_diff: None,
            cached_diff_syntax: None,
            cached_content: Some(FileContentSnapshot {
                path: "src/lib.rs".into(),
                text: std::rc::Rc::new("fn main() {}".into()),
                line_count: 1,
                revision: 1,
                dirty: true,
                truncated: false,
                binary: false,
                error: None,
            }),
        },
        FileTab {
            path: "src/commit.rs".into(),
            source: FileTabSource::Commit {
                commit_id: "def5678".into(),
                change_kind: Some(FileChangeKind::Modified),
            },
            cached_diff: None,
            cached_diff_syntax: None,
            cached_content: None,
        },
        FileTab {
            path: "src/compare.rs".into(),
            source: FileTabSource::Compare {
                from: "main".into(),
                to: "feature/ui".into(),
            },
            cached_diff: None,
            cached_diff_syntax: None,
            cached_content: None,
        },
    ]);
}

/// 来源标签在三种支持窗口中都留在标签栏内，截图测试使用同一组状态数据。
#[gpui_kit::test]
fn vcs_file_tab_sources_fit_supported_window_sizes(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);

    view.update(cx, |view, cx| {
        inject_file_tab_source_session(view);
        cx.notify();
    });

    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        cx.run_until_parked();

        let bar = cx
            .debug_bounds("vcs-ftab-bar")
            .expect("文件标签栏应保持显示");
        assert!(bar.origin.x >= px(0.0));
        assert!(bar.right() <= px(width));
        assert!(bar.bottom() <= px(height));
    }
}

/// 在 macOS Metal 视觉测试环境中捕获真实 VCS 工作区帧，供人工查看来源圆点和密度。
///
/// Linux/Windows 的 `TestAppContext` 只提供布局和交互测试；GPUI 目前没有跨平台
/// headless 图像渲染器，所以该像素测试只在 macOS 主线程视觉测试运行器中启用。
#[cfg(target_os = "macos")]
#[test]
#[ignore = "需要 macOS 主线程 Metal 视觉测试运行器"]
fn captures_vcs_file_tab_screenshot() {
    use gpui_kit::{AppContext as _, VisualTestAppContext, platform};
    use std::fs;
    use std::sync::Arc;

    use super::{MockGit, MockStorage};

    let mut cx = VisualTestAppContext::with_asset_source(
        platform::current_platform(true),
        Arc::new(ramag_ui::RamagAssets),
    );
    cx.update(|app| {
        gpui_kit::component::init(app);
        ramag_ui::theme::init_theme(Some("dark"), app);
    });

    let mut view = None;
    let window = cx
        .open_offscreen_window(size(px(1024.0), px(768.0)), |window, app| {
            let vcs_view =
                app.new(|cx| VcsView::new(Arc::new(MockGit), Arc::new(MockStorage), window, cx));
            view = Some(vcs_view.clone());
            app.new(|cx| gpui_kit::component::Root::new(vcs_view, window, cx))
        })
        .expect("应创建离屏 VCS 窗口");
    let view = view.expect("应保留 VcsView 实体");

    view.update(&mut cx, |view, cx| {
        inject_file_tab_source_session(view);
        cx.notify();
    });
    cx.run_until_parked();

    let image = cx
        .capture_screenshot(window.into())
        .expect("Metal 视觉测试环境应支持截图捕获");
    assert_eq!(image.dimensions(), (1024, 768));
    assert!(image.as_raw().iter().any(|byte| *byte != 0));

    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/ui-screenshots/vcs-file-tabs-1024x768.png");
    fs::create_dir_all(output.parent().expect("截图目录应存在")).expect("应创建截图输出目录");
    image.save(&output).expect("应写入 VCS UI 截图");
}
