//! 主壳层的 headless GPUI 几何验收。

use std::sync::Arc;

use gpui_kit::{TestAppContext, VisualTestContext, px, size};
use ramag_app::{DataSyncGate, StaticPluginHost, ToolRegistry};

use super::{Shell, WORKBENCH_TOOLBAR_HEIGHT};

fn assert_header_fits(cx: &mut VisualTestContext, width: f32, height: f32) {
    let header = cx.debug_bounds("workbench-shell-header");
    assert!(header.is_some(), "工作区顶部工具栏应渲染");
    let header = header.unwrap_or_default();
    assert_eq!(header.size.height, px(WORKBENCH_TOOLBAR_HEIGHT));
    assert!(header.origin.x >= px(0.0));
    assert!(header.origin.y >= px(0.0));
    assert!(header.right() <= px(width));
    assert!(header.bottom() <= px(height));
}

/// 在紧凑和桌面尺寸下验证共享壳层的顶部工具栏保持稳定高度并不越界。
#[gpui_kit::test]
fn workbench_shell_header_stays_inside_supported_window_sizes(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let registry = Arc::new(ToolRegistry::new());
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let gate = Arc::new(DataSyncGate::default());
    let (_, visual_cx) = cx.add_window_view(move |window, cx| {
        Shell::new(
            registry.clone(),
            plugin_host.clone(),
            gate.clone(),
            window,
            cx,
        )
    });
    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();
        assert_header_fits(visual_cx, width, height);
    }
}
