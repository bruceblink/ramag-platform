//! 容器管理空工作台的 headless 布局测试。

use gpui::Modifiers;
use gpui::{AppContext as _, Bounds, MouseButton, Pixels, TestAppContext, px, size};
use gpui_component::Root;

use super::{ContainerSection, ContainerView};
use ramag_domain::entities::{ContainerPage, DockerImageSummary};

fn assert_inside(parent: Bounds<Pixels>, child: Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label} 越出父容器：parent={parent:?}, child={child:?}"
    );
}

fn assert_horizontal_inside(parent: Bounds<Pixels>, child: Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x && child.right() <= parent.right(),
        "{label} 横向越出父容器：parent={parent:?}, child={child:?}"
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
        let subtitle = cx
            .debug_bounds("container-subtitle")
            .expect("容器管理副标题应渲染");
        let content = cx
            .debug_bounds("container-content")
            .expect("容器管理内容区应渲染");
        let empty = cx
            .debug_bounds("container-empty-state")
            .expect("容器管理空状态应渲染");

        assert_inside(root, header, "容器管理标题栏");
        assert_inside(header, subtitle, "容器管理副标题");
        assert!(
            subtitle.size.height < px(24.0),
            "容器管理副标题应保持单行: subtitle={subtitle:?}"
        );
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

#[gpui::test]
fn registry_workspace_stays_inside_narrow_window(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ContainerView::new(window, cx));
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(360.0), px(640.0)));
    cx.run_until_parked();

    let navigation = cx
        .debug_bounds("container-resource-registry")
        .expect("镜像仓库导航入口应渲染");
    let point = navigation.center();
    cx.simulate_mouse_down(point, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(point, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();

    let panel = cx
        .debug_bounds("container-registry-panel")
        .expect("镜像仓库面板应渲染");
    let content = cx
        .debug_bounds("container-content")
        .expect("容器管理内容区应渲染");
    let config = cx
        .debug_bounds("container-registry-config")
        .expect("镜像仓库配置区应渲染");
    assert_horizontal_inside(content, panel, "镜像仓库面板");
    assert_inside(panel, config, "镜像仓库配置区");
}

#[gpui::test]
fn compact_resource_navigation_wraps_without_full_width_rows(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ContainerView::new(window, cx));
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(360.0), px(640.0)));
    cx.run_until_parked();

    let navigation = cx
        .debug_bounds("container-compact-resource-nav")
        .expect("紧凑资源导航应渲染");
    assert!(
        navigation.size.height < px(150.0),
        "紧凑资源导航不应把内容推离首屏: navigation={navigation:?}"
    );
    for selector in [
        "container-resource-overview",
        "container-resource-containers",
        "container-resource-images",
        "container-resource-networks",
        "container-resource-volumes",
        "container-resource-registry",
    ] {
        let button = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} 应渲染"));
        assert_horizontal_inside(navigation, button, selector);
        assert!(
            button.size.width < navigation.size.width,
            "紧凑资源入口不应强制占满导航: navigation={navigation:?}, button={button:?}"
        );
    }
}

#[gpui::test]
fn image_rows_keep_long_names_and_subtitles_inside_narrow_window(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ContainerView::new(window, cx));
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = view_entity.expect("容器管理视图应初始化");
    view.update(visual_cx, |view, cx| {
        view.section = ContainerSection::Images;
        view.loading = false;
        view.images = Some(ContainerPage {
            items: vec![DockerImageSummary {
                id: "image-123".into(),
                repository_tags: vec![
                    "registry.example.com/team/very-long-image-name-that-must-not-overlap:latest"
                        .into(),
                ],
                repository_digests: Vec::new(),
                created: None,
                size_bytes: Some(1024 * 1024 * 512),
                shared_size_bytes: None,
                containers: None,
                architecture: None,
                operating_system: Some(
                    "linux/amd64-with-a-long-platform-description-for-narrow-windows".into(),
                ),
                labels: Vec::new(),
            }],
            page: 1,
            page_size: 100,
            total: 1,
            has_more: false,
        });
        cx.notify();
    });
    visual_cx.simulate_resize(size(px(360.0), px(640.0)));
    visual_cx.run_until_parked();

    let panel = visual_cx
        .debug_bounds("container-view")
        .expect("容器管理工作台应渲染");
    let row = visual_cx
        .debug_bounds("container-resource-image-image-123")
        .expect("镜像行应渲染");
    let title = visual_cx
        .debug_bounds("container-resource-image-image-123-title")
        .expect("镜像标题应渲染");
    let subtitle = visual_cx
        .debug_bounds("container-resource-image-image-123-subtitle")
        .expect("镜像副标题应渲染");
    assert_inside(panel, row, "镜像行");
    assert_horizontal_inside(panel, title, "镜像标题");
    assert_horizontal_inside(panel, subtitle, "镜像副标题");
    assert!(
        subtitle.origin.y >= title.bottom(),
        "镜像标题和副标题不能重叠: title={title:?}, subtitle={subtitle:?}"
    );
}
