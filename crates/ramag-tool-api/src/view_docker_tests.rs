use super::*;

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::{Modifiers, TestAppContext, VisualTestContext, point, px, size};
use ramag_app::ApiService;
use ramag_domain::entities::{ApiProtocol, ApiResponseStatus};
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
    cx.simulate_mouse_down(center, gpui_kit::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(center, gpui_kit::MouseButton::Left, Modifiers::default());
}

fn wait_for_response(
    cx: &mut VisualTestContext,
    view: &gpui_kit::Entity<ApiView>,
) -> Option<ApiResponseSnapshot> {
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

#[gpui_kit::test]
fn api_workbench_sends_http_and_grpc_requests_to_local_docker_services(cx: &mut TestAppContext) {
    if std::net::TcpStream::connect(("127.0.0.1", 18089)).is_err()
        || std::net::TcpStream::connect(("127.0.0.1", 18090)).is_err()
    {
        eprintln!("跳过 API UI Docker 集成测试；HTTP 18089 或 gRPC 18090 未监听");
        return;
    }

    cx.update(gpui_kit::component::init);
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
        gpui_kit::component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API UI 视图应初始化");
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.run_until_parked();

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.assertions
                .update(cx, |input, cx| input.set_value("status=200", window, cx));
        });
    });
    click(visual_cx, "api-send");
    let http_response = wait_for_response(visual_cx, &view);
    assert!(matches!(
        http_response.as_ref().map(|response| &response.status),
        Some(ApiResponseStatus::Http { code: 200 })
    ));
    assert!(
        String::from_utf8_lossy(&http_response.expect("HTTP UI 响应").body)
            .contains("ramag-api-http-test")
    );
    assert!(visual_cx.update(|_, cx| {
        view.read(cx)
            .assertion_results
            .first()
            .is_some_and(|result| result.passed)
    }));
    assert!(visual_cx.debug_bounds("api-assertion-results").is_some());

    click(visual_cx, "api-run-collection");
    let collection_response = wait_for_response(visual_cx, &view);
    assert!(matches!(
        collection_response
            .as_ref()
            .map(|response| &response.status),
        Some(ApiResponseStatus::Http { code: 200 })
    ));
    assert!(visual_cx.debug_bounds("api-collection-summary").is_some());

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.assertions
                .update(cx, |input, cx| input.set_value("", window, cx));
        });
    });
    click(visual_cx, "api-protocol-grpc");
    visual_cx.run_until_parked();
    click(visual_cx, "api-grpc-discover");
    for _ in 0..100 {
        visual_cx.run_until_parked();
        if !visual_cx.update(|_, cx| view.read(cx).grpc_discovering) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let discovery = visual_cx.update(|_, cx| {
        let view = view.read(cx);
        (
            view.grpc_services.len(),
            view.grpc_services
                .first()
                .map(|service| service.name.clone()),
            view.grpc_services
                .first()
                .and_then(|service| service.methods.first())
                .map(|method| method.name.clone()),
            view.grpc_service.read(cx).value(),
            view.grpc_method.read(cx).value(),
        )
    });
    assert_eq!(discovery.0, 1, "gRPC UI 应显示 Docker Service 目录");
    assert_eq!(discovery.1.as_deref(), Some("api.docker.Echo"));
    assert_eq!(discovery.2.as_deref(), Some("Unary"));
    assert_eq!(discovery.3, "api.docker.Echo");
    assert_eq!(discovery.4, "Unary");
    assert!(visual_cx.debug_bounds("api-grpc-catalog").is_some());

    click(visual_cx, "api-send");
    let grpc_response = wait_for_response(visual_cx, &view);
    assert!(matches!(
        grpc_response.as_ref().map(|response| &response.status),
        Some(ApiResponseStatus::Grpc { code }) if code == "ok"
    ));
    assert!(
        String::from_utf8_lossy(&grpc_response.expect("gRPC UI 响应").body).contains("docker echo")
    );
    assert_eq!(
        visual_cx.update(|_, cx| view.read(cx).protocol),
        ApiProtocol::Grpc
    );
}
