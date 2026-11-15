use super::*;

#[path = "request_switch_test_support.rs"]
mod support;

use gpui_kit::TestAppContext;
use std::time::Duration;
use support::*;

#[gpui_kit::test]
fn delayed_same_protocol_response_cannot_replace_sidebar_selection(cx: &mut TestAppContext) {
    let (view, visual_cx, storage, http, _grpc) = add_view(cx);
    click(visual_cx, "api-request-item-0");
    click(visual_cx, "api-send");
    assert!(wait_for_driver_event(visual_cx, &http.slow_started).contains("slow-http"));
    let (old_generation, callback_receiver) =
        callback_watch(visual_cx, &view, ApiViewCallbackKind::Request);
    let old_cancelled = visual_cx
        .update(|_, app| view.read(app).cancelled.clone())
        .expect("慢请求应持有取消句柄");

    click(visual_cx, "api-request-item-1");
    assert!(old_cancelled.load(Ordering::Relaxed));
    click(visual_cx, "api-send");
    let current = wait_for_response(visual_cx, &view);
    assert_eq!(marker(&current), "fast-http-1");
    let current_notice = visual_cx.update(|_, app| view.read(app).notice.clone());
    http.release_slow.try_send(()).expect("释放旧 HTTP 请求");
    assert!(http.slow_finished.recv_blocking().is_ok());
    wait_for_stale_callback(
        visual_cx,
        &callback_receiver,
        ApiViewCallbackKind::Request,
        old_generation,
    );
    wait_for_history(visual_cx, &view, &storage, "Slow HTTP");

    visual_cx.update(|_, app| {
        let state = view.read(app);
        assert_eq!(
            state.active_request_id,
            state.workspace.collections[0]
                .requests
                .get(1)
                .map(|r| r.id.clone())
        );
        assert_eq!(state.response.as_ref(), Some(&current));
        assert_eq!(state.notice, current_notice);
        assert_eq!(
            state
                .runtime_environment
                .variables
                .get("marker")
                .map(String::as_str),
            Some("fast-http-1")
        );
        assert_eq!(
            state
                .extracted_variables
                .first()
                .map(|value| value.value.as_str()),
            Some("fast-http-1")
        );
        assert_eq!(state.history.len(), 1, "旧完成回调不能把历史写入界面");
        assert_eq!(state.history[0].request_name, "Fast HTTP");
        assert!(!state.loading);
        assert!(state.cancelled.is_none());
    });
}

#[gpui_kit::test]
fn delayed_response_is_ignored_when_selected_request_is_not_sent(cx: &mut TestAppContext) {
    let (view, visual_cx, storage, http, _grpc) = add_view(cx);
    click(visual_cx, "api-request-item-0");
    click(visual_cx, "api-send");
    assert!(wait_for_driver_event(visual_cx, &http.slow_started).contains("slow-http"));
    let (old_generation, callback_receiver) =
        callback_watch(visual_cx, &view, ApiViewCallbackKind::Request);
    let old_cancelled = visual_cx
        .update(|_, app| view.read(app).cancelled.clone())
        .expect("慢请求应持有取消句柄");

    click(visual_cx, "api-request-item-1");
    assert!(old_cancelled.load(Ordering::Relaxed));
    let selected_notice = visual_cx.update(|_, app| view.read(app).notice.clone());
    http.release_slow.try_send(()).expect("释放旧 HTTP 请求");
    assert!(http.slow_finished.recv_blocking().is_ok());
    wait_for_stale_callback(
        visual_cx,
        &callback_receiver,
        ApiViewCallbackKind::Request,
        old_generation,
    );
    wait_for_history(visual_cx, &view, &storage, "Slow HTTP");

    let workspace_id = visual_cx.update(|_, app| view.read(app).workspace.id.clone());
    let persisted_history =
        futures::executor::block_on(storage.service.list_history(&workspace_id, 20))
            .expect("读取 API 测试历史");
    assert!(
        persisted_history
            .iter()
            .any(|record| record.request_name == "Slow HTTP"),
        "旧请求历史仍由 ApiService 持久化"
    );
    visual_cx.update(|_, app| {
        let state = view.read(app);
        assert_eq!(
            state.active_request_id,
            state.workspace.collections[0]
                .requests
                .get(1)
                .map(|record| record.id.clone())
        );
        assert_eq!(state.request_name.read(app).value().as_str(), "Fast HTTP");
        assert!(state.response.is_none());
        assert_eq!(state.notice, selected_notice);
        assert_eq!(
            state
                .runtime_environment
                .variables
                .get("marker")
                .map(String::as_str),
            Some("baseline")
        );
        assert!(!state.loading);
        assert!(state.cancelled.is_none());
        assert!(state.history.is_empty());
    });
}

#[gpui_kit::test]
fn delayed_response_cannot_cross_protocol_sidebar_switch(cx: &mut TestAppContext) {
    let (view, visual_cx, storage, http, _grpc) = add_view(cx);
    click(visual_cx, "api-request-item-0");
    click(visual_cx, "api-send");
    assert!(wait_for_driver_event(visual_cx, &http.slow_started).contains("slow-http"));
    let (old_generation, callback_receiver) =
        callback_watch(visual_cx, &view, ApiViewCallbackKind::Request);
    click(visual_cx, "api-request-item-3");
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).protocol),
        ApiProtocol::Grpc
    );
    click(visual_cx, "api-send");
    let current = wait_for_response(visual_cx, &view);
    assert_eq!(current.protocol, ApiProtocol::Grpc);
    assert_eq!(marker(&current), "fast-grpc-1");
    let current_notice = visual_cx.update(|_, app| view.read(app).notice.clone());
    http.release_slow.try_send(()).expect("释放旧 HTTP 请求");
    assert!(http.slow_finished.recv_blocking().is_ok());
    wait_for_stale_callback(
        visual_cx,
        &callback_receiver,
        ApiViewCallbackKind::Request,
        old_generation,
    );
    wait_for_history(visual_cx, &view, &storage, "Slow HTTP");

    visual_cx.update(|_, app| {
        let state = view.read(app);
        assert_eq!(state.protocol, ApiProtocol::Grpc);
        assert_eq!(state.response.as_ref(), Some(&current));
        assert_eq!(state.notice, current_notice);
        assert_eq!(
            state
                .runtime_environment
                .variables
                .get("marker")
                .map(String::as_str),
            Some("fast-grpc-1")
        );
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.history[0].request_name, "Fast gRPC");
    });
}

#[gpui_kit::test]
fn delayed_grpc_response_cannot_replace_same_protocol_selection(cx: &mut TestAppContext) {
    let (view, visual_cx, storage, _http, grpc) = add_view(cx);
    click(visual_cx, "api-request-item-2");
    click(visual_cx, "api-send");
    assert!(wait_for_driver_event(visual_cx, &grpc.slow_started).contains("slow-grpc"));
    let (old_generation, callback_receiver) =
        callback_watch(visual_cx, &view, ApiViewCallbackKind::Request);
    click(visual_cx, "api-request-item-3");
    click(visual_cx, "api-send");
    let current = wait_for_response(visual_cx, &view);
    assert_eq!(marker(&current), "fast-grpc-1");
    let current_notice = visual_cx.update(|_, app| view.read(app).notice.clone());
    grpc.release_slow.try_send(()).expect("释放旧 gRPC 请求");
    assert!(grpc.slow_finished.recv_blocking().is_ok());
    wait_for_stale_callback(
        visual_cx,
        &callback_receiver,
        ApiViewCallbackKind::Request,
        old_generation,
    );
    wait_for_history(visual_cx, &view, &storage, "Slow gRPC");

    visual_cx.update(|_, app| {
        let state = view.read(app);
        assert_eq!(state.protocol, ApiProtocol::Grpc);
        assert_eq!(state.response.as_ref(), Some(&current));
        assert_eq!(state.notice, current_notice);
        assert_eq!(
            state
                .runtime_environment
                .variables
                .get("marker")
                .map(String::as_str),
            Some("fast-grpc-1")
        );
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.history[0].request_name, "Fast gRPC");
    });
}

#[gpui_kit::test]
fn delayed_error_cannot_replace_new_request_state(cx: &mut TestAppContext) {
    let (view, visual_cx, storage, http, _grpc) = add_view(cx);
    click(visual_cx, "api-request-item-4");
    click(visual_cx, "api-send");
    assert!(wait_for_driver_event(visual_cx, &http.slow_started).contains("slow-error"));
    let (old_generation, callback_receiver) =
        callback_watch(visual_cx, &view, ApiViewCallbackKind::Request);
    click(visual_cx, "api-request-item-1");
    click(visual_cx, "api-send");
    let current = wait_for_response(visual_cx, &view);
    let current_notice = visual_cx.update(|_, app| view.read(app).notice.clone());
    http.release_slow.try_send(()).expect("释放旧失败请求");
    assert!(http.slow_finished.recv_blocking().is_ok());
    wait_for_stale_callback(
        visual_cx,
        &callback_receiver,
        ApiViewCallbackKind::Request,
        old_generation,
    );
    wait_for_history(visual_cx, &view, &storage, "Slow error");

    visual_cx.update(|_, app| {
        let state = view.read(app);
        assert_eq!(state.response.as_ref(), Some(&current));
        assert_eq!(state.notice, current_notice);
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.history[0].request_name, "Fast HTTP");
        assert!(!state.loading);
        assert!(state.cancelled.is_none());
    });
}

#[gpui_kit::test]
fn delayed_collection_result_cannot_replace_sidebar_selection(cx: &mut TestAppContext) {
    let (view, visual_cx, storage, http, _grpc) = add_view(cx);
    click(visual_cx, "api-request-item-0");
    click(visual_cx, "api-run-collection");
    assert!(wait_for_driver_event(visual_cx, &http.slow_started).contains("slow-http"));
    let (old_generation, callback_receiver) =
        callback_watch(visual_cx, &view, ApiViewCallbackKind::Collection);
    let old_cancelled = visual_cx
        .update(|_, app| view.read(app).cancelled.clone())
        .expect("Collection 应持有取消句柄");
    click(visual_cx, "api-request-item-1");
    assert!(old_cancelled.load(Ordering::Relaxed));
    click(visual_cx, "api-send");
    let current = wait_for_response(visual_cx, &view);
    assert_eq!(marker(&current), "fast-http-1");
    let current_notice = visual_cx.update(|_, app| view.read(app).notice.clone());
    http.release_slow
        .try_send(())
        .expect("释放旧 Collection 请求");
    assert!(http.slow_finished.recv_blocking().is_ok());
    wait_for_stale_callback(
        visual_cx,
        &callback_receiver,
        ApiViewCallbackKind::Collection,
        old_generation,
    );
    wait_for_history(visual_cx, &view, &storage, "Slow HTTP");

    visual_cx.update(|_, app| {
        let state = view.read(app);
        assert_eq!(state.response.as_ref(), Some(&current));
        assert_eq!(state.notice, current_notice);
        assert!(state.last_collection_run.is_none());
        assert_eq!(
            state
                .runtime_environment
                .variables
                .get("marker")
                .map(String::as_str),
            Some("fast-http-1")
        );
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.history[0].request_name, "Fast HTTP");
    });
}

#[gpui_kit::test]
fn delayed_grpc_discovery_cannot_replace_new_request_state(cx: &mut TestAppContext) {
    let (view, visual_cx, _service, _http, grpc) = add_view(cx);
    click(visual_cx, "api-request-item-2");
    click(visual_cx, "api-grpc-discover");
    wait_for_discovery(visual_cx, &grpc.discovery_started);
    let (old_generation, callback_receiver) =
        callback_watch(visual_cx, &view, ApiViewCallbackKind::GrpcDiscovery);
    let old_cancelled = visual_cx
        .update(|_, app| view.read(app).grpc_discovery_cancelled.clone())
        .expect("发现操作应持有取消句柄");
    click(visual_cx, "api-request-item-1");
    assert!(old_cancelled.load(Ordering::Relaxed));
    assert!(!visual_cx.update(|_, app| view.read(app).grpc_discovering));
    click(visual_cx, "api-send");
    let current = wait_for_response(visual_cx, &view);
    let current_notice = visual_cx.update(|_, app| view.read(app).notice.clone());
    grpc.release_discovery
        .try_send(())
        .expect("释放旧 gRPC 发现");
    wait_for_stale_callback(
        visual_cx,
        &callback_receiver,
        ApiViewCallbackKind::GrpcDiscovery,
        old_generation,
    );

    visual_cx.update(|_, app| {
        let state = view.read(app);
        assert_eq!(state.protocol, ApiProtocol::Http);
        assert!(state.grpc_services.is_empty());
        assert!(!state.grpc_discovering);
        assert!(state.grpc_discovery_cancelled.is_none());
        assert_eq!(state.response.as_ref(), Some(&current));
        assert_eq!(state.notice, current_notice);
        assert_eq!(
            state
                .runtime_environment
                .variables
                .get("marker")
                .map(String::as_str),
            Some("fast-http-1")
        );
    });
}

#[gpui_kit::test]
fn cancel_then_retry_ignores_the_first_late_response(cx: &mut TestAppContext) {
    let (view, visual_cx, storage, http, _grpc) = add_view(cx);
    click(visual_cx, "api-request-item-0");
    click(visual_cx, "api-send");
    assert!(wait_for_driver_event(visual_cx, &http.slow_started).contains("slow-http"));
    let (old_generation, callback_receiver) =
        callback_watch(visual_cx, &view, ApiViewCallbackKind::Request);
    let old_cancelled = visual_cx
        .update(|_, app| view.read(app).cancelled.clone())
        .expect("首次请求应持有取消句柄");
    click(visual_cx, "api-cancel");
    assert!(old_cancelled.load(Ordering::Relaxed));
    click(visual_cx, "api-send");
    let current = wait_for_response(visual_cx, &view);
    assert_eq!(marker(&current), "slow-http-1");
    let current_notice = visual_cx.update(|_, app| view.read(app).notice.clone());

    http.release_slow.try_send(()).expect("释放首次慢请求");
    assert!(http.slow_finished.recv_blocking().is_ok());
    wait_for_stale_callback(
        visual_cx,
        &callback_receiver,
        ApiViewCallbackKind::Request,
        old_generation,
    );
    for _ in 0..200 {
        visual_cx.run_until_parked();
        let workspace_id = visual_cx.update(|_, app| view.read(app).workspace.id.clone());
        let history = futures::executor::block_on(storage.service.list_history(&workspace_id, 20))
            .expect("读取 API 测试历史");
        if history.len() == 2 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let workspace_id = visual_cx.update(|_, app| view.read(app).workspace.id.clone());
    assert_eq!(
        futures::executor::block_on(storage.service.list_history(&workspace_id, 20))
            .expect("读取 API 测试历史")
            .len(),
        2
    );
    visual_cx.update(|_, app| {
        let state = view.read(app);
        assert_eq!(state.response.as_ref(), Some(&current));
        assert_eq!(state.notice, current_notice);
        assert_eq!(
            state
                .runtime_environment
                .variables
                .get("marker")
                .map(String::as_str),
            Some("slow-http-1")
        );
        assert!(!state.loading);
        assert!(state.cancelled.is_none());
    });
}
