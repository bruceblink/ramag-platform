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
        let controls = visual_cx
            .debug_bounds("json-path-controls")
            .expect("JSON Path 控制栏应渲染");
        let path_input = visual_cx
            .debug_bounds("json-path-path-input")
            .expect("JSON Path 输入框应渲染");
        assert!(visual_cx.debug_bounds("json-path-run").is_some());
        let run = visual_cx
            .debug_bounds("json-path-run")
            .expect("JSON Path 执行按钮应渲染");
        assert!(visual_cx.debug_bounds("json-path-output").is_some());
        assert!(
            path_input.origin.x >= controls.origin.x
                && path_input.right() <= controls.right()
                && path_input.origin.y >= controls.origin.y
                && path_input.bottom() <= controls.bottom(),
            "JSON Path 输入框不能越出响应式控制栏: controls={controls:?}, input={path_input:?}"
        );
        assert!(
            run.origin.x >= controls.origin.x
                && run.right() <= controls.right()
                && run.origin.y >= controls.origin.y
                && run.bottom() <= controls.bottom(),
            "JSON Path 执行按钮不能越出响应式控制栏: controls={controls:?}, run={run:?}"
        );
    }
}
