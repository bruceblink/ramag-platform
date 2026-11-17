use super::*;

use gpui_kit::{
    Modifiers, ScrollDelta, ScrollEvent, TestAppContext, TouchPhase, VisualTestContext, point, px,
    size,
};
use ramag_domain::entities::{
    ApiCollection, ApiGrpcMethodSummary, ApiGrpcServiceSummary, ApiProtocol, ApiRequestRecord,
    ApiWorkspace, GrpcRequestSpec, HttpRequestSpec,
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
    cx.simulate_mouse_down(center, gpui_kit::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(center, gpui_kit::MouseButton::Left, Modifiers::default());
}

#[gpui_kit::test]
fn api_grpc_catalog_selects_method_and_keeps_long_lists_bounded(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_kit::component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 视图应初始化");
    let services = (0..4)
        .map(|service_index| ApiGrpcServiceSummary {
            name: format!("api.Service{service_index}"),
            methods: (0..20)
                .map(|method_index| ApiGrpcMethodSummary {
                    name: format!("Method{method_index}"),
                    client_streaming: method_index % 4 == 0,
                    server_streaming: method_index % 3 == 0,
                })
                .collect(),
        })
        .collect();
    visual_cx.update(|_, app| {
        view.update(app, |view, _| {
            view.protocol = ApiProtocol::Grpc;
            view.grpc_services = services;
        });
    });
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.run_until_parked();

    let catalog = visual_cx
        .debug_bounds("api-grpc-catalog")
        .expect("gRPC Service 目录应渲染");
    let catalog_list_viewport = visual_cx
        .debug_bounds("api-grpc-catalog-list-viewport")
        .expect("gRPC Service 目录应有独立滚动视口");
    assert!(catalog_list_viewport.size.height <= px(220.0));
    assert!(catalog_list_viewport.origin.x >= catalog.origin.x);
    assert!(catalog_list_viewport.right() <= catalog.right());
    let last_method = visual_cx
        .debug_bounds("api-grpc-method-3-19")
        .expect("长目录末尾的方法应参与布局");
    assert!(last_method.bottom() > catalog_list_viewport.bottom());

    click(visual_cx, "api-grpc-method-0-3");
    visual_cx.run_until_parked();
    let selected = visual_cx.update(|_, app| {
        let view = view.read(app);
        (
            view.grpc_service.read(app).value().to_string(),
            view.grpc_method.read(app).value().to_string(),
        )
    });
    assert_eq!(selected, ("api.Service0".into(), "Method3".into()));

    // 滚动到末尾后，最后一个 Method 应能完整进入固定高度视口。
    visual_cx.simulate_event(ScrollEvent {
        position: catalog_list_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-1000.0))),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    visual_cx.run_until_parked();
    let last_method = visual_cx
        .debug_bounds("api-grpc-method-3-19")
        .expect("滚动后末尾方法仍应参与布局");
    assert!(
        last_method.origin.y >= catalog_list_viewport.origin.y
            && last_method.bottom() <= catalog_list_viewport.bottom(),
        "滚动视口应能到达目录末尾: method={last_method:?}, viewport={catalog_list_viewport:?}"
    );
}

#[gpui_kit::test]
fn api_saved_request_sidebar_opens_request_and_clears_stale_grpc_state(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_kit::component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 视图应初始化");
    let mut workspace = ApiWorkspace::new("Saved requests");
    let mut collection = ApiCollection::new("Default");
    collection.requests.push(ApiRequestRecord::new_http(
        "HTTP request",
        HttpRequestSpec::new("GET", "http://127.0.0.1:18089/json"),
    ));
    let grpc_record = ApiRequestRecord::new_grpc(
        "gRPC request",
        GrpcRequestSpec::new("http://127.0.0.1:18090", "api.Echo", "Unary"),
    );
    let grpc_request_id = grpc_record.id.clone();
    collection.requests.push(grpc_record);
    workspace.collections.push(collection);

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.workspace = workspace.clone();
            context::apply_imported_workspace(view, &workspace, window, cx);
            view.grpc_metadata_name
                .update(cx, |input, cx| input.set_value("stale", window, cx));
            view.grpc_metadata_value
                .update(cx, |input, cx| input.set_value("stale-value", window, cx));
            view.grpc_services = vec![ApiGrpcServiceSummary {
                name: "stale.Service".into(),
                methods: Vec::new(),
            }];
        });
    });
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.run_until_parked();
    click(visual_cx, "api-request-item-1");
    visual_cx.run_until_parked();

    let state = visual_cx.update(|_, app| {
        let view = view.read(app);
        (
            view.protocol,
            view.request_name.read(app).value().to_string(),
            view.grpc_service.read(app).value().to_string(),
            view.grpc_method.read(app).value().to_string(),
            view.grpc_metadata_name.read(app).value().to_string(),
            view.grpc_metadata_value.read(app).value().to_string(),
            view.grpc_services.len(),
            view.active_request_id.clone(),
            view.notice.clone(),
        )
    });
    assert_eq!(state.0, ApiProtocol::Grpc);
    assert_eq!(state.1, "gRPC request");
    assert_eq!(state.2, "api.Echo");
    assert_eq!(state.3, "Unary");
    assert!(state.4.is_empty());
    assert!(state.5.is_empty());
    assert_eq!(state.6, 0);
    assert_eq!(state.7, Some(grpc_request_id));
    assert_eq!(state.8, Some(("已打开请求：gRPC request".into(), false)));
}

#[gpui_kit::test]
fn api_request_sidebar_filters_all_saved_requests_with_collection_context(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_kit::component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 视图应初始化");
    let mut workspace = ApiWorkspace::new("Searchable");
    let mut collection = ApiCollection::new("Orders");
    collection.requests.push(ApiRequestRecord::new_http(
        "Health",
        HttpRequestSpec::new("GET", "http://127.0.0.1/health"),
    ));
    collection.requests.push(ApiRequestRecord::new_grpc(
        "Create",
        GrpcRequestSpec::new("http://127.0.0.1:18090", "api.Orders", "Create"),
    ));
    workspace.collections.push(collection);
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.workspace = workspace.clone();
            context::apply_imported_workspace(view, &workspace, window, cx);
            view.request_search
                .update(cx, |input, cx| input.set_value("orders", window, cx));
        });
    });
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("api-request-item-0").is_some());
    assert!(visual_cx.debug_bounds("api-request-item-1").is_some());
    assert!(visual_cx.debug_bounds("api-request-list-summary").is_some());
    assert!(visual_cx.debug_bounds("api-request-search-clear").is_some());
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.request_search
                .update(cx, |input, cx| input.set_value("18090", window, cx));
        });
    });
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("api-request-item-0").is_some());
    assert!(visual_cx.debug_bounds("api-request-item-1").is_none());
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.request_search
                .update(cx, |input, cx| input.set_value("get", window, cx));
        });
    });
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("api-request-item-0").is_some());
    assert!(visual_cx.debug_bounds("api-request-item-1").is_none());
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.request_search
                .update(cx, |input, cx| input.set_value("missing", window, cx));
        });
    });
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("api-request-item-0").is_none());
    assert!(visual_cx.debug_bounds("api-request-list-empty").is_some());
    click(visual_cx, "api-request-search-clear");
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("api-request-item-0").is_some());
    assert!(visual_cx.debug_bounds("api-request-item-1").is_some());
    assert!(visual_cx.debug_bounds("api-request-list-empty").is_none());
    assert!(visual_cx.debug_bounds("api-request-search-clear").is_none());
    let (search_value, search_focused) = visual_cx.update(|window, app| {
        let search = view.read(app).request_search.read(app);
        (
            search.value().to_string(),
            search.focus_handle(app).is_focused(window),
        )
    });
    assert!(search_value.is_empty());
    assert!(search_focused);
}
