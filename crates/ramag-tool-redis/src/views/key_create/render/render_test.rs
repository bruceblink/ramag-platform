use super::KeyCreateForm;
use gpui_kit::component::{Root, WindowExt as _};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement, Render,
    ScrollDelta, ScrollWheelEvent, Styled, TestAppContext, TouchPhase, Window, div, point, px,
    size,
};

struct KeyCreateDialogTestHost {
    form: Entity<KeyCreateForm>,
}

impl Render for KeyCreateDialogTestHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        div()
            .relative()
            .size_full()
            .child(self.form.clone())
            .children(dialog_layer)
    }
}

#[gpui_kit::test]
fn key_create_dialog_keeps_footer_visible_in_compact_window(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut form_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let form = cx.new(|cx| {
            KeyCreateForm::new(
                crate::views::key_detail::render_test::mock_service(),
                crate::views::key_detail::render_test::mock_config(),
                0,
                window,
                cx,
            )
        });
        form_entity = Some(form.clone());
        let host = cx.new(|_| KeyCreateDialogTestHost { form });
        Root::new(host, window, cx)
    });
    let Some(form) = form_entity else {
        unreachable!("测试窗口应创建 Redis 键表单")
    };

    cx.update(|window, app| {
        let form = form.clone();
        window.open_dialog(app, move |dialog, window, _| {
            let form = form.clone();
            dialog
                .title("新建 Key")
                .close_button(false)
                .w(px(640.0))
                .max_h(ramag_ui::responsive_dialog_max_height(window))
                .margin_top(ramag_ui::responsive_dialog_top(window))
                .p(px(24.0))
                .content(move |content, window, _| {
                    let body_height = (ramag_ui::responsive_dialog_max_height(window) - px(96.0))
                        .clamp(px(32.0), px(540.0));
                    content.child(
                        div()
                            .debug_selector(|| "redis-key-create-dialog-body".into())
                            .w_full()
                            .h(body_height)
                            .max_h(body_height)
                            .min_h_0()
                            .flex_none()
                            .overflow_hidden()
                            .child(form.clone()),
                    )
                })
        });
    });
    cx.simulate_resize(size(px(360.0), px(240.0)));
    cx.run_until_parked();

    let dialog = cx
        .debug_bounds("dialog-0")
        .expect("GPUI Kit 新建 Key 弹框应渲染");
    let fields = cx
        .debug_bounds("redis-key-create-fields-scroll")
        .expect("Redis 键表单字段区应渲染");
    let body = cx
        .debug_bounds("redis-key-create-dialog-body")
        .expect("Redis 键表单弹框正文应渲染");
    let editor = cx
        .debug_bounds("redis-key-create-value-editor")
        .expect("Redis 键值编辑区应渲染");
    let footer = cx
        .debug_bounds("redis-key-create-footer")
        .expect("Redis 键表单操作区应渲染");

    assert!(dialog.origin.y >= px(0.0));
    assert!(dialog.bottom() <= px(240.0));
    assert!(fields.size.height > px(0.0));
    assert!(fields.origin.y >= dialog.origin.y);
    assert!(editor.bottom() > fields.bottom());
    assert!(
        fields.bottom() <= footer.origin.y,
        "字段滚动区不能覆盖固定操作区：dialog={dialog:?}, body={body:?}, fields={fields:?}, footer={footer:?}"
    );
    assert!(editor.origin.y >= fields.origin.y);
    assert!(footer.bottom() <= dialog.bottom());

    cx.simulate_event(ScrollWheelEvent {
        position: fields.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-480.0))),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    cx.run_until_parked();

    let scrolled_editor = cx
        .debug_bounds("redis-key-create-value-editor")
        .expect("Redis 键值编辑区应随字段滚动");
    let stable_footer = cx
        .debug_bounds("redis-key-create-footer")
        .expect("Redis 键表单操作区应保持可见");
    let scrolled_fields = cx
        .debug_bounds("redis-key-create-fields-scroll")
        .expect("Redis 键表单字段区应继续参与布局");
    let scrolled_dialog = cx
        .debug_bounds("dialog-0")
        .expect("滚动字段后新建 Key 弹框应保持打开");
    assert!(scrolled_editor.origin.y < editor.origin.y);
    assert!(stable_footer.origin.y >= scrolled_fields.bottom() - px(1.0));
    assert!(stable_footer.bottom() <= scrolled_dialog.bottom());

    cx.simulate_resize(size(px(360.0), px(300.0)));
    cx.run_until_parked();
    let resized_body = cx
        .debug_bounds("redis-key-create-dialog-body")
        .expect("调整窗口后 Redis 键表单正文应重新布局");
    let resized_dialog = cx
        .debug_bounds("dialog-0")
        .expect("调整窗口后 GPUI Kit 新建 Key 弹框应渲染");
    let resized_footer = cx
        .debug_bounds("redis-key-create-footer")
        .expect("调整窗口后 Redis 键表单操作区应渲染");
    assert!(resized_body.size.height > body.size.height);
    assert!(resized_dialog.bottom() <= px(300.0));
    assert!(resized_footer.bottom() <= resized_dialog.bottom());

    cx.simulate_resize(size(px(1280.0), px(800.0)));
    cx.run_until_parked();
    let desktop_body = cx
        .debug_bounds("redis-key-create-dialog-body")
        .expect("宽屏上 Redis 键表单正文应渲染");
    let desktop_dialog = cx
        .debug_bounds("dialog-0")
        .expect("宽屏上 GPUI Kit 新建 Key 弹框应渲染");
    assert!(desktop_body.size.height <= px(540.0));
    assert!(desktop_dialog.size.height < px(700.0));
    assert!(desktop_dialog.bottom() <= px(800.0));
}
