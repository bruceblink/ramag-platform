use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, Bounds, Pixels, TestAppContext, px, size};
use ramag_domain::entities::{DockerContainerDetail, DockerContainerStats, DockerContainerSummary};
use ramag_domain::error::{ContainerError, ContainerErrorCategory, DomainError};

use super::{
    ContainerSection, ContainerView, MAX_CONTAINER_STATS_HISTORY, SelectedDetail,
    container_stats_history_text, container_stats_text, stats_trend_bar_height,
};

fn assert_inside(parent: Bounds<Pixels>, child: Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label}越出父容器：parent={parent:?}, child={child:?}"
    );
}

fn test_detail() -> DockerContainerDetail {
    DockerContainerDetail {
        summary: DockerContainerSummary {
            id: "container-stats".into(),
            names: vec!["/stats".into()],
            image: Some("alpine:3.20".into()),
            image_id: None,
            command: Some("sleep 60".into()),
            created: None,
            state: Some("running".into()),
            status: Some("Up 2 minutes".into()),
            health: None,
            ports: Vec::new(),
            networks: Vec::new(),
            labels: Vec::new(),
        },
        path: Some("/bin/sh".into()),
        args: Vec::new(),
        platform: Some("linux".into()),
        working_directory: None,
        entrypoint: Vec::new(),
        command: vec!["sleep".into(), "60".into()],
        restart_policy: None,
        env_keys: Vec::new(),
        mounts: Vec::new(),
        networks: Vec::new(),
    }
}

fn test_stats() -> DockerContainerStats {
    DockerContainerStats {
        container_id: "container-stats".into(),
        name: Some("/stats".into()),
        read_at: Some("2026-09-28T00:00:00Z".into()),
        cpu_percent: Some(12.5),
        memory_usage_bytes: Some(512),
        memory_limit_bytes: Some(1024),
        memory_percent: Some(50.0),
        network_rx_bytes: Some(10),
        network_tx_bytes: Some(20),
    }
}

#[test]
fn container_stats_text_keeps_unknown_values_explicit() {
    let mut stats = test_stats();
    let text = container_stats_text(&stats);
    assert!(text.contains("CPU 使用率：12.5%"));
    assert!(text.contains("内存：512 B / 1.0 KiB（50.0%）"));
    assert!(text.contains("网络接收：10 B"));

    stats.cpu_percent = None;
    stats.memory_limit_bytes = None;
    stats.memory_percent = None;
    assert!(container_stats_text(&stats).contains("CPU 使用率：未知"));
    assert!(container_stats_text(&stats).contains("内存：512 B / 未知（未知）"));
}

#[test]
fn failed_stats_refresh_keeps_the_previous_snapshot() {
    let mut view = ContainerView::without_service();
    let previous = test_stats();
    view.container_stats = Some(previous.clone());
    view.stats_loading = true;
    view.apply_container_stats_result(Err(DomainError::Container(ContainerError::new(
        ContainerErrorCategory::Network,
        "读取 Docker 容器指标",
        "Docker Engine 请求失败",
    ))));
    assert_eq!(view.container_stats, Some(previous));
    assert!(!view.stats_loading);
    assert!(view.error.is_some());
}

#[test]
fn successful_stats_refreshes_keep_ordered_bounded_history() {
    let mut view = ContainerView::without_service();
    for index in 0..(MAX_CONTAINER_STATS_HISTORY + 3) {
        let mut stats = test_stats();
        stats.read_at = Some(format!("sample-{index}"));
        view.apply_container_stats_result(Ok(stats));
    }
    assert_eq!(
        view.container_stats_history.len(),
        MAX_CONTAINER_STATS_HISTORY
    );
    assert_eq!(
        view.container_stats_history
            .front()
            .and_then(|stats| stats.read_at.as_deref()),
        Some("sample-3")
    );
    assert_eq!(
        view.container_stats_history
            .back()
            .and_then(|stats| stats.read_at.as_deref()),
        Some("sample-22")
    );
    let text = container_stats_history_text(&view.container_stats_history);
    assert!(text.starts_with("最近 20 次成功刷新\n1 · sample-22"));
}

#[test]
fn stats_trend_bar_height_bounds_percentages_and_unknown_values() {
    assert_eq!(stats_trend_bar_height(None), 4.0);
    assert_eq!(stats_trend_bar_height(Some(0.0)), 4.0);
    assert_eq!(stats_trend_bar_height(Some(50.0)), 26.0);
    assert_eq!(stats_trend_bar_height(Some(100.0)), 48.0);
    assert_eq!(stats_trend_bar_height(Some(150.0)), 48.0);
}

#[gpui_kit::test]
fn container_stats_panel_stays_inside_supported_window_widths(cx: &mut TestAppContext) {
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
        view.selected_detail = Some(SelectedDetail::Container(test_detail()));
        view.container_stats = Some(test_stats());
        view.container_stats_history.push_back(test_stats());
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
        let viewport = visual_cx
            .debug_bounds("container-detail-scroll")
            .expect("详情应保留独立滚动视口");
        let information = visual_cx
            .debug_bounds("container-detail-information")
            .expect("详情与资源指标应位于同一滚动内容中");
        let actions = visual_cx
            .debug_bounds("container-detail-actions")
            .expect("容器详情操作区应渲染");
        let stats = visual_cx
            .debug_bounds("container-detail-stats-panel")
            .expect("容器资源指标面板应渲染");
        let history = visual_cx
            .debug_bounds("container-detail-stats-history-panel")
            .expect("容器指标历史面板应渲染");
        let trend = visual_cx
            .debug_bounds("container-detail-stats-trend-panel")
            .expect("容器指标趋势面板应渲染");
        let cpu_trend = visual_cx
            .debug_bounds("container-detail-stats-cpu-trend")
            .expect("CPU 趋势行应渲染");
        let memory_trend = visual_cx
            .debug_bounds("container-detail-stats-memory-trend")
            .expect("内存趋势行应渲染");
        let refresh = visual_cx
            .debug_bounds("container-detail-stats")
            .expect("刷新指标按钮应渲染");
        assert_inside(content, detail, "容器详情面板");
        assert_inside(detail, viewport, "详情滚动视口");
        assert!(viewport.size.height > px(0.0));
        assert_inside(detail, actions, "容器详情操作区");
        assert_inside(actions, refresh, "刷新指标按钮");
        assert_inside(information, stats, "容器资源指标面板");
        assert_inside(information, history, "容器指标历史面板");
        assert_inside(information, trend, "容器指标趋势面板");
        assert!(
            information.size.height > viewport.size.height,
            "长详情应产生独立滚动内容，而不是扩大工作区"
        );
        assert_inside(trend, cpu_trend, "CPU 趋势行");
        assert_inside(trend, memory_trend, "内存趋势行");
    }
}
