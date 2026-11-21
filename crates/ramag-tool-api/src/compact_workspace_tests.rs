use super::*;
use gpui_kit::component::{Root, WindowExt as _};
use gpui_kit::{Modifiers, TestAppContext, VisualTestContext, size};
use ramag_domain::entities::ApiRequestRecord;

fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let Some(bounds) = visual.debug_bounds(selector) else {
        unreachable!("应显示 {selector}");
    };
    visual.simulate_click(bounds.center(), Modifiers::default());
    visual.run_until_parked();
}

#[gpui_kit::test]
fn compact_api_switches_panes_and_reuses_live_collection(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let api = cx.new(|cx| ApiView::new(window, cx));
        view = Some(api.clone());
        Root::new(api, window, cx)
    });
    let Some(view) = view else {
        unreachable!("API 视图应初始化");
    };
    let mut collection = ApiCollection::new("测试集合");
    collection.requests.push(ApiRequestRecord::new_http(
        "HTTP 健康检查",
        HttpRequestSpec::new("GET", "http://127.0.0.1/health"),
    ));
    let grpc = ApiRequestRecord::new_grpc(
        "gRPC 回显",
        GrpcRequestSpec::new("http://127.0.0.1:18090", "api.Echo", "Unary"),
    );
    collection.requests.push(grpc.clone());
    view.update(visual, |view, cx| {
        view.workspace.collections.push(collection);
        cx.notify();
    });
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    assert!(visual.debug_bounds("api-sidebar").is_none());
    assert!(visual.debug_bounds("api-request-pane").is_some());
    assert!(visual.debug_bounds("api-response").is_none());

    click(visual, "api-compact-response");
    assert!(visual.debug_bounds("api-request-pane").is_none());
    assert!(visual.debug_bounds("api-response-empty").is_some());

    click(visual, "api-open-collection");
    assert!(visual.update(|window, cx| window.has_active_dialog(cx)));
    assert!(visual.debug_bounds("api-history").is_some());
    assert!(visual.debug_bounds("api-request-item-1").is_some());
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.request_search.update(cx, |input, cx| {
                input.set_value("18090", window, cx);
            });
        });
    });
    visual.run_until_parked();
    assert!(visual.debug_bounds("api-request-item-0").is_some());
    assert!(visual.debug_bounds("api-request-item-1").is_none());
    click(visual, "api-request-item-0");
    assert!(!visual.update(|window, cx| window.has_active_dialog(cx)));
    assert!(visual.debug_bounds("api-request-pane").is_some());
    assert!(visual.debug_bounds("api-response").is_none());
    assert!(visual.update(|window, cx| {
        let view = view.read(cx);
        view.active_request_id.as_ref() == Some(&grpc.id)
            && view.protocol == ApiProtocol::Grpc
            && view.collection_button_focus.is_focused(window)
    }));

    click(visual, "api-send");
    assert!(visual.debug_bounds("api-request-pane").is_none());
    assert!(visual.debug_bounds("api-response-error").is_some());
    click(visual, "api-compact-request");
    assert!(visual.update(|_, cx| view.read(cx).grpc_method.read(cx).value() == "Unary"));

    click(visual, "api-open-collection");
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();
    assert!(visual.debug_bounds("api-sidebar-divider").is_none());
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    assert!(!visual.update(|window, cx| window.has_active_dialog(cx)));
    assert!(visual.debug_bounds("api-sidebar-divider").is_some());
    assert!(visual.debug_bounds("api-request-pane").is_some());
    assert!(visual.debug_bounds("api-response").is_some());
    assert!(visual.debug_bounds("api-compact-request").is_none());
}
