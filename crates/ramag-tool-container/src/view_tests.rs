//! 容器管理空工作台的 headless 布局测试。

use gpui::{AppContext as _, Bounds, Pixels, TestAppContext, px, size};
use gpui_component::Root;

use super::ContainerView;

fn assert_inside(parent: Bounds<Pixels>, child: Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label} 越出父容器：parent={parent:?}, child={child:?}"
    );
}

#[gpui::test]
fn empty_workspace_stays_inside_supported_window_widths(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ContainerView::new(window, cx));
        Root::new(view, window, cx)
    });

    for width in [360.0, 800.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(640.0)));
        cx.run_until_parked();

        let root = cx
            .debug_bounds("container-view")
            .expect("容器管理工作台应渲染");
        let header = cx
            .debug_bounds("container-header")
            .expect("容器管理标题栏应渲染");
        let content = cx
            .debug_bounds("container-content")
            .expect("容器管理内容区应渲染");
        let empty = cx
            .debug_bounds("container-empty-state")
            .expect("容器管理空状态应渲染");

        assert_inside(root, header, "容器管理标题栏");
        assert_inside(root, content, "容器管理内容区");
        assert_inside(content, empty, "容器管理空状态");

        for selector in ["container-platform-picker", "container-connection-status"] {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} 应渲染"));
            assert_inside(header, bounds, selector);
        }

        if width < 720.0 {
            let navigation = cx
                .debug_bounds("container-compact-resource-nav")
                .expect("窄窗口应渲染紧凑资源导航");
            assert_inside(root, navigation, "窄窗口资源导航");
            assert!(
                cx.debug_bounds("container-resource-nav").is_none(),
                "窄窗口不应保留固定侧栏"
            );
        } else {
            let navigation = cx
                .debug_bounds("container-resource-nav")
                .expect("宽窗口应渲染资源导航");
            assert_inside(root, navigation, "资源导航");
            assert!(
                cx.debug_bounds("container-compact-resource-nav").is_none(),
                "宽窗口不应重复渲染紧凑导航"
            );
        }
    }
}
