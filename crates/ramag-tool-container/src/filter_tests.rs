use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, Bounds, Pixels, TestAppContext, px, size};
use ramag_domain::entities::{DockerContainerLogLine, DockerContainerLogs, DockerLogStream};

use super::{ContainerSection, ContainerView, filtered_log_line_count, normalize_log_search};

fn assert_inside(parent: Bounds<Pixels>, child: Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label}越出父容器：parent={parent:?}, child={child:?}"
    );
}

fn sample_logs() -> DockerContainerLogs {
    DockerContainerLogs {
        container_id: "container-filter".into(),
        lines: vec![
            DockerContainerLogLine {
                stream: DockerLogStream::Stdout,
                message: "server ready".into(),
            },
            DockerContainerLogLine {
                stream: DockerLogStream::Stderr,
                message: "Database ERROR".into(),
            },
            DockerContainerLogLine {
                stream: DockerLogStream::Stdout,
                message: "request complete".into(),
            },
        ],
        bytes: 36,
        dropped_lines: 0,
        truncated: false,
    }
}

#[test]
fn local_log_filter_matches_message_and_stream_without_changing_window() {
    let logs = sample_logs();
    assert_eq!(
        filtered_log_line_count(&logs, &normalize_log_search(" error ")),
        1
    );
    assert_eq!(
        filtered_log_line_count(&logs, &normalize_log_search("STDERR")),
        1
    );
    assert_eq!(
        filtered_log_line_count(&logs, &normalize_log_search("missing")),
        0
    );
    assert_eq!(
        filtered_log_line_count(&logs, &normalize_log_search("  ")),
        3
    );
    assert_eq!(logs.lines.len(), 3);
}

#[gpui_kit::test]
fn log_filter_toolbar_and_results_stay_inside_supported_window_widths(cx: &mut TestAppContext) {
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
        view.selected_log_container = Some("container-filter".into());
        view.logs_search = "error".into();
        view.logs = Some(sample_logs());
        cx.notify();
    });

    for width in [360.0, 1024.0, 1440.0] {
        visual_cx.simulate_resize(size(px(width), px(640.0)));
        visual_cx.run_until_parked();

        let content = visual_cx
            .debug_bounds("container-content")
            .expect("容器内容区应渲染");
        let filter = visual_cx
            .debug_bounds("container-log-filter")
            .expect("日志筛选工具栏应渲染");
        let input = visual_cx
            .debug_bounds("container-log-filter-input")
            .expect("日志筛选输入框应渲染");
        let output = visual_cx
            .debug_bounds("container-logs-output")
            .expect("日志输出区应渲染");
        let matched = visual_cx
            .debug_bounds("container-log-line-1")
            .expect("匹配日志行应渲染");

        assert_inside(content, filter, "日志筛选工具栏");
        assert_inside(filter, input, "日志筛选输入框");
        assert_inside(content, output, "日志输出区");
        assert_inside(output, matched, "匹配日志行");
    }
}
