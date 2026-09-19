use super::*;

use std::sync::Arc;
use std::time::Duration;

use gpui::{Modifiers, TestAppContext, VisualTestContext, point, px, size};
use ramag_app::ApiService;
use ramag_domain::entities::{
    ApiParameter, ApiProtocol, ApiResponseSnapshot, ApiResponseSnapshotParts, ApiResponseStatus,
};
use ramag_domain::traits::ApiDriver;
use ramag_infra_api::{GrpcApiDriver, HttpApiDriver};
use ramag_infra_storage::RedbStorage;
use tempfile::tempdir;

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("控件应参与布局: {selector}"));
    let center = point(
        bounds.origin.x + bounds.size.width / 2.0,
        bounds.origin.y + bounds.size.height / 2.0,
    );
    cx.simulate_mouse_move(center, None, Modifiers::default());
    cx.simulate_mouse_down(center, gpui::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(center, gpui::MouseButton::Left, Modifiers::default());
}

fn wait_for_response(
    cx: &mut VisualTestContext,
    view: &gpui::Entity<ApiView>,
) -> Option<ramag_domain::entities::ApiResponseSnapshot> {
    for _ in 0..100 {
        cx.run_until_parked();
        let response = cx.update(|_, app| view.read(app).response.clone());
        if response.is_some() {
            return response;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    cx.update(|_, app| view.read(app).response.clone())
}

#[gpui::test]
fn api_workbench_reflows_request_editor_and_response_at_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let _view = view_entity.expect("API 视图应初始化");

    for (width, height) in [
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
            .debug_bounds("api-request-editor")
            .expect("请求编辑器应渲染");
        let response = visual_cx
            .debug_bounds("api-response")
            .expect("响应面板应渲染");
        assert!(editor.right() <= root.right(), "编辑器不能越出根节点");
        assert!(response.right() <= root.right(), "响应面板不能越出根节点");
        assert!(editor.origin.x >= root.origin.x, "编辑器左边界不能越界");
    }
}

#[gpui::test]
fn api_protocol_switch_changes_editor_and_send_controls_remain_visible(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 视图应初始化");
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.run_until_parked();

    assert!(visual_cx.debug_bounds("api-http-fields").is_some());
    assert!(visual_cx.debug_bounds("api-http-headers").is_some());
    assert!(visual_cx.debug_bounds("api-send").is_some());
    assert!(visual_cx.debug_bounds("api-save").is_some());
    click(visual_cx, "api-protocol-grpc");
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("api-http-fields").is_none());
    assert!(visual_cx.debug_bounds("api-grpc-fields").is_some());
    assert_eq!(
        visual_cx.update(|_, cx| view.read(cx).protocol),
        ApiProtocol::Grpc
    );

    click(visual_cx, "api-protocol-http");
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("api-http-fields").is_some());
    assert_eq!(
        visual_cx.update(|_, cx| view.read(cx).protocol),
        ApiProtocol::Http
    );
}

#[test]
fn api_headers_parse_as_name_value_pairs_and_report_invalid_lines() {
    let headers = parse_http_headers("Content-Type: application/json\nX-Test: enabled")
        .expect("有效请求头应解析");
    assert_eq!(
        headers,
        vec![
            ApiParameter::new("Content-Type", "application/json", false),
            ApiParameter::new("X-Test", "enabled", false),
        ]
    );
    assert!(parse_http_headers("Invalid Header").is_err());
    assert!(parse_http_headers("X-Empty:   ").is_err());
}

#[test]
fn api_response_formats_json_and_preserves_non_json_body() {
    let json_body = br#"{"ok":true,"nested":{"id":1}}"#.to_vec();
    let json_snapshot = ApiResponseSnapshot::new(ApiResponseSnapshotParts {
        protocol: ApiProtocol::Http,
        status: ApiResponseStatus::Http { code: 200 },
        headers: vec![ApiParameter::new("Content-Type", "application/json", false)],
        metadata: Vec::new(),
        size_bytes: json_body.len() as u64,
        body: json_body,
        elapsed_millis: 2,
        truncated: false,
        error: None,
    })
    .expect("构造 JSON 响应");
    assert_eq!(body_format_label(&json_snapshot), "JSON");
    assert!(body_preview(&json_snapshot).contains("\n  \"ok\": true"));

    let raw_body = b"not-json".to_vec();
    let raw_snapshot = ApiResponseSnapshot::new(ApiResponseSnapshotParts {
        protocol: ApiProtocol::Http,
        status: ApiResponseStatus::Http { code: 200 },
        headers: Vec::new(),
        metadata: Vec::new(),
        size_bytes: raw_body.len() as u64,
        body: raw_body,
        elapsed_millis: 1,
        truncated: false,
        error: None,
    })
    .expect("构造原文响应");
    assert_eq!(body_format_label(&raw_snapshot), "原文");
    assert_eq!(body_preview(&raw_snapshot), "not-json");
}

#[gpui::test]
fn api_without_service_explains_send_and_save_state_without_panicking(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 视图应初始化");
    visual_cx.run_until_parked();

    click(visual_cx, "api-send");
    visual_cx.run_until_parked();
    assert_eq!(
        visual_cx.update(|_, cx| view.read(cx).notice.as_ref().map(|value| value.1)),
        Some(true)
    );
    click(visual_cx, "api-save");
    visual_cx.run_until_parked();
    assert_eq!(
        visual_cx.update(|_, cx| view.read(cx).notice.as_ref().map(|value| value.1)),
        Some(true)
    );
}

#[gpui::test]
fn api_workbench_sends_http_and_grpc_requests_to_local_docker_fixtures(cx: &mut TestAppContext) {
    if std::net::TcpStream::connect(("127.0.0.1", 18089)).is_err()
        || std::net::TcpStream::connect(("127.0.0.1", 18090)).is_err()
    {
        eprintln!("跳过 API UI Docker 集成测试；HTTP 18089 或 gRPC 18090 未监听");
        return;
    }

    cx.update(gpui_component::init);
    cx.executor().allow_parking();
    let directory = tempdir().expect("创建 API UI 测试目录");
    let storage = RedbStorage::open_with_key(&directory.path().join("api-ui.redb"), &[0x47; 32])
        .expect("打开 API UI 测试存储");
    let http_driver: Arc<dyn ApiDriver> = Arc::new(HttpApiDriver::new().expect("创建 HTTP 驱动"));
    let grpc_driver: Arc<dyn ApiDriver> = Arc::new(GrpcApiDriver::new().expect("创建 gRPC 驱动"));
    let service = Arc::new(
        ApiService::new(http_driver, grpc_driver, Arc::new(storage)).expect("创建 API UI 测试服务"),
    );
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::with_service(service, window, cx));
        view_entity = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API UI 视图应初始化");
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.run_until_parked();

    click(visual_cx, "api-send");
    let http_response = wait_for_response(visual_cx, &view);
    assert!(
        matches!(
            http_response.as_ref().map(|response| &response.status),
            Some(ApiResponseStatus::Http { code: 200 })
        ),
        "HTTP UI response={http_response:?}, notice={:?}",
        visual_cx.update(|_, cx| view.read(cx).notice.clone())
    );
    assert!(
        String::from_utf8_lossy(&http_response.expect("HTTP UI 响应").body)
            .contains("ramag-api-http-test")
    );

    click(visual_cx, "api-protocol-grpc");
    visual_cx.run_until_parked();
    click(visual_cx, "api-send");
    let grpc_response = wait_for_response(visual_cx, &view);
    assert!(
        matches!(
            grpc_response.as_ref().map(|response| &response.status),
            Some(ApiResponseStatus::Grpc { code }) if code == "ok"
        ),
        "gRPC UI response={grpc_response:?}, notice={:?}",
        visual_cx.update(|_, cx| view.read(cx).notice.clone())
    );
    assert!(
        String::from_utf8_lossy(&grpc_response.expect("gRPC UI 响应").body).contains("docker echo")
    );
    assert_eq!(
        visual_cx.update(|_, cx| view.read(cx).protocol),
        ApiProtocol::Grpc
    );
}
