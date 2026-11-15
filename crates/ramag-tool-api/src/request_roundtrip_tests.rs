use super::*;
use crate::view::context::request_record;

use ramag_domain::entities::{
    ApiBody, ApiParameter, ApiProtocol, ApiRequestRecord, ApiRequestSpec, ApiWorkspace,
    GrpcRequestSpec, HttpRequestSpec,
};

fn http_record() -> ApiRequestRecord {
    let mut request = HttpRequestSpec::new("POST", "http://127.0.0.1/original");
    request.timeout_millis = 37_000;
    request.query = vec![
        ApiParameter::new("token", "查询敏感值一", true),
        ApiParameter::new("page", "  第 2 页  ", false),
        ApiParameter::new("token", "查询敏感值二", false),
    ];
    request.headers = vec![
        ApiParameter::new("X-Secret", "请求头敏感值", true),
        ApiParameter::new("X-Empty", "", false),
        ApiParameter::new("X-Trace", "跟踪原值", false),
    ];
    request.body = Some(ApiBody::text("  带首尾空格的正文  ", None));
    ApiRequestRecord::new_http("HTTP 原始请求", request)
}

fn grpc_record(metadata: Vec<ApiParameter>) -> ApiRequestRecord {
    let mut request = GrpcRequestSpec::new("http://127.0.0.1:18090", "api.docker.Echo", "Unary");
    request.message = r#"{"message":"原始内容"}"#.into();
    request.timeout_millis = 53_000;
    request.metadata = metadata;
    ApiRequestRecord::new_grpc("gRPC 原始请求", request)
}

fn workspace_with(requests: Vec<ApiRequestRecord>) -> ApiWorkspace {
    let mut workspace = ApiWorkspace::new("API request round-trip tests");
    let mut collection = ApiCollection::new("Saved requests");
    collection.requests = requests;
    workspace.collections.push(collection);
    workspace
}

fn save_and_read(
    visual_cx: &mut VisualTestContext,
    view: &Entity<ApiView>,
    storage: &WorkspaceTestStorage,
    request_id: &ramag_domain::entities::ApiRequestId,
) -> ApiRequestRecord {
    click(visual_cx, "api-save");
    for _ in 0..200 {
        visual_cx.run_until_parked();
        if !visual_cx.update(|_, app| view.read(app).saving) {
            break;
        }
        std::thread::yield_now();
    }
    assert!(
        !visual_cx.update(|_, app| view.read(app).saving),
        "保存应在有界等待内完成"
    );
    let saved = storage
        .workspaces
        .lock()
        .expect("锁定测试工作区")
        .iter()
        .flat_map(|workspace| workspace.collections.iter())
        .flat_map(|collection| collection.requests.iter())
        .find(|record| &record.id == request_id)
        .cloned()
        .expect("保存后的请求应存在于 Storage");
    assert_saved_notice(visual_cx, view);
    saved
}

fn assert_editor_matches_record(
    visual_cx: &mut VisualTestContext,
    view: &Entity<ApiView>,
    expected: &ApiRequestRecord,
) {
    let mut rebuilt = visual_cx
        .update(|_, app| request_record(view.read(app), app))
        .expect("编辑器应能重建有效请求");
    // `request_record` 会生成候选 ID；保存时应保留当前记录的 ID。
    rebuilt.id = expected.id.clone();
    assert_eq!(rebuilt, *expected);
}

fn assert_saved_notice(visual_cx: &mut VisualTestContext, view: &Entity<ApiView>) {
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).notice.clone()),
        Some(("请求、环境和断言已保存".into(), false))
    );
}

#[gpui_kit::test]
fn saved_http_request_round_trips_without_edits_and_redacts_sensitive_debug(
    cx: &mut TestAppContext,
) {
    let original = http_record();
    let request_id = original.id.clone();
    let workspace = workspace_with(vec![original.clone()]);
    let storage = Arc::new(WorkspaceTestStorage::new(false, vec![workspace]));
    let (view, visual_cx) = add_view(cx, storage.clone());
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );

    click(visual_cx, "api-request-item-0");
    assert_editor_matches_record(visual_cx, &view, &original);
    let debug = format!("{:?}", original.request);
    assert!(!debug.contains("查询敏感值一"));
    assert!(!debug.contains("请求头敏感值"));
    assert!(debug.contains("[REDACTED]"));
    assert_eq!(
        storage.workspaces.lock().expect("锁定测试工作区")[0].collections[0].requests[0],
        original,
        "fixture must retain original body and parameter bytes"
    );

    let saved = save_and_read(visual_cx, &view, &storage, &request_id);
    assert_eq!(saved, original);
    assert_saved_notice(visual_cx, &view);
}

#[gpui_kit::test]
fn editing_only_http_method_preserves_other_saved_fields(cx: &mut TestAppContext) {
    let original = http_record();
    let request_id = original.id.clone();
    let storage = Arc::new(WorkspaceTestStorage::new(
        false,
        vec![workspace_with(vec![original.clone()])],
    ));
    let (view, visual_cx) = add_view(cx, storage.clone());
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );
    click(visual_cx, "api-request-item-0");

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.http_method
                .update(cx, |input, cx| input.set_value("PATCH", window, cx));
        });
    });
    let mut expected = original;
    let ApiRequestSpec::Http(spec) = &mut expected.request else {
        panic!("测试请求应使用 HTTP 协议");
    };
    spec.method = "PATCH".into();
    assert_editor_matches_record(visual_cx, &view, &expected);
    assert_eq!(
        save_and_read(visual_cx, &view, &storage, &request_id),
        expected
    );
}

#[gpui_kit::test]
fn http_parameter_edits_reorder_preserve_sensitive_flags_and_add_values(cx: &mut TestAppContext) {
    let original = http_record();
    let request_id = original.id.clone();
    let storage = Arc::new(WorkspaceTestStorage::new(
        false,
        vec![workspace_with(vec![original.clone()])],
    ));
    let (view, visual_cx) = add_view(cx, storage.clone());
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );
    click(visual_cx, "api-request-item-0");

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.http_method
                .update(cx, |input, cx| input.set_value("PUT", window, cx));
            view.http_query.update(cx, |input, cx| {
                input.set_value(
                    "token=edited-one\npage=3\ntoken=edited-two\nnew=value",
                    window,
                    cx,
                )
            });
            view.http_headers.update(cx, |input, cx| {
                input.set_value(
                    "X-Trace: trace-edited\nx-secret: edited-secret\nX-New: added",
                    window,
                    cx,
                )
            });
        });
    });

    let mut expected = original.clone();
    let ApiRequestSpec::Http(spec) = &mut expected.request else {
        panic!("fixture must be HTTP");
    };
    spec.method = "PUT".into();
    spec.query = vec![
        ApiParameter::new("token", "edited-one", true),
        ApiParameter::new("page", "3", false),
        ApiParameter::new("token", "edited-two", false),
        ApiParameter::new("new", "value", false),
    ];
    spec.headers = vec![
        ApiParameter::new("X-Trace", "trace-edited", false),
        ApiParameter::new("x-secret", "edited-secret", true),
        ApiParameter::new("X-New", "added", false),
    ];
    assert_editor_matches_record(visual_cx, &view, &expected);

    let saved = save_and_read(visual_cx, &view, &storage, &request_id);
    assert_eq!(saved, expected);
}

#[gpui_kit::test]
fn clearing_http_parameter_editors_removes_saved_parameters(cx: &mut TestAppContext) {
    let original = http_record();
    let request_id = original.id.clone();
    let storage = Arc::new(WorkspaceTestStorage::new(
        false,
        vec![workspace_with(vec![original.clone()])],
    ));
    let (view, visual_cx) = add_view(cx, storage.clone());
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );
    click(visual_cx, "api-request-item-0");

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.http_query
                .update(cx, |input, cx| input.set_value("", window, cx));
            view.http_headers
                .update(cx, |input, cx| input.set_value("", window, cx));
        });
    });
    let mut expected = original.clone();
    let ApiRequestSpec::Http(spec) = &mut expected.request else {
        panic!("fixture must be HTTP");
    };
    spec.query.clear();
    spec.headers.clear();
    assert_editor_matches_record(visual_cx, &view, &expected);
    assert_eq!(
        save_and_read(visual_cx, &view, &storage, &request_id),
        expected
    );
}

#[gpui_kit::test]
fn saved_grpc_request_round_trips_duplicate_metadata_and_redacts_sensitive_debug(
    cx: &mut TestAppContext,
) {
    let original = grpc_record(vec![
        ApiParameter::new("x-token", "gRPC 敏感值一", true),
        ApiParameter::new("x-token", "gRPC 敏感值二", false),
        ApiParameter::new("x-trace", "跟踪原值", true),
    ]);
    let request_id = original.id.clone();
    let storage = Arc::new(WorkspaceTestStorage::new(
        false,
        vec![workspace_with(vec![original.clone()])],
    ));
    let (view, visual_cx) = add_view(cx, storage.clone());
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );

    click(visual_cx, "api-request-item-0");
    assert_editor_matches_record(visual_cx, &view, &original);
    let ApiRequestSpec::Grpc(spec) = &original.request else {
        panic!("测试请求应使用 gRPC 协议");
    };
    assert!(!format!("{:?}", spec.metadata[0]).contains("gRPC 敏感值一"));
    assert!(format!("{:?}", spec.metadata[0]).contains("[REDACTED]"));

    assert_eq!(
        save_and_read(visual_cx, &view, &storage, &request_id),
        original
    );
    assert_saved_notice(visual_cx, &view);
}

#[gpui_kit::test]
fn editing_only_grpc_message_preserves_other_saved_fields(cx: &mut TestAppContext) {
    let original = grpc_record(vec![
        ApiParameter::new("x-token", "gRPC 敏感值", true),
        ApiParameter::new("x-trace", "跟踪原值", false),
    ]);
    let request_id = original.id.clone();
    let storage = Arc::new(WorkspaceTestStorage::new(
        false,
        vec![workspace_with(vec![original.clone()])],
    ));
    let (view, visual_cx) = add_view(cx, storage.clone());
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );
    click(visual_cx, "api-request-item-0");

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.grpc_message.update(cx, |input, cx| {
                input.set_value(r#"{"message":"只改消息"}"#, window, cx);
            });
        });
    });
    let mut expected = original;
    let ApiRequestSpec::Grpc(spec) = &mut expected.request else {
        panic!("测试请求应使用 gRPC 协议");
    };
    spec.message = r#"{"message":"只改消息"}"#.into();
    assert_editor_matches_record(visual_cx, &view, &expected);
    assert_eq!(
        save_and_read(visual_cx, &view, &storage, &request_id),
        expected
    );
}

#[gpui_kit::test]
fn grpc_message_and_first_metadata_edit_preserve_tail_entries(cx: &mut TestAppContext) {
    let original = grpc_record(vec![
        ApiParameter::new("x-token", "gRPC 敏感值一", true),
        ApiParameter::new("x-token", "gRPC 敏感值二", false),
        ApiParameter::new("x-trace", "跟踪原值", true),
    ]);
    let request_id = original.id.clone();
    let storage = Arc::new(WorkspaceTestStorage::new(
        false,
        vec![workspace_with(vec![original.clone()])],
    ));
    let (view, visual_cx) = add_view(cx, storage.clone());
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );
    click(visual_cx, "api-request-item-0");

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.grpc_message.update(cx, |input, cx| {
                input.set_value(r#"{"message":"edited"}"#, window, cx)
            });
            view.grpc_metadata_name
                .update(cx, |input, cx| input.set_value("x-renamed", window, cx));
            view.grpc_metadata_value
                .update(cx, |input, cx| input.set_value("replacement", window, cx));
        });
    });
    let mut expected = original.clone();
    let ApiRequestSpec::Grpc(spec) = &mut expected.request else {
        panic!("fixture must be gRPC");
    };
    spec.message = r#"{"message":"edited"}"#.into();
    spec.metadata[0] = ApiParameter::new("x-renamed", "replacement", true);
    assert_editor_matches_record(visual_cx, &view, &expected);
    assert_eq!(
        save_and_read(visual_cx, &view, &storage, &request_id),
        expected
    );
}

#[gpui_kit::test]
fn clearing_grpc_first_metadata_preserves_tail_across_repeated_saves(cx: &mut TestAppContext) {
    let original = grpc_record(vec![
        ApiParameter::new("x-token", "gRPC 敏感值一", true),
        ApiParameter::new("x-token", "gRPC 敏感值二", false),
        ApiParameter::new("x-trace", "跟踪原值", true),
    ]);
    let request_id = original.id.clone();
    let storage = Arc::new(WorkspaceTestStorage::new(
        false,
        vec![workspace_with(vec![original.clone()])],
    ));
    let (view, visual_cx) = add_view(cx, storage.clone());
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );
    click(visual_cx, "api-request-item-0");

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.grpc_metadata_name
                .update(cx, |input, cx| input.set_value("", window, cx));
            view.grpc_metadata_value
                .update(cx, |input, cx| input.set_value("", window, cx));
        });
    });
    let mut expected = original.clone();
    let ApiRequestSpec::Grpc(spec) = &mut expected.request else {
        panic!("fixture must be gRPC");
    };
    spec.metadata.remove(0);
    assert_editor_matches_record(visual_cx, &view, &expected);
    assert_eq!(
        save_and_read(visual_cx, &view, &storage, &request_id),
        expected
    );
    assert_eq!(
        save_and_read(visual_cx, &view, &storage, &request_id),
        expected,
        "重复保存不能继续删除未显示的 metadata"
    );
}

#[gpui_kit::test]
fn empty_grpc_metadata_can_add_one_default_parameter(cx: &mut TestAppContext) {
    let original = grpc_record(Vec::new());
    let request_id = original.id.clone();
    let storage = Arc::new(WorkspaceTestStorage::new(
        false,
        vec![workspace_with(vec![original.clone()])],
    ));
    let (view, visual_cx) = add_view(cx, storage.clone());
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );
    click(visual_cx, "api-request-item-0");
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.grpc_metadata_name
                .update(cx, |input, cx| input.set_value("x-added", window, cx));
            view.grpc_metadata_value
                .update(cx, |input, cx| input.set_value("added-value", window, cx));
        });
    });

    let mut expected = original.clone();
    let ApiRequestSpec::Grpc(spec) = &mut expected.request else {
        panic!("fixture must be gRPC");
    };
    spec.metadata
        .push(ApiParameter::new("x-added", "added-value", false));
    assert_editor_matches_record(visual_cx, &view, &expected);
    assert_eq!(
        save_and_read(visual_cx, &view, &storage, &request_id),
        expected
    );
}

#[gpui_kit::test]
fn switching_saved_requests_keeps_protocol_baselines_isolated(cx: &mut TestAppContext) {
    let http = http_record();
    let grpc = grpc_record(vec![ApiParameter::new("x-grpc", "grpc-value", true)]);
    let http_id = http.id.clone();
    let grpc_id = grpc.id.clone();
    let storage = Arc::new(WorkspaceTestStorage::new(
        false,
        vec![workspace_with(vec![http.clone(), grpc.clone()])],
    ));
    let (view, visual_cx) = add_view(cx, storage.clone());
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );

    click(visual_cx, "api-request-item-0");
    assert_editor_matches_record(visual_cx, &view, &http);
    click(visual_cx, "api-request-item-1");
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).protocol),
        ApiProtocol::Grpc
    );
    assert_editor_matches_record(visual_cx, &view, &grpc);
    click(visual_cx, "api-request-item-0");
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).protocol),
        ApiProtocol::Http
    );
    assert_editor_matches_record(visual_cx, &view, &http);

    assert_eq!(save_and_read(visual_cx, &view, &storage, &http_id), http);
    click(visual_cx, "api-request-item-1");
    assert_eq!(save_and_read(visual_cx, &view, &storage, &grpc_id), grpc);
}

#[gpui_kit::test]
fn switching_protocol_keeps_timeout_only_for_matching_saved_protocol(cx: &mut TestAppContext) {
    let http = http_record();
    let original_timeout = match &http.request {
        ApiRequestSpec::Http(spec) => spec.timeout_millis,
        ApiRequestSpec::Grpc(_) => panic!("测试请求应使用 HTTP 协议"),
    };
    let storage = Arc::new(WorkspaceTestStorage::new(
        false,
        vec![workspace_with(vec![http])],
    ));
    let (view, visual_cx) = add_view(cx, storage);
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );
    click(visual_cx, "api-request-item-0");

    click(visual_cx, "api-protocol-grpc");
    let grpc_timeout = visual_cx.update(|_, app| {
        let request = request_from_view(view.read(app), app).expect("应能构造 gRPC 草稿");
        let ApiRequestSpec::Grpc(spec) = request else {
            panic!("切换后应得到 gRPC 请求");
        };
        spec.timeout_millis
    });
    assert_eq!(
        grpc_timeout,
        GrpcRequestSpec::new("", "", "").timeout_millis,
        "切换到不同于已保存请求的协议时应使用默认超时"
    );

    click(visual_cx, "api-protocol-http");
    let http_timeout = visual_cx.update(|_, app| {
        let request = request_from_view(view.read(app), app).expect("应能构造 HTTP 请求");
        let ApiRequestSpec::Http(spec) = request else {
            panic!("切回后应得到 HTTP 请求");
        };
        spec.timeout_millis
    });
    assert_eq!(
        http_timeout, original_timeout,
        "切回与已保存快照相同的协议时应保留原超时"
    );
}
