use super::*;

use gpui_kit::{
    Modifiers, MouseButton, ScrollDelta, ScrollEvent, TestAppContext, TouchPhase,
    VisualTestContext, point, px, size,
};

fn drag_api_divider_to_x(
    cx: &mut VisualTestContext,
    start: gpui_kit::Point<gpui_kit::Pixels>,
    target_x: gpui_kit::Pixels,
) {
    let target = point(target_x, start.y);
    cx.simulate_mouse_move(start, None, Modifiers::default());
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.update(|window, app| {
        let _ = window.draw(app);
    });
    cx.simulate_mouse_move(target, Some(MouseButton::Left), Modifiers::default());
    cx.update(|window, app| {
        let _ = window.draw(app);
    });
    cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::default());
    cx.update(|window, app| {
        let _ = window.draw(app);
    });
    cx.run_until_parked();
}

#[gpui_kit::test]
fn api_sidebar_divider_resizes_and_clamps_to_available_width(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        gpui_kit::component::Root::new(view, window, cx)
    });
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.run_until_parked();

    let sidebar = visual_cx
        .debug_bounds("api-sidebar")
        .expect("桌面布局应显示 API 请求侧栏");
    assert!(
        (sidebar.size.width - px(API_SIDEBAR_WIDTH)).abs() <= px(1.0),
        "默认侧栏宽度应保持初始值 {API_SIDEBAR_WIDTH}px: {sidebar:?}"
    );
    let divider = visual_cx
        .debug_bounds("api-sidebar-divider")
        .expect("分隔栏应有可见的拖动区域");
    let y = divider.origin.y + px(20.0);
    let drag_start = point(divider.origin.x + divider.size.width / 2.0, y);
    drag_api_divider_to_x(visual_cx, drag_start, drag_start.x + px(90.0));
    let widened = visual_cx
        .debug_bounds("api-sidebar")
        .expect("拖动后侧栏应继续显示");
    assert!(
        (widened.size.width - px(API_SIDEBAR_WIDTH + 90.0)).abs() <= px(2.0),
        "向右拖动应同步增加侧栏宽度: before={sidebar:?}, after={widened:?}"
    );

    let divider = visual_cx
        .debug_bounds("api-sidebar-divider")
        .expect("侧栏加宽后分隔栏应继续显示");
    drag_api_divider_to_x(
        visual_cx,
        point(divider.origin.x + divider.size.width / 2.0, y),
        widened.origin.x + px(900.0),
    );
    let maximum = visual_cx
        .debug_bounds("api-sidebar")
        .expect("拖动到右侧后侧栏应继续显示");
    assert!(
        (maximum.size.width - px(API_SIDEBAR_MAX_WIDTH)).abs() <= px(1.0),
        "侧栏宽度不得超过最大值: {maximum:?}"
    );

    let divider = visual_cx
        .debug_bounds("api-sidebar-divider")
        .expect("侧栏达到最大宽度后分隔栏应继续显示");
    drag_api_divider_to_x(
        visual_cx,
        point(divider.origin.x + divider.size.width / 2.0, y),
        maximum.origin.x + px(1.0),
    );
    let minimum = visual_cx
        .debug_bounds("api-sidebar")
        .expect("拖动到左侧后侧栏应继续显示");
    assert!(
        (minimum.size.width - px(API_SIDEBAR_MIN_WIDTH)).abs() <= px(1.0),
        "侧栏宽度不得小于最小值: {minimum:?}"
    );
}

#[gpui_kit::test]
fn api_request_response_divider_resizes_and_preserves_minimum_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        gpui_kit::component::Root::new(view, window, cx)
    });
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.run_until_parked();

    let request_initial = visual_cx
        .debug_bounds("api-request-pane")
        .expect("桌面布局应显示请求面板");
    let response_initial = visual_cx
        .debug_bounds("api-response")
        .expect("桌面布局应显示响应面板");
    let divider = visual_cx
        .debug_bounds("api-request-response-divider")
        .expect("请求与响应之间应有可拖动分隔栏");
    let y = divider.origin.y + px(20.0);
    let drag_start = point(divider.origin.x + divider.size.width / 2.0, y);
    drag_api_divider_to_x(visual_cx, drag_start, drag_start.x + px(90.0));

    let request_widened = visual_cx
        .debug_bounds("api-request-pane")
        .expect("拖动后请求面板应继续显示");
    let response_narrowed = visual_cx
        .debug_bounds("api-response")
        .expect("拖动后响应面板应继续显示");
    assert!(
        (request_widened.size.width - request_initial.size.width - px(90.0)).abs() <= px(2.0),
        "向右拖动应增加请求区宽度: before={request_initial:?}, after={request_widened:?}"
    );
    assert!(
        (response_initial.size.width - response_narrowed.size.width - px(90.0)).abs() <= px(2.0),
        "向右拖动应同步减少响应区宽度: before={response_initial:?}, after={response_narrowed:?}"
    );

    let divider = visual_cx
        .debug_bounds("api-request-response-divider")
        .expect("请求区加宽后分隔栏应继续显示");
    drag_api_divider_to_x(
        visual_cx,
        point(divider.origin.x + divider.size.width / 2.0, y),
        px(1018.0),
    );
    let request_maximum = visual_cx
        .debug_bounds("api-request-pane")
        .expect("请求区达到最大宽度后应继续显示");
    let response_minimum = visual_cx
        .debug_bounds("api-response")
        .expect("响应区达到最小宽度后应继续显示");
    assert!(
        response_minimum.size.width >= px(API_RESPONSE_MIN_WIDTH - 1.0),
        "响应区不得小于 {API_RESPONSE_MIN_WIDTH}px: {response_minimum:?}"
    );
    assert!(
        request_maximum.size.width
            + response_minimum.size.width
            + px(API_REQUEST_RESPONSE_DIVIDER_WIDTH)
            <= px(1024.0 - API_SIDEBAR_WIDTH - API_SIDEBAR_DIVIDER_WIDTH + 2.0),
        "请求区和响应区必须留在工作区内: request={request_maximum:?}, response={response_minimum:?}"
    );

    let divider = visual_cx
        .debug_bounds("api-request-response-divider")
        .expect("请求区达到最大宽度后分隔栏应继续显示");
    drag_api_divider_to_x(
        visual_cx,
        point(divider.origin.x + divider.size.width / 2.0, y),
        request_initial.origin.x + px(API_REQUEST_PANE_MIN_WIDTH),
    );
    let request_minimum = visual_cx
        .debug_bounds("api-request-pane")
        .expect("请求区达到最小宽度后应继续显示");
    let response_widened = visual_cx
        .debug_bounds("api-response")
        .expect("请求区达到最小宽度后响应区应继续显示");
    assert!(
        request_minimum.size.width >= px(API_REQUEST_PANE_MIN_WIDTH - 1.0),
        "请求区不得小于 {API_REQUEST_PANE_MIN_WIDTH}px: {request_minimum:?}"
    );
    assert!(
        response_widened.size.width > response_minimum.size.width,
        "请求区缩至最小宽度后响应区应变宽: min={response_minimum:?}, after={response_widened:?}"
    );
}

/// 在 360px 视口中验证 HTTP 与 gRPC 目标字段及执行按钮都保留可用边界。
#[gpui_kit::test]
fn api_request_targets_keep_fields_and_actions_visible_on_narrow_windows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_kit::component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 视图应初始化");
    visual_cx.simulate_resize(size(px(360.0), px(640.0)));
    visual_cx.run_until_parked();

    let command_row = visual_cx
        .debug_bounds("api-request-command-row")
        .expect("窄屏请求命令行应渲染");
    let target = visual_cx
        .debug_bounds("api-request-target")
        .expect("HTTP 请求目标应渲染");
    let method = visual_cx
        .debug_bounds("api-http-method")
        .expect("HTTP 方法输入框应渲染");
    let url = visual_cx
        .debug_bounds("api-http-url")
        .expect("HTTP URL 输入框应渲染");
    let actions = visual_cx
        .debug_bounds("api-request-actions")
        .expect("HTTP 操作区应渲染");
    let send = visual_cx
        .debug_bounds("api-send")
        .expect("HTTP 发送按钮应渲染");
    let collection = visual_cx
        .debug_bounds("api-run-collection")
        .expect("Collection 按钮应渲染");
    assert!(
        method.size.width >= px(92.0) && url.size.width >= px(180.0),
        "窄屏 HTTP 方法与 URL 必须保留最小宽度: method={method:?}, url={url:?}"
    );
    assert!(
        method.origin.x >= target.origin.x
            && url.right() <= target.right()
            && method.bottom() <= target.bottom()
            && url.bottom() <= target.bottom(),
        "HTTP 方法和 URL 必须留在目标区内: method={method:?}, url={url:?}, target={target:?}"
    );
    assert!(
        actions.origin.y >= target.bottom()
            && actions.right() <= command_row.right()
            && send.right() <= actions.right()
            && collection.right() <= actions.right(),
        "窄屏 HTTP 操作区应换行并完整显示: target={target:?}, actions={actions:?}, send={send:?}, collection={collection:?}"
    );

    visual_cx.update(|_, app| {
        view.update(app, |view, cx| view.set_protocol(ApiProtocol::Grpc, cx));
    });
    visual_cx.run_until_parked();

    let target = visual_cx
        .debug_bounds("api-request-target")
        .expect("gRPC 请求目标应渲染");
    let endpoint = visual_cx
        .debug_bounds("api-grpc-endpoint")
        .expect("gRPC endpoint 输入框应渲染");
    let service = visual_cx
        .debug_bounds("api-grpc-service")
        .expect("gRPC service 输入框应渲染");
    let method = visual_cx
        .debug_bounds("api-grpc-method-input")
        .expect("gRPC method 输入框应渲染");
    let actions = visual_cx
        .debug_bounds("api-request-actions")
        .expect("gRPC 操作区应渲染");
    let discover = visual_cx
        .debug_bounds("api-grpc-discover")
        .expect("gRPC 服务发现按钮应渲染");
    let send = visual_cx
        .debug_bounds("api-send")
        .expect("gRPC 发送按钮应渲染");
    let collection = visual_cx
        .debug_bounds("api-run-collection")
        .expect("gRPC Collection 按钮应渲染");
    assert!(
        endpoint.size.width >= px(88.0)
            && service.size.width >= px(88.0)
            && method.size.width >= px(88.0),
        "窄屏 gRPC 目标、服务和方法输入框必须保留最小宽度: endpoint={endpoint:?}, service={service:?}, method={method:?}"
    );
    assert!(
        endpoint.origin.x >= target.origin.x
            && service.origin.x >= target.origin.x
            && method.origin.x >= target.origin.x
            && endpoint.right() <= target.right()
            && service.right() <= target.right()
            && method.right() <= target.right(),
        "gRPC 目标、服务和方法输入框必须留在目标区内: endpoint={endpoint:?}, service={service:?}, method={method:?}, target={target:?}"
    );
    assert!(
        actions.origin.y >= target.bottom()
            && discover.right() <= actions.right()
            && send.right() <= actions.right()
            && collection.right() <= actions.right(),
        "窄屏 gRPC 操作区应换行并完整显示: target={target:?}, actions={actions:?}, discover={discover:?}, send={send:?}, collection={collection:?}"
    );
}

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
    visual_cx
        .update(|_, app| view.update(app, |view, _| view.request_tab = ApiRequestTab::Advanced));

    for (width, height) in [
        (360.0, 240.0),
        (1024.0, 240.0),
        (360.0, 640.0),
        (640.0, 800.0),
        (720.0, 768.0),
        (1024.0, 768.0),
        (1202.0, 812.0),
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
        let content_root = visual_cx
            .debug_bounds("api-content")
            .expect("API 主体容器应渲染");
        let page_heading = visual_cx
            .debug_bounds("api-page-heading")
            .expect("请求名称和上下文应位于页面标题区");
        let page_title = visual_cx
            .debug_bounds("api-page-title")
            .expect("请求名称应保持可编辑");
        let page_actions = visual_cx
            .debug_bounds("api-page-actions")
            .expect("协议、导入和保存入口应位于页面标题区");
        let request_context = visual_cx
            .debug_bounds("api-request-context")
            .expect("标题区应保留 API 工作区上下文");
        let sidebar = visual_cx
            .debug_bounds("api-sidebar")
            .expect("API 侧栏应渲染");
        let request_search = visual_cx
            .debug_bounds("api-request-search")
            .expect("请求搜索框应渲染");
        let toolbar = visual_cx
            .debug_bounds("api-request-toolbar")
            .expect("请求工具栏应渲染");
        let request_tabs = visual_cx
            .debug_bounds("api-request-config-tabs")
            .expect("请求配置页签应渲染");
        let protocol_switcher = visual_cx
            .debug_bounds("api-protocol-switcher")
            .expect("协议切换器应渲染");
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
        let request_heading = visual_cx
            .debug_bounds("api-request-pane-heading")
            .expect("请求编辑区应有独立标题行");
        let request_scroll = visual_cx
            .debug_bounds("api-request-scroll-content")
            .expect("请求编辑内容应有独立滚动区域");
        let proxy_editor = visual_cx
            .debug_bounds("api-proxy-editor")
            .expect("代理编辑器应渲染");
        let response = visual_cx
            .debug_bounds("api-response")
            .expect("响应面板应渲染");
        let response_heading = visual_cx
            .debug_bounds("api-response-heading")
            .expect("响应区应有独立标题行");
        let response_tabs = visual_cx
            .debug_bounds("api-response-tabs-row")
            .expect("响应页签应位于单独一行");
        let response_scroll = visual_cx
            .debug_bounds("api-response-scroll-content")
            .expect("响应内容应有独立滚动区域");
        if width >= API_STACK_BREAKPOINT {
            assert!(
                editor.size.width >= px(API_EDITOR_MIN_WIDTH - 1.0),
                "桌面布局必须为编辑工作区保留最小宽度: viewport={width}, editor={editor:?}"
            );
        }
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
            page_title.origin.y >= page_heading.origin.y
                && page_title.bottom() <= page_heading.bottom()
                && request_context.bottom() <= page_heading.bottom(),
            "请求标题和上下文必须留在页面标题区: title={page_title:?}, context={request_context:?}, heading={page_heading:?}"
        );
        assert!(
            page_heading.bottom() < content_root.origin.y,
            "页面标题必须位于有边框的工作台之前: heading={page_heading:?}, workbench={content_root:?}"
        );
        assert!(
            page_actions.origin.y >= page_heading.origin.y
                && page_actions.bottom() <= page_heading.bottom(),
            "协议、导入和保存入口必须留在标题区: actions={page_actions:?}, heading={page_heading:?}"
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
            protocol_switcher.right() <= page_actions.right(),
            "协议切换器不能越出标题区操作范围"
        );
        assert!(
            (request_heading.size.height - px(ramag_ui::WORKBENCH_TOOLBAR_HEIGHT)).abs() <= px(1.0),
            "请求标题行应复用共享工具栏高度: {request_heading:?}"
        );
        assert!(
            (response_tabs.size.height - px(ramag_ui::WORKBENCH_TAB_HEIGHT)).abs() <= px(1.0),
            "响应页签行应复用共享页签高度: {response_tabs:?}"
        );
        assert!(
            request_tabs.origin.y >= request_pane.origin.y,
            "请求配置页签必须位于请求面板内"
        );
        assert!(
            request_heading.bottom() <= request_tabs.origin.y
                && request_scroll.origin.y >= request_heading.bottom(),
            "请求标题、请求编辑器和配置页签应按顺序排列: heading={request_heading:?}, tabs={request_tabs:?}, scroll={request_scroll:?}"
        );
        assert!(
            response_heading.bottom() <= response_tabs.origin.y
                && response_tabs.bottom() <= response_scroll.origin.y,
            "响应标题、页签和内容应按顺序排列: heading={response_heading:?}, tabs={response_tabs:?}, content={response_scroll:?}"
        );
        if width >= API_STACK_BREAKPOINT {
            assert!(
                (toolbar.origin.y - content_root.origin.y) <= px(3.0),
                "宽布局应从请求工作台顶栏开始: toolbar={toolbar:?}, content={content_root:?}"
            );
        } else {
            assert!(
                toolbar.origin.y >= sidebar.bottom(),
                "窄布局应先显示请求列表，再显示紧凑请求栏"
            );
        }
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
        if width >= API_STACK_BREAKPOINT {
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
                (sidebar.size.width - content_root.size.width).abs() <= px(4.0),
                "窄窗口侧栏应占满主体宽度: sidebar={sidebar:?}, content={content_root:?}"
            );
            assert!(
                sidebar.size.height <= px(240.0),
                "窄窗口侧栏高度应受限并保留请求编辑器空间: sidebar={sidebar:?}"
            );
            assert!(
                request_tabs.bottom() <= request_pane.bottom() + px(1.0),
                "窄屏请求页签必须完整留在请求面板内: viewport={width}x{height}, tabs={request_tabs:?}, pane={request_pane:?}"
            );
            assert!(
                response.origin.y >= request_pane.bottom() - px(1.0),
                "窄屏响应面板必须排在完整请求面板之后: viewport={width}x{height}, request={request_pane:?}, response={response:?}"
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
    visual_cx.simulate_event(ScrollEvent {
        position: point(px(20.0), px(24.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-80.0))),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    visual_cx.run_until_parked();
    assert!(visual_cx.update(|_, app| view.read(app).layout_scroll.offset().y < px(0.0)));

    visual_cx.simulate_resize(size(px(360.0), px(800.0)));
    visual_cx.run_until_parked();
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).layout_scroll.offset().y),
        px(0.0),
        "恢复常规窗口高度后页面应回到顶部"
    );
}

/// The desktop request pane must scroll all the way to its last connection fields.
#[gpui_kit::test]
fn desktop_request_pane_scroll_reaches_proxy_fields(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_kit::component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 视图应初始化");
    visual_cx
        .update(|_, app| view.update(app, |view, _| view.request_tab = ApiRequestTab::Advanced));
    visual_cx.simulate_resize(size(px(1202.0), px(812.0)));
    visual_cx.run_until_parked();
    let max_offset = visual_cx.update(|_, app| view.read(app).request_scroll.max_offset());
    assert!(max_offset.y > px(0.0));
    view.update(visual_cx, |view, cx| {
        view.request_scroll
            .set_offset(point(px(0.0), -max_offset.y));
        cx.notify();
    });
    visual_cx.run_until_parked();
    let pane = visual_cx
        .debug_bounds("api-request-pane")
        .expect("请求面板应存在");
    let proxy = visual_cx
        .debug_bounds("api-proxy-editor")
        .expect("代理字段应存在");
    assert!(proxy.size.height > px(0.0));
    assert!(
        proxy.bottom() <= pane.bottom() + px(1.0),
        "滚动到底后代理字段仍越出面板: {proxy:?} / {pane:?}"
    );
    assert!(
        proxy.origin.y >= pane.origin.y,
        "代理字段应完整可见: {proxy:?} / {pane:?}"
    );
}
