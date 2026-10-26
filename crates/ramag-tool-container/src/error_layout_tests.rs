//! 容器连接错误和 WSL 引导卡的 headless 布局测试。

use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, Bounds, Pixels, TestAppContext, px, size};

use super::ContainerView;

fn assert_inside(parent: Bounds<Pixels>, child: Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label} 越出父容器：parent={parent:?}, child={child:?}"
    );
}

/// Windows named pipe 连接失败时，错误信息和 WSL 建议保持在内容区内并按顺序显示。
#[gpui_kit::test]
fn windows_pipe_error_shows_wsl_guidance_inside_content(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|_| {
            let mut view = ContainerView::without_service();
            // Keep this layout test platform-independent: the production default
            // is a Unix socket on Linux/macOS, so set the Windows endpoint that
            // the WSL guidance is designed to explain explicitly.
            view.docker_endpoint = "npipe:////./pipe/docker_engine".into();
            view.error = Some(
                "无法连接 Windows Docker named pipe；如果 Docker Engine 运行在 WSL 中，请将端点改为 ssh://<WSL用户名>@localhost"
                    .into(),
            );
            view
        });
        Root::new(view, window, cx)
    });

    for width in [360.0, 800.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(640.0)));
        cx.run_until_parked();

        let content = cx
            .debug_bounds("container-content")
            .expect("容器内容区应渲染");
        let error = cx
            .debug_bounds("container-error")
            .expect("容器错误区应渲染");
        let guidance = cx
            .debug_bounds("container-wsl-guidance")
            .expect("WSL 连接建议应渲染");
        let title = cx
            .debug_bounds("container-wsl-guidance-title")
            .expect("WSL 连接建议标题应渲染");
        let endpoint = cx
            .debug_bounds("container-wsl-endpoint-hint")
            .expect("WSL SSH 端点提示应渲染");

        assert_inside(content, error, "容器错误区");
        assert_inside(error, guidance, "WSL 连接建议");
        assert_inside(guidance, title, "WSL 连接建议标题");
        assert_inside(guidance, endpoint, "WSL SSH 端点提示");
        assert!(error.origin.y < guidance.origin.y);
        assert!(title.origin.y < endpoint.origin.y);
    }
}

#[gpui_kit::test]
fn non_named_pipe_error_keeps_original_notice_without_wsl_guidance(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|_| {
            let mut view = ContainerView::without_service();
            view.docker_endpoint = "ssh://user@localhost".into();
            view.error = Some("Docker SSH 连接失败".into());
            view
        });
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(800.0), px(640.0)));
    cx.run_until_parked();

    assert!(
        cx.debug_bounds("container-error").is_some(),
        "非 named pipe 错误应保留原有错误提示"
    );
    assert!(
        cx.debug_bounds("container-wsl-guidance").is_none(),
        "非 named pipe 端点不应显示 WSL 专用建议"
    );
}
