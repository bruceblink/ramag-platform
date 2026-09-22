use super::*;

use gpui::{Modifiers, TestAppContext, VisualTestContext, point, px, size};
use ramag_domain::entities::{ApiGrpcMethodSummary, ApiGrpcServiceSummary, ApiProtocol};

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
fn api_grpc_catalog_selects_method_and_keeps_long_lists_bounded(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::new(window, cx));
        view_entity = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
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
    let catalog_list = visual_cx
        .debug_bounds("api-grpc-catalog-list")
        .expect("gRPC Service 目录应有独立列表区域");
    assert!(catalog_list.size.height <= px(220.0));
    assert!(catalog_list.origin.x >= catalog.origin.x);
    assert!(catalog_list.right() <= catalog.right());

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
}
