use super::RedisSessionPanel;
use gpui_kit::component::Root;
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement, Render, ScrollDelta,
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
fn value_edit_dialog_keeps_save_controls_visible_in_compact_window(cx: &mut TestAppContext) {
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
            session.open_value_dialog("compact:value".into(), "line\n".repeat(40), window, cx);
        });
    });
    cx.simulate_resize(size(px(360.0), px(240.0)));
    cx.run_until_parked();

    let dialog = cx
        .debug_bounds("dialog-0")
        .expect("GPUI Kit Redis 值编辑弹框应渲染");
    let body = cx
        .debug_bounds("redis-value-edit-dialog-body")
        .expect("Redis 值编辑弹框正文应渲染");
    let fields = cx
        .debug_bounds("redis-value-edit-fields-scroll")
        .expect("Redis 值编辑字段区应渲染");
    let textarea = cx
        .debug_bounds("redis-value-edit-textarea")
        .expect("Redis 值编辑文本区应渲染");
    let footer = cx
        .debug_bounds("redis-value-edit-footer")
        .expect("Redis 值编辑操作区应渲染");

    assert!(dialog.bottom() <= px(240.0));
    assert!(body.bottom() <= dialog.bottom());
    assert!(fields.bottom() <= footer.origin.y);
    assert!(textarea.bottom() > fields.bottom());
    assert!(footer.bottom() <= dialog.bottom());

    cx.simulate_event(ScrollWheelEvent {
        position: fields.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    cx.run_until_parked();

    let scrolled_textarea = cx
        .debug_bounds("redis-value-edit-textarea")
        .expect("Redis 值编辑文本区应随字段滚动");
    let scrolled_fields = cx
        .debug_bounds("redis-value-edit-fields-scroll")
        .expect("Redis 值编辑字段区应保持可见");
    let scrolled_footer = cx
        .debug_bounds("redis-value-edit-footer")
        .expect("Redis 值编辑操作区应保持可见");
    assert!(scrolled_textarea.origin.y < textarea.origin.y);
    assert!(scrolled_footer.origin.y >= scrolled_fields.bottom() - px(1.0));
    assert!(scrolled_footer.bottom() <= dialog.bottom());

    cx.simulate_resize(size(px(360.0), px(640.0)));
    cx.run_until_parked();
    let resized_body = cx
        .debug_bounds("redis-value-edit-dialog-body")
        .expect("窗口增高后 Redis 值编辑正文应重新布局");
    let resized_dialog = cx
        .debug_bounds("dialog-0")
        .expect("窗口增高后 Redis 值编辑弹框应渲染");
    let resized_footer = cx
        .debug_bounds("redis-value-edit-footer")
        .expect("窗口增高后 Redis 值编辑操作区应渲染");
    assert!(resized_body.size.height > body.size.height);
    assert!(resized_footer.bottom() <= resized_dialog.bottom());
    assert!(resized_dialog.bottom() <= px(640.0));
}
