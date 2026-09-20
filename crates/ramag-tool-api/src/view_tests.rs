use super::*;

use gpui::{Modifiers, TestAppContext, VisualTestContext, point, px, size};
use ramag_domain::entities::{
    ApiAssertion, ApiAuth, ApiBody, ApiBodyMode, ApiCollection, ApiGrpcDescriptor, ApiParameter,
    ApiProtocol, ApiRequestRecord, ApiRequestSpec, ApiResponseSnapshot, ApiResponseSnapshotParts,
    ApiResponseStatus, ApiWorkspace, GrpcRequestSpec, HttpRequestSpec, import_api_json,
};

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
        let response = visual_cx
            .debug_bounds("api-response")
            .expect("响应面板应渲染");
        assert!(editor.right() <= root.right(), "编辑器不能越出根节点");
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
            workbench.bottom() <= content_root.bottom(),
            "请求响应工作区不能越出主体底部: workbench={workbench:?}, content={content_root:?}"
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
                response.bottom() <= workbench.bottom(),
                "响应面板不能越出工作区底部: response={response:?}, workbench={workbench:?}"
            );
            assert!(
                (response.bottom() - request_pane.bottom()).abs() <= px(1.0),
                "左右分栏必须共享完整高度: pane={request_pane:?}, response={response:?}"
            );
        }
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
    assert!(visual_cx.debug_bounds("api-http-query").is_some());
    assert!(visual_cx.debug_bounds("api-body-text").is_some());
    assert!(visual_cx.debug_bounds("api-body-multipart").is_some());
    assert!(visual_cx.debug_bounds("api-http-headers").is_some());
    assert!(visual_cx.debug_bounds("api-context-editor").is_some());
    assert!(visual_cx.debug_bounds("api-history").is_some());
    assert!(visual_cx.debug_bounds("api-request-target").is_some());
    assert!(visual_cx.debug_bounds("api-send").is_some());
    assert!(visual_cx.debug_bounds("api-save").is_some());
    assert!(visual_cx.debug_bounds("api-import").is_some());
    assert!(visual_cx.debug_bounds("api-run-collection").is_some());
    click(visual_cx, "api-protocol-grpc");
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("api-http-fields").is_none());
    assert!(visual_cx.debug_bounds("api-grpc-fields").is_some());
    assert!(visual_cx.debug_bounds("api-grpc-import-proto").is_some());
    assert!(
        visual_cx
            .debug_bounds("api-grpc-import-descriptor")
            .is_some()
    );
    assert_eq!(
        visual_cx.update(|_, cx| view.read(cx).protocol),
        ApiProtocol::Grpc
    );

    click(visual_cx, "api-protocol-http");
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("api-http-fields").is_some());
    assert!(visual_cx.debug_bounds("api-http-query").is_some());
    assert_eq!(
        visual_cx.update(|_, cx| view.read(cx).protocol),
        ApiProtocol::Http
    );
    click(visual_cx, "api-body-multipart");
    visual_cx.run_until_parked();
    assert_eq!(
        visual_cx.update(|_, cx| view.read(cx).http_body_mode),
        ApiBodyMode::Multipart
    );
    click(visual_cx, "api-body-text");
    visual_cx.run_until_parked();
    assert_eq!(
        visual_cx.update(|_, cx| view.read(cx).http_body_mode),
        ApiBodyMode::Text
    );
}

#[gpui::test]
fn api_grpc_descriptor_set_survives_request_build_and_import(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 视图应初始化");
    let descriptor = ApiGrpcDescriptor::FileDescriptorSet {
        bytes: vec![1, 2, 3],
    };

    visual_cx.update(|_, app| {
        view.update(app, |view, _| {
            view.protocol = ApiProtocol::Grpc;
            view.grpc_descriptor = descriptor.clone();
        });
    });
    let request = visual_cx.update(|_, app| {
        let view = view.read(app);
        request_from_view(view, app)
    });
    assert!(matches!(
        request,
        Ok(ApiRequestSpec::Grpc(GrpcRequestSpec {
            descriptor: ApiGrpcDescriptor::FileDescriptorSet { ref bytes },
            ..
        })) if bytes == &vec![1, 2, 3]
    ));

    let mut grpc_request = GrpcRequestSpec::new("http://127.0.0.1:50051", "api.Echo", "Unary");
    grpc_request.descriptor = descriptor;
    let mut workspace = ApiWorkspace::new("Imported");
    let mut collection = ApiCollection::new("gRPC");
    collection.requests.push(ApiRequestRecord::new_grpc(
        "Descriptor request",
        grpc_request,
    ));
    workspace.collections.push(collection);
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.workspace = workspace.clone();
            context::apply_imported_workspace(view, &workspace, window, cx);
        });
    });
    visual_cx.run_until_parked();

    let imported = visual_cx.update(|_, app| {
        let view = view.read(app);
        view.grpc_descriptor.clone()
    });
    assert!(matches!(
        imported,
        ApiGrpcDescriptor::FileDescriptorSet { bytes } if bytes == vec![1, 2, 3]
    ));
}

#[gpui::test]
fn api_imported_request_populates_editor_and_preserves_authentication(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 视图应初始化");
    let mut workspace = ApiWorkspace::new("Imported");
    let mut request = HttpRequestSpec::new("POST", "{{base_url}}/users");
    request
        .query
        .push(ApiParameter::new("q", "{{query}}", false));
    request.headers = vec![ApiParameter::new("Content-Type", "application/json", false)];
    request.auth = ApiAuth::Bearer {
        token: "{{token}}".into(),
    };
    request.body = Some(ApiBody::text(
        "{\"enabled\":true}",
        Some("application/json".into()),
    ));
    let mut collection = ApiCollection::new("Imported Collection");
    collection
        .requests
        .push(ApiRequestRecord::new_http("Imported Request", request));
    workspace.collections.push(collection);

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.workspace = workspace.clone();
            context::apply_imported_workspace(view, &workspace, window, cx);
        });
    });
    visual_cx.run_until_parked();

    let imported = visual_cx.update(|_, app| {
        let view = view.read(app);
        (
            view.request_name.read(app).value().to_string(),
            view.http_url.read(app).value().to_string(),
            view.http_query.read(app).value().to_string(),
            view.http_auth.clone(),
            view.http_body_content_type.clone(),
            view.http_body.read(app).value().to_string(),
        )
    });
    assert_eq!(imported.0, "Imported Request");
    assert_eq!(imported.1, "{{base_url}}/users");
    assert_eq!(imported.2, "q={{query}}");
    assert_eq!(
        imported.3,
        ApiAuth::Bearer {
            token: "{{token}}".into()
        }
    );
    assert_eq!(imported.4, "application/json");
    assert_eq!(imported.5, "{\"enabled\":true}");
}

#[gpui::test]
fn api_openapi_import_populates_request_editor_and_environment(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 视图应初始化");
    let document = serde_json::json!({
        "openapi": "3.0.3",
        "info": {"title": "Docker API", "version": "1.0.0"},
        "servers": [{
            "url": "http://127.0.0.1:18089/{version}",
            "variables": {"version": {"default": "api"}}
        }],
        "security": [{"bearerAuth": []}],
        "components": {
            "securitySchemes": {
                "bearerAuth": {"type": "http", "scheme": "bearer"}
            }
        },
        "paths": {
            "/json": {
                "post": {
                    "operationId": "apiJson",
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "example": {"ok": true}
                            }
                        }
                    },
                    "responses": {"200": {"description": "ok"}}
                }
            }
        }
    });
    let bundle = import_api_json(&document.to_string()).expect("OpenAPI 导入应成功");
    let mut workspace = ApiWorkspace::new("Imported");
    let summary = bundle
        .merge_into(&mut workspace)
        .expect("OpenAPI 工作区合并应成功");
    assert_eq!(summary.format.label(), "OpenAPI 3 JSON");
    assert_eq!(summary.request_count, 1);

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.workspace = workspace.clone();
            context::apply_imported_workspace(view, &workspace, window, cx);
        });
    });
    visual_cx.run_until_parked();

    let imported = visual_cx.update(|_, app| {
        let view = view.read(app);
        (
            view.request_name.read(app).value().to_string(),
            view.http_method.read(app).value().to_string(),
            view.http_url.read(app).value().to_string(),
            view.http_body_content_type.clone(),
            view.http_body.read(app).value().to_string(),
            view.environment_variables.read(app).value().to_string(),
            view.environment_sensitive.read(app).value().to_string(),
            view.http_auth.clone(),
        )
    });
    assert_eq!(imported.0, "apiJson");
    assert_eq!(imported.1, "POST");
    assert_eq!(imported.2, "http://127.0.0.1:18089/{{version}}/json");
    assert_eq!(imported.3, "application/json");
    assert_eq!(imported.4, "{\"ok\":true}");
    assert!(imported.5.contains("version=api"));
    assert_eq!(imported.6, "openapi_bearerAuth_token");
    assert_eq!(
        imported.7,
        ApiAuth::Bearer {
            token: "{{openapi_bearerAuth_token}}".into()
        }
    );
}

#[test]
fn api_assertion_editor_format_parses_supported_rules_and_rejects_unknown_types() {
    let assertions = context::parse_assertions(
        "status=200\nheader=content-type:application/json\nbody=ok\njson=$.ok:true\nlatency=500",
        ApiProtocol::Http,
    )
    .expect("断言编辑格式应解析");
    assert_eq!(assertions.len(), 5);
    assert!(matches!(
        assertions[0],
        ApiAssertion::HttpStatus { expected: 200 }
    ));
    assert!(context::parse_assertions("metadata=server:api", ApiProtocol::Http).is_err());
    assert!(context::parse_assertions("unknown=value", ApiProtocol::Http).is_err());
}

#[test]
fn api_environment_editor_format_parses_values_and_sensitive_references() {
    let environment =
        context::parse_environment("base_url=http://127.0.0.1:18089\ntoken=secret", "token")
            .expect("环境变量编辑格式应解析");
    assert_eq!(
        environment.variable("base_url"),
        Some("http://127.0.0.1:18089")
    );
    assert_eq!(environment.sensitive_variable_refs, vec!["token"]);
    assert!(context::parse_environment("base_url", "").is_err());
    assert!(context::parse_environment("base_url=x", "missing").is_err());
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
fn api_query_editor_format_parses_name_value_pairs_and_rejects_missing_name() {
    let query = parse_http_query("q=hello world\npage=2");
    assert_eq!(
        query.expect("查询参数应解析"),
        vec![
            ApiParameter::new("q", "hello world", false),
            ApiParameter::new("page", "2", false),
        ]
    );
    assert!(parse_http_query("q").is_err());
    assert!(parse_http_query("=missing").is_err());
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
    click(visual_cx, "api-run-collection");
    visual_cx.run_until_parked();
    assert_eq!(
        visual_cx.update(|_, cx| view.read(cx).notice.as_ref().map(|value| value.1)),
        Some(true)
    );
    click(visual_cx, "api-import");
    visual_cx.run_until_parked();
    assert_eq!(
        visual_cx.update(|_, cx| view.read(cx).notice.as_ref().map(|value| value.1)),
        Some(true)
    );
}
