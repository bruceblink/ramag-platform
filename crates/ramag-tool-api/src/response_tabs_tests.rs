use super::*;

use gpui_kit::{Modifiers, TestAppContext, VisualTestContext, point, px, size};
use ramag_domain::entities::{
    ApiAssertionResult, ApiParameter, ApiResponseSnapshot, ApiResponseSnapshotParts,
    ApiResponseStatus,
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

fn response_snapshot() -> ApiResponseSnapshot {
    let body = br#"{"ok":true}"#.to_vec();
    ApiResponseSnapshot::new(ApiResponseSnapshotParts {
        protocol: ApiProtocol::Http,
        status: ApiResponseStatus::Http { code: 200 },
        headers: vec![ApiParameter::new("Content-Type", "application/json", false)],
        metadata: vec![ApiParameter::new("x-request-id", "test-1", false)],
        size_bytes: body.len() as u64,
        body,
        elapsed_millis: 12,
        truncated: false,
        error: None,
    })
    .expect("测试响应应有效")
}

#[gpui_kit::test]
fn api_response_tabs_switch_between_existing_response_sections(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_kit::component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 视图应初始化");
    let snapshot = response_snapshot();
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.response = Some(snapshot.clone());
            view.assertion_results = vec![ApiAssertionResult {
                passed: true,
                message: "HTTP 状态为 200".into(),
            }];
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

    assert!(visual_cx.debug_bounds("api-response-tabs").is_some());
    assert!(visual_cx.debug_bounds("api-response-body").is_some());
    assert!(visual_cx.debug_bounds("api-response-parameters").is_none());
    assert!(visual_cx.debug_bounds("api-response-timing").is_none());
    assert!(visual_cx.debug_bounds("api-response-assertions").is_none());

    click(visual_cx, "api-response-tab-headers");
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("api-response-body").is_none());
    assert!(visual_cx.debug_bounds("api-response-parameters").is_some());
    assert!(visual_cx.debug_bounds("api-response-headers").is_some());
    assert!(visual_cx.debug_bounds("api-response-metadata").is_some());

    click(visual_cx, "api-response-tab-timing");
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("api-response-parameters").is_none());
    assert!(visual_cx.debug_bounds("api-response-timing").is_some());

    click(visual_cx, "api-response-tab-assertions");
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("api-response-timing").is_none());
    assert!(visual_cx.debug_bounds("api-response-assertions").is_some());
    assert!(visual_cx.debug_bounds("api-assertion-results").is_some());

    let selected = visual_cx.update(|_, app| view.read(app).response_tab);
    assert_eq!(selected, ApiResponseTab::Assertions);
}

#[gpui_kit::test]
fn api_response_tab_resets_to_body_when_protocol_clears_response(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_kit::component::Root::new(view, window, cx)
    });
    let view = view_entity.expect("API 视图应初始化");
    let snapshot = response_snapshot();
    visual_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.response = Some(snapshot.clone());
            view.response_tab = ApiResponseTab::Headers;
            cx.notify();
        });
    });
    visual_cx.run_until_parked();
    click(visual_cx, "api-response-tab-headers");
    visual_cx.run_until_parked();

    visual_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.set_protocol(ApiProtocol::Grpc, cx);
            view.response = Some(snapshot);
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

    assert_eq!(
        visual_cx.update(|_, app| view.read(app).response_tab),
        ApiResponseTab::Body
    );
    assert!(visual_cx.debug_bounds("api-response-body").is_some());
    assert!(visual_cx.debug_bounds("api-response-parameters").is_none());
}
