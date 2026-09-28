use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, Bounds, Pixels, TestAppContext, px, size};

use super::{ContainerSection, ContainerView, SelectedDetail, container_detail_text};
use ramag_domain::entities::{DockerContainerDetail, DockerContainerSummary};

fn assert_inside(parent: Bounds<Pixels>, child: Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label}越出父容器：parent={parent:?}, child={child:?}"
    );
}

fn test_container_detail() -> DockerContainerDetail {
    DockerContainerDetail {
        summary: DockerContainerSummary {
            id: "container-detail".into(),
            names: vec!["/detail".into()],
            image: Some("alpine:3.20".into()),
            image_id: None,
            command: Some("sleep 60".into()),
            created: Some(1_700_000_000),
            state: Some("running".into()),
            status: Some("Up 2 minutes".into()),
            health: Some("healthy".into()),
            ports: Vec::new(),
            networks: Vec::new(),
            labels: Vec::new(),
        },
        path: Some("/bin/sh".into()),
        args: vec!["-c".into()],
        platform: Some("linux".into()),
        working_directory: None,
        entrypoint: Vec::new(),
        command: vec!["sleep".into(), "60".into()],
        restart_policy: None,
        env_keys: vec!["APP_ENV".into()],
        mounts: Vec::new(),
        networks: Vec::new(),
    }
}

#[test]
fn container_detail_text_exposes_state_and_health_without_raw_unknowns() {
    let detail = test_container_detail();
    let text = container_detail_text(&detail);
    assert!(text.contains("状态：运行中"));
    assert!(text.contains("健康检查：健康"));
    assert!(text.contains("创建时间：2023-11-14T22:13:20+00:00"));

    let mut without_health = detail;
    without_health.summary.health = None;
    assert!(container_detail_text(&without_health).contains("健康检查：未配置健康检查"));
}

#[gpui_kit::test]
fn container_detail_state_stays_inside_supported_window_widths(cx: &mut TestAppContext) {
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
        view.selected_detail = Some(SelectedDetail::Container(test_container_detail()));
        cx.notify();
    });

    for width in [360.0, 1024.0, 1440.0] {
        visual_cx.simulate_resize(size(px(width), px(640.0)));
        visual_cx.run_until_parked();
        let content = visual_cx
            .debug_bounds("container-content")
            .expect("容器内容区应渲染");
        let detail = visual_cx
            .debug_bounds("container-detail-panel")
            .expect("容器详情面板应渲染");
        let actions = visual_cx
            .debug_bounds("container-detail-actions")
            .expect("容器详情操作区应渲染");
        let refresh = visual_cx
            .debug_bounds("container-detail-refresh")
            .expect("刷新状态按钮应渲染");
        assert_inside(content, detail, "容器详情面板");
        assert_inside(detail, actions, "容器详情操作区");
        assert_inside(actions, refresh, "刷新状态按钮");
    }
}
