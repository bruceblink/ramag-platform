use super::RedisSessionPanel;
use gpui_kit::component::{Root, WindowExt as _};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, Modifiers, MouseButton, ParentElement, Render,
    ScrollDelta, ScrollWheelEvent, Styled, TestAppContext, TouchPhase, VisualTestContext, Window,
    div, point, px, size,
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

enum LinesDialog {
    List,
    Set,
}

fn open_lines_dialog(
    cx: &mut VisualTestContext,
    session: &Entity<RedisSessionPanel>,
    kind: LinesDialog,
) {
    cx.update(|window, app| {
        session.update(app, |session, cx| match kind {
            LinesDialog::List => session.open_list_element_dialog("items:large".into(), window, cx),
            LinesDialog::Set => session.open_set_element_dialog("members:large".into(), window, cx),
        });
    });
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(300));
    cx.run_until_parked();

    for _ in 0..6 {
        let add = cx
            .debug_bounds("redis-lines-add")
            .expect("List/Set 编辑器应显示添加按钮");
        let center = point(add.center().x, add.center().y);
        cx.simulate_mouse_move(center, None, Modifiers::default());
        cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(center, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();
        let moved_add = cx
            .debug_bounds("redis-lines-add")
            .expect("添加按钮应在行数更新后继续渲染");
        assert!(
            moved_add.origin.y > add.origin.y,
            "添加按钮点击应增加一行：before={add:?}, after={moved_add:?}"
        );
    }
}

fn assert_compact_lines_dialog(
    cx: &mut VisualTestContext,
    fields_selector: &'static str,
    footer_selector: &'static str,
) {
    cx.simulate_resize(size(px(360.0), px(240.0)));
    cx.run_until_parked();

    let dialog = cx
        .debug_bounds("dialog-0")
        .expect("GPUI Kit List/Set 弹框应渲染");
    let fields = cx
        .debug_bounds(fields_selector)
        .expect("List/Set 字段区应渲染");
    let toolbar = cx
        .debug_bounds("redis-lines-toolbar")
        .expect("List/Set 字段操作区应渲染");
    let footer = cx
        .debug_bounds(footer_selector)
        .expect("List/Set 保存操作区应渲染");

    assert!(dialog.bottom() <= px(240.0));
    assert!(
        toolbar.bottom() > fields.bottom(),
        "多行输入应让字段工具栏超出滚动视口：fields={fields:?}, toolbar={toolbar:?}"
    );
    assert!(footer.bottom() <= dialog.bottom());

    cx.simulate_event(ScrollWheelEvent {
        position: fields.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-500.0))),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    cx.run_until_parked();

    let scrolled_toolbar = cx
        .debug_bounds("redis-lines-toolbar")
        .expect("滚动字段后 List/Set 操作区应渲染");
    let stable_footer = cx
        .debug_bounds(footer_selector)
        .expect("滚动字段后 List/Set 保存区应保持可见");
    assert!(scrolled_toolbar.origin.y < toolbar.origin.y);
    assert!(scrolled_toolbar.bottom() <= fields.bottom());
    assert!((stable_footer.origin.y - footer.origin.y).abs() <= px(2.0));
    assert!(stable_footer.bottom() <= dialog.bottom());
}

#[gpui_kit::test]
fn list_and_set_dialogs_keep_actions_visible_after_adding_rows(cx: &mut TestAppContext) {
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

    open_lines_dialog(cx, &session, LinesDialog::List);
    assert_compact_lines_dialog(cx, "redis-list-fields-scroll", "redis-list-footer");
    cx.update(|window, app| window.close_dialog(app));
    cx.run_until_parked();

    open_lines_dialog(cx, &session, LinesDialog::Set);
    assert_compact_lines_dialog(cx, "redis-set-fields-scroll", "redis-set-footer");
}
