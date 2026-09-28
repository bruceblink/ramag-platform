//! 容器管理空工作台的 headless 布局测试。

use gpui_kit::Modifiers;
use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, Bounds, MouseButton, Pixels, TestAppContext, px, size};

use super::{ContainerSection, ContainerView};
use ramag_domain::entities::{
    ContainerPage, DockerContainerLogLine, DockerContainerLogs, DockerImageSummary,
    DockerLogStream, MAX_CONTAINER_LOG_BYTES, MAX_CONTAINER_LOG_LINES,
};

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

#[gpui_kit::test]
fn empty_workspace_stays_inside_supported_window_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
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

        for selector in [
            "container-platform-picker",
            "container-connection-status",
            "container-connection-config",
        ] {
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
            assert_eq!(
                navigation.origin.y, content.origin.y,
                "导航与正文应顶部对齐"
            );
            assert_eq!(navigation.bottom(), content.bottom(), "侧栏应撑满工作区");
            let overview = cx
                .debug_bounds("container-resource-overview")
                .expect("首个导航项应渲染");
            assert!(
                overview.origin.y - navigation.origin.y <= px(12.0),
                "导航项必须从顶部开始，不能垂直居中：{overview:?} / {navigation:?}"
            );
            assert!(
                cx.debug_bounds("container-compact-resource-nav").is_none(),
                "宽窗口不应重复渲染紧凑导航"
            );
        }
    }
}

#[gpui_kit::test]
fn registry_workspace_stays_inside_narrow_window(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
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

#[gpui_kit::test]
fn compact_resource_navigation_wraps_without_full_width_rows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
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
        "container-resource-logs",
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

#[gpui_kit::test]
fn resource_filter_controls_stay_inside_content_on_narrow_and_wide_windows(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::component::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ContainerView::new(window, cx));
        Root::new(view, window, cx)
    });

    for width in [360.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(640.0)));
        cx.run_until_parked();
        let navigation = cx
            .debug_bounds("container-resource-containers")
            .expect("容器资源入口应渲染");
        let point = navigation.center();
        cx.simulate_mouse_down(point, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(point, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();

        let content = cx
            .debug_bounds("container-content")
            .expect("容器内容区应渲染");
        let filter = cx
            .debug_bounds("container-resource-filter")
            .expect("资源筛选工具栏应渲染");
        let input = cx
            .debug_bounds("container-resource-filter-input")
            .expect("资源筛选输入应渲染");
        let apply = cx
            .debug_bounds("container-resource-filter-apply")
            .expect("资源筛选按钮应渲染");
        assert_inside(content, filter, "资源筛选工具栏");
        assert_inside(filter, input, "资源筛选输入");
        assert_inside(filter, apply, "资源筛选按钮");
    }
}

#[test]
fn resource_query_preserves_bounded_search_for_the_driver() {
    let mut view = ContainerView::without_service();
    view.resource_search = "  alpine  ".into();
    let query = view.resource_query();
    assert_eq!(query.page, 1);
    assert_eq!(query.page_size, 100);
    assert_eq!(query.search.as_deref(), Some("  alpine  "));
    assert_eq!(query.normalized_search().as_deref(), Some("alpine"));
}

#[gpui_kit::test]
fn historical_logs_stay_inside_narrow_content_bounds(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ContainerView::new(window, cx));
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = view_entity.expect("容器管理视图应初始化");
    view.update(visual_cx, |view, cx| {
        view.section = ContainerSection::Logs;
        view.selected_log_container = Some("container-logs".into());
        view.logs_loading = true;
        view.logs = Some(DockerContainerLogs {
            container_id: "container-logs".into(),
            lines: vec![DockerContainerLogLine {
                stream: DockerLogStream::Stdout,
                message: "server ready".into(),
            }],
            bytes: 12,
            dropped_lines: 0,
            truncated: false,
        });
        cx.notify();
    });
    for width in [360.0, 1024.0, 1440.0] {
        visual_cx.simulate_resize(size(px(width), px(640.0)));
        visual_cx.run_until_parked();

        let content = visual_cx
            .debug_bounds("container-content")
            .expect("容器内容区应渲染");
        let panel = visual_cx
            .debug_bounds("container-logs-panel")
            .expect("日志面板应渲染");
        let output = visual_cx
            .debug_bounds("container-logs-output")
            .expect("日志输出区应渲染");
        let line = visual_cx
            .debug_bounds("container-log-line-0")
            .expect("日志行应渲染");
        let controls = visual_cx
            .debug_bounds("container-logs-controls")
            .expect("日志控制区应渲染");
        let cancel = visual_cx
            .debug_bounds("container-logs-cancel")
            .expect("停止读取按钮应渲染");
        assert_inside(content, panel, "日志面板");
        assert_inside(panel, output, "日志输出区");
        assert_inside(output, line, "日志行");
        assert_inside(content, controls, "日志控制区");
        assert_inside(controls, cancel, "停止读取按钮");
    }
}

#[gpui_kit::test]
fn continuous_log_controls_stay_inside_supported_window_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ContainerView::new(window, cx));
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = view_entity.expect("容器管理视图应初始化");
    view.update(visual_cx, |view, cx| {
        view.section = ContainerSection::Logs;
        view.selected_log_container = Some("container-follow".into());
        view.logs = Some(DockerContainerLogs {
            container_id: "container-follow".into(),
            lines: vec![DockerContainerLogLine {
                stream: DockerLogStream::Stdout,
                message: "ready".into(),
            }],
            bytes: 5,
            dropped_lines: 0,
            truncated: false,
        });
        cx.notify();
    });

    for width in [360.0, 1024.0, 1440.0] {
        visual_cx.simulate_resize(size(px(width), px(640.0)));
        visual_cx.run_until_parked();
        let content = visual_cx
            .debug_bounds("container-content")
            .expect("容器内容区应渲染");
        let controls = visual_cx
            .debug_bounds("container-logs-controls")
            .expect("持续读取控制区应渲染");
        let follow = visual_cx
            .debug_bounds("container-logs-follow")
            .expect("持续读取按钮应渲染");
        let copy = visual_cx
            .debug_bounds("container-logs-copy")
            .expect("复制日志按钮应渲染");
        let export = visual_cx
            .debug_bounds("container-logs-export")
            .expect("导出日志按钮应渲染");
        assert_inside(content, controls, "持续读取控制区");
        assert_inside(controls, follow, "持续读取按钮");
        assert_inside(controls, copy, "复制日志按钮");
        assert_inside(controls, export, "导出日志按钮");
    }
}

#[gpui_kit::test]
fn paused_log_controls_stay_inside_supported_window_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ContainerView::new(window, cx));
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = view_entity.expect("容器管理视图应初始化");
    view.update(visual_cx, |view, cx| {
        view.section = ContainerSection::Logs;
        view.selected_log_container = Some("container-paused".into());
        view.logs_following = true;
        view.log_follow_paused = true;
        view.logs = Some(DockerContainerLogs {
            container_id: "container-paused".into(),
            lines: vec![DockerContainerLogLine {
                stream: DockerLogStream::Stdout,
                message: "visible".into(),
            }],
            bytes: 7,
            dropped_lines: 0,
            truncated: false,
        });
        cx.notify();
    });

    for width in [360.0, 1024.0, 1440.0] {
        visual_cx.simulate_resize(size(px(width), px(640.0)));
        visual_cx.run_until_parked();
        let content = visual_cx
            .debug_bounds("container-content")
            .expect("容器内容区应渲染");
        let controls = visual_cx
            .debug_bounds("container-logs-controls")
            .expect("暂停控制区应渲染");
        let pause = visual_cx
            .debug_bounds("container-logs-follow-pause")
            .expect("恢复展示按钮应渲染");
        let stop = visual_cx
            .debug_bounds("container-logs-follow-stop")
            .expect("停止持续读取按钮应渲染");
        let copy = visual_cx
            .debug_bounds("container-logs-copy")
            .expect("复制日志按钮应渲染");
        let export = visual_cx
            .debug_bounds("container-logs-export")
            .expect("导出日志按钮应渲染");
        assert_inside(content, controls, "暂停控制区");
        assert_inside(controls, pause, "恢复展示按钮");
        assert_inside(controls, stop, "停止持续读取按钮");
        assert_inside(controls, copy, "复制日志按钮");
        assert_inside(controls, export, "导出日志按钮");
    }
}

#[gpui_kit::test]
fn pause_and_resume_controls_freeze_then_flush_log_lines(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ContainerView::new(window, cx));
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = view_entity.expect("容器管理视图应初始化");
    view.update(visual_cx, |view, cx| {
        view.section = ContainerSection::Logs;
        view.selected_log_container = Some("container-paused".into());
        view.logs_following = true;
        view.log_follow_cancellation = Some(std::sync::Arc::new(
            std::sync::atomic::AtomicBool::new(false),
        ));
        view.logs = Some(DockerContainerLogs {
            container_id: "container-paused".into(),
            lines: vec![DockerContainerLogLine {
                stream: DockerLogStream::Stdout,
                message: "visible".into(),
            }],
            bytes: 7,
            dropped_lines: 0,
            truncated: false,
        });
        cx.notify();
    });
    visual_cx.simulate_resize(size(px(360.0), px(640.0)));
    visual_cx.run_until_parked();

    let pause_button = visual_cx
        .debug_bounds("container-logs-follow-pause")
        .expect("暂停展示按钮应渲染");
    let point = pause_button.center();
    visual_cx.simulate_mouse_down(point, MouseButton::Left, Modifiers::default());
    visual_cx.simulate_mouse_up(point, MouseButton::Left, Modifiers::default());
    visual_cx.run_until_parked();
    assert!(visual_cx.update(|_, app| view.read(app).log_follow_paused));

    view.update(visual_cx, |view, cx| {
        view.queue_follow_log_line(DockerContainerLogLine {
            stream: DockerLogStream::Stderr,
            message: "pending".into(),
        });
        cx.notify();
    });
    assert!(visual_cx.update(|_, app| {
        let view = view.read(app);
        view.logs.as_ref().is_some_and(|logs| logs.lines.len() == 1)
            && view.pending_follow_lines.len() == 1
    }));

    let resume_button = visual_cx
        .debug_bounds("container-logs-follow-pause")
        .expect("恢复展示按钮应继续渲染");
    let point = resume_button.center();
    visual_cx.simulate_mouse_down(point, MouseButton::Left, Modifiers::default());
    visual_cx.simulate_mouse_up(point, MouseButton::Left, Modifiers::default());
    visual_cx.run_until_parked();
    assert!(visual_cx.update(|_, app| {
        let view = view.read(app);
        !view.log_follow_paused
            && view.pending_follow_lines.is_empty()
            && view
                .logs
                .as_ref()
                .is_some_and(|logs| logs.lines.len() == 2 && logs.lines[1].message == "pending")
    }));
}

#[test]
fn continuous_log_window_evicts_oldest_lines_within_budgets() {
    let mut view = ContainerView::without_service();
    view.logs = Some(DockerContainerLogs {
        container_id: "container-follow".into(),
        lines: Vec::new(),
        bytes: 0,
        dropped_lines: 0,
        truncated: false,
    });
    for index in 0..(MAX_CONTAINER_LOG_LINES + 1) {
        view.append_follow_log_line(DockerContainerLogLine {
            stream: DockerLogStream::Stdout,
            message: format!("line-{index}"),
        });
    }
    let logs = view.logs.as_ref().expect("持续日志窗口应存在");
    assert_eq!(logs.lines.len(), MAX_CONTAINER_LOG_LINES);
    assert!(logs.bytes <= MAX_CONTAINER_LOG_BYTES);
    assert_eq!(view.logs_follow_evicted_lines, 1);
}

#[test]
fn paused_log_window_keeps_visible_lines_until_resume_and_bounds_pending_lines() {
    let mut view = ContainerView::without_service();
    view.logs_following = true;
    view.log_follow_paused = true;
    view.logs = Some(DockerContainerLogs {
        container_id: "container-paused".into(),
        lines: vec![DockerContainerLogLine {
            stream: DockerLogStream::Stdout,
            message: "visible".into(),
        }],
        bytes: 7,
        dropped_lines: 0,
        truncated: false,
    });
    view.queue_follow_log_line(DockerContainerLogLine {
        stream: DockerLogStream::Stdout,
        message: "waiting-1".into(),
    });
    view.queue_follow_log_line(DockerContainerLogLine {
        stream: DockerLogStream::Stdout,
        message: "waiting-2".into(),
    });
    assert_eq!(view.logs.as_ref().expect("日志窗口应存在").lines.len(), 1);
    assert_eq!(view.pending_follow_lines.len(), 2);

    view.resume_container_log_follow();
    let logs = view.logs.as_ref().expect("日志窗口应存在");
    assert_eq!(logs.lines.len(), 3);
    assert_eq!(logs.lines[1].message, "waiting-1");
    assert_eq!(logs.lines[2].message, "waiting-2");
    assert!(view.pending_follow_lines.is_empty());

    view.log_follow_paused = true;
    view.logs_follow_evicted_lines = 0;
    for index in 0..(MAX_CONTAINER_LOG_LINES + 1) {
        view.queue_follow_log_line(DockerContainerLogLine {
            stream: DockerLogStream::Stdout,
            message: format!("pending-{index}"),
        });
    }
    assert_eq!(view.pending_follow_lines.len(), MAX_CONTAINER_LOG_LINES);
    assert!(view.pending_follow_bytes <= MAX_CONTAINER_LOG_BYTES);
    assert_eq!(view.logs_follow_evicted_lines, 1);
}

#[gpui_kit::test]
fn image_rows_keep_long_names_and_subtitles_inside_narrow_window(cx: &mut TestAppContext) {
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
