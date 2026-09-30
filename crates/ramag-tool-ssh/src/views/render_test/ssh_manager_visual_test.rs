//! SSH 管理器真实像素截图测试。
#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::super::SshView;
use super::support::service;
use gpui_kit::{AppContext as _, VisualTestAppContext, platform, px, size};
use ramag_domain::entities::SshProfile;
use std::fs;
use std::sync::Arc;

/// 在 macOS Metal 视觉测试环境中捕获 SSH 管理器，供人工查看环境标签和行密度。
///
/// Linux/Windows 的 `TestAppContext` 只提供布局和交互测试；GPUI 目前没有跨平台
/// headless 图像渲染器，所以该像素测试只在 macOS 主线程视觉测试运行器中启用。
#[test]
#[ignore = "需要 macOS 主线程 Metal 视觉测试运行器"]
fn captures_ssh_manager_environment_badges_screenshot() {
    let mut dev = SshProfile::new("开发环境", "dev.example.com");
    dev.environment = Some("dev".into());
    dev.username = "alice".into();

    let mut test = SshProfile::new("测试环境", "test.example.com");
    test.environment = Some("test".into());
    test.username = "bob".into();

    let mut prod = SshProfile::new("生产环境", "prod.example.com");
    prod.environment = Some("prod".into());
    prod.username = "carol".into();

    let service = service(vec![dev, test, prod], None);
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
            let ssh_view = app.new(|cx| SshView::new(service, window, cx));
            view = Some(ssh_view.clone());
            app.new(|cx| gpui_kit::component::Root::new(ssh_view, window, cx))
        })
        .expect("应创建离屏 SSH 管理器窗口");
    let _view = view.expect("应保留 SshView 实体");

    cx.run_until_parked();
    let image = cx
        .capture_screenshot(window.into())
        .expect("Metal 视觉测试环境应支持截图捕获");
    assert_eq!(image.dimensions(), (1024, 768));
    assert!(image.as_raw().iter().any(|byte| *byte != 0));

    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/ui-screenshots/ssh-manager-environment-badges-1024x768.png");
    fs::create_dir_all(output.parent().expect("截图目录应存在")).expect("应创建截图输出目录");
    image.save(&output).expect("应写入 SSH 管理器 UI 截图");
}
