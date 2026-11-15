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
#[ignore = "需要本机 HTTP 与 gRPC Docker 测试服务；启动服务后使用 cargo test -- --ignored 显式运行"]
fn api_workbench_sends_http_and_grpc_requests_to_local_docker_services(cx: &mut TestAppContext) {
    assert!(
        std::net::TcpStream::connect(("127.0.0.1", 18089)).is_ok(),
        "本机 HTTP Docker 测试服务未监听 127.0.0.1:18089；请先启动 scripts/api-test/api-test.ps1"
    );
    assert!(
        std::net::TcpStream::connect(("127.0.0.1", 18090)).is_ok(),
        "本机 gRPC Docker 测试服务未监听 127.0.0.1:18090；请先启动 scripts/api-test/grpc-test.ps1"
    );

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
    click(visual_cx, "api-response-tab-assertions");
    assert!(visual_cx.debug_bounds("api-assertion-results").is_some());

    click(visual_cx, "api-run-collection");
    let collection_response = wait_for_response(visual_cx, &view);
    assert!(matches!(
        collection_response
            .as_ref()
            .map(|response| &response.status),
        Some(ApiResponseStatus::Http { code: 200 })
    ));
    click(visual_cx, "api-response-tab-assertions");
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

#[gpui_kit::test]
#[ignore = "需要本机 Docker HTTP/gRPC 服务；显式运行以验证延迟请求后的侧栏切换"]
fn saved_request_switches_keep_current_response_and_environment_with_real_docker_drivers(
    cx: &mut TestAppContext,
) {
    use ramag_domain::entities::{
        ApiCollection, ApiEnvironment, ApiParameter, ApiRequestRecord, ApiVariableExtraction,
        ApiVariableSource, ApiWorkspace, GrpcRequestSpec, HttpRequestSpec,
    };

    for port in [18089, 18090] {
        assert!(
            std::net::TcpStream::connect(("127.0.0.1", port)).is_ok(),
            "本机 Docker API 测试服务未监听 {port}"
        );
    }
    cx.update(gpui_kit::component::init);
    cx.executor().allow_parking();
    let directory = tempdir().expect("创建 API 切换测试目录");
    let storage = Arc::new(
        RedbStorage::open_with_key(&directory.path().join("switch.redb"), &[0x49; 32])
            .expect("打开 API 切换测试存储"),
    );
    let service = Arc::new(
        ApiService::new(
            Arc::new(HttpApiDriver::new().expect("创建真实 HTTP 驱动")),
            Arc::new(GrpcApiDriver::new().expect("创建真实 gRPC 驱动")),
            storage,
        )
        .expect("装配 API 切换测试服务"),
    );
    let mut workspace = ApiWorkspace::new("Docker 切换测试");
    let mut environment = ApiEnvironment::new("local");
    environment
        .variables
        .insert("marker".into(), "baseline".into());
    workspace.default_environment_id = Some(environment.id.clone());
    workspace.environments.push(environment);
    let mut collection = ApiCollection::new("本机请求");
    for (name, path) in [("慢 HTTP", "/delay?ms=600"), ("快 HTTP", "/json")] {
        collection.requests.push(ApiRequestRecord::new_http(
            name,
            HttpRequestSpec::new("GET", format!("http://127.0.0.1:18089{path}")),
        ));
    }
    for (name, message) in [("慢 gRPC", "delay:600"), ("快 gRPC", "current")] {
        let mut spec = GrpcRequestSpec::new("http://127.0.0.1:18090", "api.docker.Echo", "Unary");
        spec.message = serde_json::json!({ "message": message }).to_string();
        spec.metadata
            .push(ApiParameter::new("x-request", "docker", false));
        collection
            .requests
            .push(ApiRequestRecord::new_grpc(name, spec));
    }
    for index in [0, 2] {
        collection.requests[index]
            .response_variables
            .push(ApiVariableExtraction {
                name: "marker".into(),
                source: ApiVariableSource::Body,
                sensitive: false,
            });
    }
    workspace.collections.push(collection);
    futures::executor::block_on(service.save_workspace(&workspace))
        .expect("保存 Docker 切换验收工作区");
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::with_service(service, window, cx));
        view_entity = Some(view.clone());
        gpui_kit::component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 切换测试视图");
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    for _ in 0..100 {
        visual_cx.run_until_parked();
        if visual_cx.update(|_, app| view.read(app).workspace_load_state)
            == ApiWorkspaceLoadState::Loaded
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).workspace_load_state),
        ApiWorkspaceLoadState::Loaded
    );

    for (slow, current, protocol) in [
        (
            "api-request-item-0",
            "api-request-item-1",
            ApiProtocol::Http,
        ),
        (
            "api-request-item-0",
            "api-request-item-3",
            ApiProtocol::Grpc,
        ),
        (
            "api-request-item-2",
            "api-request-item-1",
            ApiProtocol::Http,
        ),
        (
            "api-request-item-2",
            "api-request-item-3",
            ApiProtocol::Grpc,
        ),
    ] {
        click(visual_cx, slow);
        click(visual_cx, "api-send");
        let cancellation = visual_cx
            .update(|_, app| view.read(app).cancelled.clone())
            .expect("延迟请求应处于执行中");
        // 允许真实驱动进入 HTTP/gRPC 服务；迟到回调的精确顺序另由受控驱动测试验证。
        std::thread::sleep(Duration::from_millis(100));
        click(visual_cx, current);
        assert!(cancellation.load(std::sync::atomic::Ordering::Relaxed));
        assert!(!visual_cx.update(|_, app| view.read(app).loading));
        click(visual_cx, "api-send");
        let snapshot = wait_for_response(visual_cx, &view).expect("新请求应完成");
        assert_eq!(snapshot.protocol, protocol);
        let body = String::from_utf8_lossy(&snapshot.body);
        assert!(
            body.contains(if protocol == ApiProtocol::Http {
                "ramag-api-http-test"
            } else {
                "docker echo: current"
            }),
            "应显示当前请求的响应：{body}"
        );
        let notice = visual_cx.update(|_, app| view.read(app).notice.clone());
        for _ in 0..40 {
            visual_cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(20));
        }
        visual_cx.update(|_, app| {
            let current_view = view.read(app);
            assert_eq!(current_view.response.as_ref(), Some(&snapshot));
            assert_eq!(current_view.notice, notice);
            assert_eq!(
                current_view
                    .runtime_environment
                    .variables
                    .get("marker")
                    .map(String::as_str),
                Some("baseline")
            );
            assert!(!current_view.loading);
            assert!(current_view.cancelled.is_none());
        });
    }
}
