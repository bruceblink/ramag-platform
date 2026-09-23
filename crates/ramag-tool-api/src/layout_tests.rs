use super::*;

use gpui_kit::{ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase, point, px, size};

#[gpui_kit::test]
fn api_workbench_reflows_request_editor_and_response_at_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_kit::component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 视图应初始化");

    for (width, height) in [
        (360.0, 240.0),
        (1024.0, 240.0),
        (360.0, 640.0),
        (640.0, 800.0),
        (1024.0, 768.0),
        (1440.0, 900.0),
    ] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();
        let root = visual_cx
            .debug_bounds("api-root")
            .expect("API 根节点应渲染");
        let editor = visual_cx
            .debug_bounds("api-editor")
            .expect("API 编辑器应渲染");
        let request_editor = visual_cx
            .debug_bounds("api-request-editor")
            .expect("请求编辑器应渲染");
        let header = visual_cx
            .debug_bounds("api-header")
            .expect("API Header 应渲染");
        let content_root = visual_cx
            .debug_bounds("api-content")
            .expect("API 主体容器应渲染");
        let sidebar = visual_cx
            .debug_bounds("api-sidebar")
            .expect("API 侧栏应渲染");
        let request_search = visual_cx
            .debug_bounds("api-request-search")
            .expect("请求搜索框应渲染");
        let toolbar = visual_cx
            .debug_bounds("api-request-toolbar")
            .expect("请求工具栏应渲染");
        let command_row = visual_cx
            .debug_bounds("api-request-command-row")
            .expect("请求命令行应渲染");
        let actions = visual_cx
            .debug_bounds("api-request-actions")
            .expect("请求操作区应渲染");
        let workbench = visual_cx
            .debug_bounds("api-request-response")
            .expect("请求响应工作区应渲染");
        let request_pane = visual_cx
            .debug_bounds("api-request-pane")
            .expect("请求面板应渲染");
        let proxy_editor = visual_cx
            .debug_bounds("api-proxy-editor")
            .expect("代理编辑器应渲染");
        let response = visual_cx
            .debug_bounds("api-response")
            .expect("响应面板应渲染");
        assert!(editor.right() <= root.right(), "编辑器不能越出根节点");
        assert!(sidebar.right() <= content_root.right(), "侧栏不能越出主体");
        assert!(
            sidebar.bottom() <= content_root.bottom(),
            "侧栏不能越出主体底部: sidebar={sidebar:?}, content={content_root:?}"
        );
        assert!(
            request_search.origin.x >= sidebar.origin.x
                && request_search.right() <= sidebar.right(),
            "请求搜索框不能越出侧栏: search={request_search:?}, sidebar={sidebar:?}"
        );
        assert!(
            editor.origin.y >= content_root.origin.y,
            "编辑器不能垂直越过主体顶部"
        );
        assert!(
            editor.bottom() <= content_root.bottom(),
            "编辑器不能越出主体底部"
        );
        assert!(
            request_editor.right() <= editor.right(),
            "请求编辑器不能越出 API 编辑器"
        );
        assert!(response.right() <= root.right(), "响应面板不能越出根节点");
        assert!(editor.origin.x >= root.origin.x, "编辑器左边界不能越界");
        assert!(
            header.bottom() <= content_root.origin.y,
            "Header 不能与主体重叠: root={root:?}, header={header:?}, content={content_root:?}"
        );
        assert!(
            toolbar.bottom() <= workbench.origin.y,
            "请求工具栏不能与请求响应工作区重叠: toolbar={toolbar:?}, workbench={workbench:?}"
        );
        assert!(
            command_row.bottom() <= workbench.origin.y,
            "请求命令行不能与请求响应工作区重叠: command={command_row:?}, workbench={workbench:?}"
        );
        assert!(
            request_pane.right() <= workbench.right(),
            "请求面板不能越出工作区: pane={request_pane:?}, workbench={workbench:?}"
        );
        assert!(
            proxy_editor.origin.x >= request_pane.origin.x
                && proxy_editor.right() <= request_pane.right(),
            "代理编辑器不能横向越出请求面板: proxy={proxy_editor:?}, pane={request_pane:?}"
        );
        assert!(
            workbench.bottom() <= content_root.bottom(),
            "请求响应工作区不能越出主体底部: sidebar={sidebar:?}, editor={editor:?}, toolbar={toolbar:?}, command={command_row:?}, workbench={workbench:?}, response={response:?}, content={content_root:?}"
        );
        assert!(
            response.bottom() <= workbench.bottom(),
            "响应面板不能越出请求响应工作区: response={response:?}, workbench={workbench:?}"
        );
        assert!(
            request_pane.origin.y >= command_row.origin.y,
            "请求面板必须位于命令行之后: pane={request_pane:?}, command={command_row:?}"
        );
        assert!(
            actions.right() <= command_row.right(),
            "请求操作区不能越出命令行: actions={actions:?}, command={command_row:?}"
        );
        if width >= 1280.0 {
            assert!(
                (actions.origin.y - command_row.origin.y).abs() <= px(3.0),
                "宽窗口操作区应与请求目标同排: actions={actions:?}, command={command_row:?}"
            );
        }
        if width >= 720.0 {
            assert!(
                (response.origin.y - request_pane.origin.y).abs() <= px(1.0),
                "左右分栏必须顶部对齐: pane={request_pane:?}, response={response:?}"
            );
            assert!(
                (response.bottom() - request_pane.bottom()).abs() <= px(1.0),
                "左右分栏必须共享完整高度: pane={request_pane:?}, response={response:?}"
            );
        } else {
            assert!(
                (sidebar.size.width - content_root.size.width).abs() <= px(1.0),
                "窄窗口侧栏应占满主体宽度: sidebar={sidebar:?}, content={content_root:?}"
            );
            assert!(
                sidebar.size.height <= px(240.0),
                "窄窗口侧栏高度应受限并保留请求编辑器空间: sidebar={sidebar:?}"
            );
        }
    }

    visual_cx.simulate_resize(size(px(360.0), px(240.0)));
    visual_cx.run_until_parked();
    let max_offset = visual_cx.update(|_, app| view.read(app).layout_scroll.max_offset());
    assert!(
        max_offset.y > px(0.0),
        "紧凑高度下 API 主体应提供垂直滚动范围: {max_offset:?}"
    );
    visual_cx.simulate_event(ScrollWheelEvent {
        position: point(px(20.0), px(24.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-80.0))),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    visual_cx.run_until_parked();
    assert!(visual_cx.update(|_, app| view.read(app).layout_scroll.offset().y < px(0.0)));
}
