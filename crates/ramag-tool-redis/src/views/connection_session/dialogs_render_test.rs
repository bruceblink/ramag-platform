use super::RedisSessionPanel;
use gpui_kit::component::Root;
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, Modifiers, ParentElement, Render, ScrollDelta,
    ScrollWheelEvent, Styled, TestAppContext, TouchPhase, Window, div, point, px, size,
};

struct RedisDialogTestHost {
    session: Entity<RedisSessionPanel>,
}

impl Render for RedisDialogTestHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        div()
            .relative()
            .size_full()
            .child(self.session.clone())
            .children(dialog_layer)
    }
}

#[gpui_kit::test]
fn stream_dialog_keeps_save_controls_visible_after_adding_rows_in_compact_window(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::component::init);
    let mut session_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let session = cx.new(|cx| {
            RedisSessionPanel::new(
                crate::views::key_detail::render_test::mock_config(),
                crate::views::key_detail::render_test::mock_service(),
                window,
                cx,
            )
        });
        session_entity = Some(session.clone());
        let host = cx.new(|_| RedisDialogTestHost { session });
        Root::new(host, window, cx)
    });
    let Some(session) = session_entity else {
        unreachable!("测试窗口应创建 Redis 会话")
    };

    cx.update(|window, app| {
        session.update(app, |session, cx| {
            session.open_stream_entry_dialog("events:large".into(), window, cx);
        });
    });
    cx.run_until_parked();

    for _ in 0..6 {
        let add = cx
            .debug_bounds("redis-pairs-add")
            .expect("Stream 字段编辑器应显示添加按钮");
        cx.simulate_click(add.center(), Modifiers::default());
        cx.run_until_parked();
    }
    assert!(
        cx.debug_bounds("redis-pairs-row-6").is_some(),
        "测试数据应包含 7 行 Stream 字段"
    );

    cx.simulate_resize(size(px(360.0), px(240.0)));
    cx.run_until_parked();

    let dialog = cx
        .debug_bounds("dialog-0")
        .expect("GPUI Kit Stream 新增弹框应渲染");
    let fields = cx
        .debug_bounds("redis-stream-fields-scroll")
        .expect("Stream 字段区应渲染");
    let toolbar = cx
        .debug_bounds("redis-pairs-toolbar")
        .expect("Stream 字段操作区应渲染");
    let footer = cx
        .debug_bounds("redis-stream-footer")
        .expect("Stream 保存操作区应渲染");

    assert!(dialog.bottom() <= px(240.0));
    assert!(fields.bottom() <= toolbar.origin.y);
    assert!(toolbar.origin.y > fields.bottom());
    assert!(footer.bottom() <= dialog.bottom());

    cx.simulate_event(ScrollWheelEvent {
        position: fields.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-500.0))),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    cx.run_until_parked();

    let scrolled_toolbar = cx
        .debug_bounds("redis-pairs-toolbar")
        .expect("字段滚动后 Stream 操作区应渲染");
    let stable_footer = cx
        .debug_bounds("redis-stream-footer")
        .expect("字段滚动后 Stream 保存操作区应保持可见");
    assert!(scrolled_toolbar.origin.y < toolbar.origin.y);
    assert!(scrolled_toolbar.bottom() <= fields.bottom());
    assert!((stable_footer.origin.y - footer.origin.y).abs() <= px(2.0));
    assert!(stable_footer.bottom() <= dialog.bottom());
}
