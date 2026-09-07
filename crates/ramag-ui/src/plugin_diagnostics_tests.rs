#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;

use gpui::{
    AppContext, Context, InteractiveElement, ParentElement, Render, StatefulInteractiveElement,
    Styled, TestAppContext, VisualTestContext, Window, div, px, size,
};
use ramag_app::{
    PluginContext, PluginOperationError, StaticPlugin, StaticPluginAdapter, StaticPluginHost,
    ToolRegistry,
};
use ramag_domain::{PluginDescriptor, PluginId, Tool, ToolMeta};

use super::PluginDiagnosticsView;

struct TestTool {
    meta: ToolMeta,
}

impl TestTool {
    fn new(id: &str, name: &str) -> Arc<Self> {
        Arc::new(Self {
            meta: ToolMeta::new(id, name, "headless test tool"),
        })
    }
}

impl Tool for TestTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

struct FailingPlugin {
    descriptor: PluginDescriptor,
    tool: Arc<TestTool>,
}

impl StaticPlugin for FailingPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn tool(&self) -> Arc<dyn Tool> {
        self.tool.clone()
    }

    fn initialize(&self, _context: &PluginContext) -> Result<(), PluginOperationError> {
        Err(PluginOperationError::new("headless 初始化失败"))
    }
}

fn diagnostic_host() -> Arc<StaticPluginHost> {
    let registry = Arc::new(ToolRegistry::new());
    let host = Arc::new(StaticPluginHost::new(registry));
    let ready_tool = TestTool::new("ready-entry", "可用测试入口");
    host.register_plugin(Arc::new(
        StaticPluginAdapter::from_tool(ready_tool).unwrap(),
    ))
    .unwrap();

    let registration_tool = TestTool::new("actual-entry", "注册失败测试入口");
    let registration_descriptor = PluginDescriptor::new(
        PluginId::new("registration-failed").unwrap(),
        "注册失败测试插件",
        "declared-entry",
    );
    let _ = host.register_plugin(Arc::new(StaticPluginAdapter::new(
        registration_descriptor,
        registration_tool,
    )));

    let broken_tool = TestTool::new("broken-entry", "失败测试插件");
    host.register_plugin(Arc::new(FailingPlugin {
        descriptor: PluginDescriptor::new(
            PluginId::new("broken-plugin").unwrap(),
            "失败测试插件",
            "broken-entry",
        ),
        tool: broken_tool,
    }))
    .unwrap();
    host.initialize_all();
    host
}

fn assert_inside(child: gpui::Bounds<gpui::Pixels>, parent: gpui::Bounds<gpui::Pixels>) {
    assert!(child.origin.x >= parent.origin.x);
    assert!(child.origin.y >= parent.origin.y);
    assert!(child.right() <= parent.right());
    assert!(child.bottom() <= parent.bottom());
}

struct PluginDiagnosticsTestHost {
    view: gpui::Entity<PluginDiagnosticsView>,
}

impl Render for PluginDiagnosticsTestHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        div()
            .id("plugin-diagnostics-scroll")
            .debug_selector(|| "plugin-diagnostics-scroll".into())
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .child(self.view.clone())
    }
}

#[gpui::test]
fn plugin_diagnostics_stays_inside_supported_headless_widths(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let host = diagnostic_host();
    let host_for_view = host.clone();
    let (_, visual_cx) = cx.add_window_view(move |_, cx| PluginDiagnosticsTestHost {
        view: cx.new(|_| PluginDiagnosticsView::new(host_for_view.clone())),
    });
    let visual_cx: &mut VisualTestContext = visual_cx;

    for (width, height) in [(360.0, 520.0), (1024.0, 520.0), (1440.0, 520.0)] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();

        let root = visual_cx
            .debug_bounds("plugin-diagnostics")
            .expect("插件诊断页应渲染");
        let scroll = visual_cx
            .debug_bounds("plugin-diagnostics-scroll")
            .expect("插件诊断滚动区应渲染");
        let summary = visual_cx
            .debug_bounds("plugin-summary")
            .expect("插件概览应渲染");
        let available = visual_cx
            .debug_bounds("plugin-available")
            .expect("可用入口区域应渲染");
        let states = visual_cx
            .debug_bounds("plugin-state-list")
            .expect("插件状态区域应渲染");
        let ready = visual_cx
            .debug_bounds("plugin-state-ready-entry")
            .expect("就绪插件状态应渲染");
        let broken = visual_cx
            .debug_bounds("plugin-state-broken-plugin")
            .expect("失败插件状态应渲染");
        let registration_failed = visual_cx
            .debug_bounds("plugin-state-registration-failed")
            .expect("注册失败插件状态应渲染");
        let entry = visual_cx
            .debug_bounds("plugin-available-ready-entry")
            .expect("可用入口明细应渲染");

        assert_eq!(root.size.width, px(width));
        assert!(scroll.origin.x >= px(0.0));
        assert!(scroll.origin.y >= px(0.0));
        assert!(scroll.right() <= px(width));
        assert!(scroll.bottom() <= px(height));
        assert!(root.origin.x >= scroll.origin.x);
        assert!(root.right() <= scroll.right());
        assert!(root.origin.y >= scroll.origin.y);
        for child in [
            summary,
            available,
            states,
            ready,
            broken,
            registration_failed,
            entry,
        ] {
            assert_inside(child, root);
        }
        assert!(broken.bottom() <= states.bottom());
        assert!(entry.bottom() <= available.bottom());
    }
}
