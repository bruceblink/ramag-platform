//! 已保存请求经过真实编辑器、加密存储及本机 Docker 服务的往返回归。

use super::*;
use ramag_domain::entities::{ApiCollection, ApiRequestRecord, ApiWorkspace};
use ramag_domain::traits::Storage as _;

#[gpui_kit::test]
#[ignore = "需要 scripts/api-test 的本机 HTTP 与 gRPC Docker 测试服务"]
fn saved_requests_roundtrip_through_encrypted_storage_and_local_services(cx: &mut TestAppContext) {
    for port in [18089, 18090] {
        assert!(
            std::net::TcpStream::connect(("127.0.0.1", port)).is_ok(),
            "本机 API Docker 测试服务未就绪"
        );
    }
    cx.update(gpui_kit::component::init);
    cx.executor().allow_parking();
    let directory = tempdir().expect("创建 API 往返测试目录");
    let storage = Arc::new(
        RedbStorage::open_with_key(&directory.path().join("roundtrip.redb"), &[0x53; 32])
            .expect("打开隔离加密测试存储"),
    );
    let service = Arc::new(
        ApiService::new(
            Arc::new(HttpApiDriver::new().expect("创建 HTTP 驱动")),
            Arc::new(GrpcApiDriver::new().expect("创建 gRPC 驱动")),
            storage.clone(),
        )
        .expect("创建 API 服务"),
    );
    let mut http = HttpRequestSpec::new("GET", "http://127.0.0.1:18089/echo/roundtrip");
    http.timeout_millis = 37_000;
    http.query = vec![
        ApiParameter::new("item", "first", false),
        ApiParameter::new("item", "second", true),
    ];
    http.headers = vec![ApiParameter::new("X-Request-Token", "roundtrip", true)];
    let mut grpc = GrpcRequestSpec::new("http://127.0.0.1:18090", "api.docker.Echo", "Unary");
    grpc.timeout_millis = 43_000;
    grpc.message = r#"{"message":"roundtrip"}"#.into();
    grpc.metadata = vec![
        ApiParameter::new("x-request", "docker", true),
        ApiParameter::new("x-extra", "first", false),
        ApiParameter::new("x-extra", "second", true),
        ApiParameter::new("x-tail", "last", false),
    ];
    let records = [
        ApiRequestRecord::new_http("HTTP 往返", http),
        ApiRequestRecord::new_grpc("gRPC 往返", grpc),
    ];
    let mut collection = ApiCollection::new("往返请求");
    collection.requests.extend(records.iter().cloned());
    let mut workspace = ApiWorkspace::new("隔离往返验收");
    workspace.collections.push(collection);
    futures::executor::block_on(service.save_workspace(&workspace)).expect("保存初始测试工作区");

    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::with_service(service, window, cx));
        view_entity = Some(view.clone());
        gpui_kit::component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("创建 API 往返视图");
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

    for (selector, expected) in ["api-request-item-0", "api-request-item-1"]
        .into_iter()
        .zip(&records)
    {
        click(visual_cx, selector);
        assert!(visual_cx.update(|_, app| {
            request_from_view(view.read(app), app).is_ok_and(|request| request == expected.request)
        }));
        click(visual_cx, "api-save");
        for _ in 0..100 {
            visual_cx.run_until_parked();
            if !visual_cx.update(|_, app| view.read(app).saving) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            visual_cx.update(|_, app| view.read(app).notice.clone()),
            Some(("请求、环境和断言已保存".into(), false))
        );
        let saved =
            futures::executor::block_on(storage.list_api_workspaces()).expect("重新读取加密工作区");
        let saved_record = saved
            .iter()
            .flat_map(|workspace| &workspace.collections)
            .flat_map(|collection| &collection.requests)
            .find(|record| record.id == expected.id)
            .expect("保存后原请求仍存在");
        assert!(saved_record == expected, "存储往返必须保留完整请求");

        click(visual_cx, "api-send");
        let response = wait_for_response(visual_cx, &view).expect("本机服务应返回响应");
        match &response.status {
            ApiResponseStatus::Http { code: 200 } => {
                let json: serde_json::Value =
                    serde_json::from_slice(&response.body).expect("解析 HTTP 回显");
                assert_eq!(
                    json["query"]["item"],
                    serde_json::json!(["first", "second"])
                );
                assert_eq!(json["request_token"], "roundtrip");
            }
            ApiResponseStatus::Grpc { code } if code == "ok" => {
                assert!(String::from_utf8_lossy(&response.body).contains("docker echo: roundtrip"));
            }
            _ => panic!("本机 API 服务请求应成功"),
        }
        assert!(visual_cx.update(|_, app| {
            request_from_view(view.read(app), app).is_ok_and(|request| request == expected.request)
        }));
    }
}
