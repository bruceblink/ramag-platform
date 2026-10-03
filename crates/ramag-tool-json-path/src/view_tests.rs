use super::*;
use gpui_kit::{TestAppContext, px, size};
use ramag_app::ToolRegistry;

#[test]
fn request_keeps_raw_json_and_path_separate() {
    let request = JsonPathRequest {
        raw_json: "{value: 1}".into(),
        path: "$.value".into(),
    };
    let encoded = serde_json::to_vec(&request).expect("request serializes");
    let decoded: JsonPathRequest = serde_json::from_slice(&encoded).expect("request decodes");
    assert_eq!(decoded, request);
}

#[gpui_kit::test]
fn json_path_view_keeps_native_controls_visible_at_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let host = Arc::new(StaticPluginHost::new(Arc::new(ToolRegistry::new())));
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| JsonPathView::new(host, window, cx));
        gpui_kit::component::Root::new(view, window, cx)
    });
    for width in [360.0, 1024.0, 1440.0] {
        visual_cx.simulate_resize(size(px(width), px(900.0)));
        visual_cx.run_until_parked();
        assert!(visual_cx.debug_bounds("json-path-view").is_some());
        assert!(
            visual_cx.debug_bounds("json-path-page-header").is_some(),
            "JSON Path 页头应在支持的窗口宽度下保持可见"
        );
        assert!(visual_cx.debug_bounds("json-path-status").is_some());
        assert!(visual_cx.debug_bounds("json-path-run").is_some());
        assert!(visual_cx.debug_bounds("json-path-output").is_some());
    }
}
