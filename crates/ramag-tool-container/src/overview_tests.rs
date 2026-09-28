use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, Bounds, Pixels, TestAppContext, px, size};
use ramag_domain::entities::{
    ContainerEndpointId, DockerConnectionInfo, DockerEngineVersion, DockerOverview,
    DockerResourceCounts,
};

use super::{ContainerView, format_capacity_bytes};

fn assert_inside(parent: Bounds<Pixels>, child: Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label}越出父容器：parent={parent:?}, child={child:?}"
    );
}

fn test_overview() -> DockerOverview {
    DockerOverview {
        connection: DockerConnectionInfo {
            endpoint_id: ContainerEndpointId::default(),
            api_version: Some("1.55".into()),
            server_version: Some("29.7.2".into()),
            server_name: Some("test-engine".into()),
            operating_system: Some("linux".into()),
            architecture: Some("amd64".into()),
            read_only: true,
        },
        version: DockerEngineVersion {
            api_version: Some("1.55".into()),
            min_api_version: Some("1.40".into()),
            server_version: Some("29.7.2".into()),
            build_time: None,
            git_commit: None,
            go_version: None,
            os: Some("linux".into()),
            architecture: Some("amd64".into()),
            kernel_version: None,
            experimental: Some(false),
        },
        counts: DockerResourceCounts {
            containers: 3,
            running_containers: 2,
            paused_containers: 0,
            stopped_containers: 1,
            images: 4,
            networks: Some(2),
            volumes: Some(1),
        },
        cpu_count: Some(8),
        memory_bytes: Some(8 * 1024 * 1024 * 1024),
    }
}

#[test]
fn engine_capacity_values_are_optional_and_human_readable() {
    assert_eq!(format_capacity_bytes(None), "未知");
    assert_eq!(format_capacity_bytes(Some(512)), "512 B");
    assert_eq!(format_capacity_bytes(Some(1024)), "1.0 KiB");
    assert_eq!(format_capacity_bytes(Some(1024 * 1024)), "1.0 MiB");
    assert_eq!(format_capacity_bytes(Some(1024 * 1024 * 1024)), "1.0 GiB");
}

#[gpui_kit::test]
fn engine_capacity_cards_stay_inside_supported_window_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ContainerView::new(window, cx));
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = view_entity.expect("容器管理视图应初始化");
    view.update(visual_cx, |view, cx| {
        view.section = super::ContainerSection::Overview;
        view.overview = Some(test_overview());
        cx.notify();
    });

    for width in [360.0, 1024.0, 1440.0] {
        visual_cx.simulate_resize(size(px(width), px(640.0)));
        visual_cx.run_until_parked();
        let content = visual_cx
            .debug_bounds("container-content")
            .expect("容器内容区应渲染");
        let panel = visual_cx
            .debug_bounds("container-overview-panel")
            .expect("容器概览面板应渲染");
        assert_inside(content, panel, "容器概览面板");
        for (key, selector) in [
            ("containers", "container-overview-card-containers"),
            ("running", "container-overview-card-running"),
            ("images", "container-overview-card-images"),
            ("networks", "container-overview-card-networks"),
            ("volumes", "container-overview-card-volumes"),
            ("cpu", "container-overview-card-cpu"),
            ("memory", "container-overview-card-memory"),
        ] {
            let card = visual_cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("概览卡片 {key} 应渲染"));
            assert_inside(panel, card, key);
        }
    }
}
