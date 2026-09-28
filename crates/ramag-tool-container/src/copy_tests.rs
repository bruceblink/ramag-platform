use gpui_kit::Modifiers;
use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, TestAppContext, px, size};

use super::{ContainerSection, ContainerView, append_bounded_text, container_logs_copy_text};
use ramag_domain::entities::{DockerContainerLogLine, DockerContainerLogs, DockerLogStream};

#[test]
fn container_log_copy_text_keeps_stream_labels_and_utf8_boundaries() {
    let logs = DockerContainerLogs {
        container_id: "container-copy".into(),
        lines: vec![
            DockerContainerLogLine {
                stream: DockerLogStream::Stdout,
                message: "已启动".into(),
            },
            DockerContainerLogLine {
                stream: DockerLogStream::Stderr,
                message: "警告".into(),
            },
        ],
        bytes: 12,
        dropped_lines: 0,
        truncated: false,
    };
    assert_eq!(
        container_logs_copy_text(&logs),
        "stdout: 已启动\nstderr: 警告"
    );

    let mut bounded = String::new();
    assert!(!append_bounded_text(&mut bounded, "a界b", 4));
    assert_eq!(bounded, "a界");
    assert!(bounded.is_char_boundary(bounded.len()));
}

#[gpui_kit::test]
fn copy_log_button_copies_only_the_visible_log_window(cx: &mut TestAppContext) {
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
        view.selected_log_container = Some("container-copy".into());
        view.log_follow_paused = true;
        view.pending_follow_lines.push_back(DockerContainerLogLine {
            stream: DockerLogStream::Stderr,
            message: "pending-not-visible".into(),
        });
        view.logs = Some(DockerContainerLogs {
            container_id: "container-copy".into(),
            lines: vec![DockerContainerLogLine {
                stream: DockerLogStream::Stdout,
                message: "visible-line".into(),
            }],
            bytes: 12,
            dropped_lines: 0,
            truncated: false,
        });
        cx.notify();
    });
    visual_cx.simulate_resize(size(px(360.0), px(640.0)));
    visual_cx.run_until_parked();

    let copy = visual_cx
        .debug_bounds("container-logs-copy")
        .expect("复制日志按钮应渲染");
    visual_cx.simulate_click(copy.center(), Modifiers::default());
    visual_cx.run_until_parked();
    assert_eq!(
        visual_cx.read_from_clipboard().and_then(|item| item.text()),
        Some("stdout: visible-line".into())
    );
}
