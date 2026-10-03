#![allow(clippy::expect_used, clippy::unwrap_used)]

use gpui_kit::{
    AppContext, Context, IntoElement, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, Window, div, px, size,
};
use ramag_domain::{PluginEntryDataSpec, PluginEntryDescriptor, PluginId};

use super::StandardPluginEntryView;

struct TestHost {
    view: gpui_kit::Entity<StandardPluginEntryView>,
}

impl Render for TestHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.view.clone())
    }
}

#[gpui_kit::test]
fn standard_entry_contract_stays_inside_supported_windows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let entry = PluginEntryDescriptor::new("json.extract", "JSON 提取")
        .with_description("从 JSON 文本提取指定路径")
        .with_input(PluginEntryDataSpec::json(1024 * 1024))
        .with_output(PluginEntryDataSpec::text(64 * 1024));
    let (_, visual_cx) = cx.add_window_view(move |_, cx| TestHost {
        view: cx.new(|_| StandardPluginEntryView::new("it-tools", entry.clone())),
    });
    let visual_cx: &mut VisualTestContext = visual_cx;

    for (width, height) in [(360.0, 520.0), (1024.0, 640.0), (1440.0, 900.0)] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();

        let root = visual_cx
            .debug_bounds("plugin-entry-view-json.extract")
            .expect("标准入口应渲染");
        let header = visual_cx
            .debug_bounds("plugin-entry-header")
            .expect("入口标题应渲染");
        let page_title = visual_cx
            .debug_bounds("pulse-page-title")
            .expect("入口应使用公共 Pulse 页面标题");
        let contract = visual_cx
            .debug_bounds("plugin-entry-contract")
            .expect("输入输出契约应渲染");
        let input = visual_cx
            .debug_bounds("plugin-entry-contract-输入")
            .expect("输入卡片应渲染");
        let output = visual_cx
            .debug_bounds("plugin-entry-contract-输出")
            .expect("输出卡片应渲染");
        let unbound = visual_cx
            .debug_bounds("plugin-entry-unbound")
            .expect("未绑定说明应渲染");
        let notice = visual_cx
            .debug_bounds("pulse-status-notice")
            .expect("未绑定入口应使用公共状态提示");

        assert_eq!(root.size.width, px(width));
        for child in [header, page_title, contract, input, output, unbound, notice] {
            assert!(child.origin.x >= root.origin.x);
            assert!(child.right() <= root.right());
            assert!(child.origin.y >= root.origin.y);
            assert!(child.bottom() <= root.bottom());
        }
        if width < 720.0 {
            assert!(
                input.bottom() <= output.origin.y,
                "窄窗口下输入和输出契约应纵向排列: input={input:?}, output={output:?}"
            );
        } else {
            assert!(
                input.right() <= output.origin.x,
                "宽窗口下输入和输出契约应横向排列: input={input:?}, output={output:?}"
            );
        }
    }
}

#[test]
fn standard_entry_view_keeps_descriptor_identity() {
    let plugin_id = PluginId::new("example").expect("测试 ID 应有效");
    let entry = PluginEntryDescriptor::new("example.entry", "Example");
    assert_eq!(plugin_id.as_str(), "example");
    assert_eq!(entry.id, "example.entry");
}
