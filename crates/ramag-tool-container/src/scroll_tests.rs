use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, TestAppContext, px, size};
use ramag_domain::entities::{DockerContainerLogLine, DockerContainerLogs, DockerLogStream};

use super::{ContainerSection, ContainerView};

#[gpui_kit::test]
fn live_log_append_follows_the_bottom_of_the_output(cx: &mut TestAppContext) {
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
        view.selected_log_container = Some("container-scroll".into());
        view.logs_following = true;
        view.log_follow_cancellation = Some(std::sync::Arc::new(
            std::sync::atomic::AtomicBool::new(false),
        ));
        let lines = (0..48)
            .map(|index| DockerContainerLogLine {
                stream: DockerLogStream::Stdout,
                message: format!("line-{index}"),
            })
            .collect::<Vec<_>>();
        view.logs = Some(DockerContainerLogs {
            container_id: "container-scroll".into(),
            bytes: lines.iter().map(|line| line.message.len()).sum(),
            lines,
            dropped_lines: 0,
            truncated: false,
        });
        cx.notify();
    });
    visual_cx.simulate_resize(size(px(360.0), px(640.0)));
    visual_cx.run_until_parked();

    view.update(visual_cx, |view, cx| {
        view.append_follow_log_line(DockerContainerLogLine {
            stream: DockerLogStream::Stdout,
            message: "latest".into(),
        });
        cx.notify();
    });
    visual_cx.run_until_parked();

    let (offset, max_offset) = visual_cx.update(|_, app| {
        let view = view.read(app);
        (view.logs_scroll.offset(), view.logs_scroll.max_offset())
    });
    assert!(
        max_offset.y > px(0.0),
        "窄窗口的日志输出应产生垂直滚动范围: max_offset={max_offset:?}"
    );
    assert!(
        offset.y >= -max_offset.y - px(1.0) && offset.y <= -max_offset.y + px(1.0),
        "新增日志后输出区应跟到最后一行: offset={offset:?}, max_offset={max_offset:?}"
    );
}
