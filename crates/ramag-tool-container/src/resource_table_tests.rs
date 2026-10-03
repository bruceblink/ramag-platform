use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, TestAppContext, px, size};
use ramag_domain::entities::{
    ContainerPage, DockerContainerPort, DockerContainerSummary, DockerImageSummary,
    DockerNetworkSummary, DockerVolumeSummary,
};

use super::{ContainerSection, ContainerView};

fn single_item_page<T>(item: T) -> ContainerPage<T> {
    ContainerPage {
        items: vec![item],
        page: 1,
        page_size: 100,
        total: 1,
        has_more: false,
    }
}

#[gpui_kit::test]
fn image_rows_render_in_a_scrollable_table_on_narrow_window(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
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
        view.images = Some(single_item_page(DockerImageSummary {
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
        }));
        cx.notify();
    });
    visual_cx.simulate_resize(size(px(360.0), px(640.0)));
    visual_cx.run_until_parked();

    let panel = visual_cx
        .debug_bounds("container-view")
        .expect("容器管理工作台应渲染");
    let content = visual_cx
        .debug_bounds("container-content")
        .expect("资源列表内容区应渲染");
    let table_panel = visual_cx
        .debug_bounds("container-resource-panel")
        .expect("镜像表格面板应渲染");
    let table_viewport = visual_cx
        .debug_bounds("container-resource-table-vertical-scroll")
        .expect("窄窗口应保留横向滚动表格视口");
    let row = visual_cx
        .debug_bounds("container-resource-image-image-123")
        .expect("镜像行应渲染");
    let header = visual_cx
        .debug_bounds("container-resource-image-header-reference")
        .expect("镜像表格应显示标签列表头");
    let first_cell = visual_cx
        .debug_bounds("container-resource-image-image-123-cell-reference")
        .expect("镜像名称应位于独立表格列");
    let second_cell = visual_cx
        .debug_bounds("container-resource-image-image-123-cell-id")
        .expect("镜像 ID 应位于独立表格列");
    let horizontal_overflow =
        visual_cx.update(|_, cx| view.read(cx).resource_table_scroll.max_offset().x);

    assert!(
        panel.origin.x <= table_panel.origin.x
            && panel.right() >= table_panel.right()
            && panel.origin.y <= table_panel.origin.y
            && panel.bottom() >= table_panel.bottom(),
        "镜像表格面板应位于工作台内: panel={panel:?}, table_panel={table_panel:?}"
    );
    assert!(
        table_panel.origin.x <= table_viewport.origin.x
            && table_panel.right() >= table_viewport.right()
            && table_panel.origin.y <= table_viewport.origin.y
            && table_panel.bottom() >= table_viewport.bottom(),
        "镜像表格滚动视口应位于面板内: panel={table_panel:?}, viewport={table_viewport:?}"
    );
    assert!(
        table_panel.bottom() >= content.bottom() - px(20.0),
        "资源表格面板应延伸到内容区底部并填满可用高度: content={content:?}, panel={table_panel:?}"
    );
    assert!(
        horizontal_overflow > px(0.0),
        "窄窗口下表格列应通过横向滚动访问: max_offset={horizontal_overflow:?}"
    );
    assert!(
        row.origin.y >= table_viewport.origin.y && row.bottom() <= table_viewport.bottom(),
        "镜像表格行应处于可视滚动区域: row={row:?}, viewport={table_viewport:?}"
    );
    assert!(
        first_cell.right() <= second_cell.origin.x,
        "镜像名称和 ID 应分列排列: first={first_cell:?}, second={second_cell:?}"
    );
    assert!(
        header.size.height > px(0.0),
        "镜像表格列标题应可见: header={header:?}"
    );
}

#[gpui_kit::test]
fn resource_list_fills_remaining_height_and_scrolls_long_tables(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ContainerView::new(window, cx));
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = view_entity.expect("容器管理视图应初始化");
    view.update(visual_cx, |view, cx| {
        view.section = ContainerSection::Containers;
        view.loading = false;
        view.containers = Some(ContainerPage {
            items: (0..40)
                .map(|index| DockerContainerSummary {
                    id: format!("container-{index}"),
                    names: vec![format!("/service-{index}")],
                    image: Some("example/service:latest".into()),
                    image_id: None,
                    command: None,
                    created: None,
                    state: Some("running".into()),
                    status: Some("Up 2 hours (healthy)".into()),
                    health: Some("healthy".into()),
                    ports: Vec::new(),
                    networks: vec!["bridge".into()],
                    labels: Vec::new(),
                })
                .collect(),
            page: 1,
            page_size: 100,
            total: 40,
            has_more: false,
        });
        cx.notify();
    });
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.run_until_parked();

    let content = visual_cx
        .debug_bounds("container-content")
        .expect("资源列表内容区应渲染");
    let panel = visual_cx
        .debug_bounds("container-resource-panel")
        .expect("容器资源表格面板应渲染");
    let viewport = visual_cx
        .debug_bounds("container-resource-table-vertical-scroll")
        .expect("表格应保留独立的纵向滚动区");
    let table = visual_cx
        .debug_bounds("container-resource-table-container")
        .expect("容器数据行表格应渲染");
    let last_row = visual_cx
        .debug_bounds("container-resource-container-container-39")
        .expect("最后一条容器资源行应渲染");
    let vertical_overflow =
        visual_cx.update(|_, cx| view.read(cx).resource_table_scroll.max_offset().y);

    assert!(
        panel.bottom() >= content.bottom() - px(20.0),
        "表格面板应延伸到内容区底部: content={content:?}, panel={panel:?}"
    );
    assert!(
        viewport.size.height > px(300.0),
        "表格滚动区应占据页面剩余高度: viewport={viewport:?}"
    );
    assert!(
        vertical_overflow > px(0.0),
        "长表格应在填满窗口后继续纵向滚动: max_offset={vertical_overflow:?}, viewport={viewport:?}, table={table:?}, last_row={last_row:?}"
    );
}

#[gpui_kit::test]
fn resource_sections_render_consistent_pulse_tables(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ContainerView::new(window, cx));
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = view_entity.expect("容器管理视图应初始化");
    view.update(visual_cx, |view, cx| {
        view.loading = false;
        view.containers = Some(single_item_page(DockerContainerSummary {
            id: "container-123".into(),
            names: vec!["/api".into()],
            image: Some("example/api:latest".into()),
            image_id: None,
            command: None,
            created: None,
            state: Some("running".into()),
            status: Some("Up 2 hours (healthy)".into()),
            health: Some("healthy".into()),
            ports: vec![DockerContainerPort {
                ip: Some("0.0.0.0".into()),
                private_port: Some(8080),
                public_port: Some(8080),
                protocol: Some("tcp".into()),
            }],
            networks: vec!["bridge".into()],
            labels: Vec::new(),
        }));
        view.images = Some(single_item_page(DockerImageSummary {
            id: "img1234567890".into(),
            repository_tags: vec!["example/api:latest".into()],
            repository_digests: Vec::new(),
            created: None,
            size_bytes: Some(1024),
            shared_size_bytes: None,
            containers: Some(1),
            architecture: Some("amd64".into()),
            operating_system: Some("linux".into()),
            labels: Vec::new(),
        }));
        view.networks = Some(single_item_page(DockerNetworkSummary {
            id: "network-123".into(),
            name: Some("bridge".into()),
            driver: Some("bridge".into()),
            scope: Some("local".into()),
            internal: Some(false),
            attachable: Some(true),
            ingress: Some(false),
            labels: Vec::new(),
            container_count: 1,
            subnets: Vec::new(),
        }));
        view.volumes = Some(single_item_page(DockerVolumeSummary {
            name: "app-data".into(),
            driver: Some("local".into()),
            mountpoint: Some("/var/lib/docker/volumes/app-data/_data".into()),
            scope: Some("local".into()),
            labels: Vec::new(),
            container_count: 1,
            usage_size_bytes: Some(4096),
            usage_reference_count: Some(1),
        }));
        view.section = ContainerSection::Containers;
        cx.notify();
    });

    for (section, table_selector, header_selector, row_selector) in [
        (
            ContainerSection::Containers,
            "container-resource-table-container",
            "container-resource-container-header-name",
            "container-resource-container-container-123",
        ),
        (
            ContainerSection::Images,
            "container-resource-table-image",
            "container-resource-image-header-reference",
            "container-resource-image-img1234567890",
        ),
        (
            ContainerSection::Networks,
            "container-resource-table-network",
            "container-resource-network-header-driver",
            "container-resource-network-network-123",
        ),
        (
            ContainerSection::Volumes,
            "container-resource-table-volume",
            "container-resource-volume-header-mountpoint",
            "container-resource-volume-app-data",
        ),
    ] {
        view.update(visual_cx, |view, cx| {
            view.section = section;
            cx.notify();
        });
        visual_cx.run_until_parked();

        visual_cx
            .debug_bounds(table_selector)
            .unwrap_or_else(|| panic!("{table_selector} 资源表格应渲染"));
        visual_cx
            .debug_bounds(header_selector)
            .unwrap_or_else(|| panic!("{header_selector} 表格列标题应渲染"));
        visual_cx
            .debug_bounds(row_selector)
            .unwrap_or_else(|| panic!("{row_selector} 表格资源行应渲染"));
    }
}
