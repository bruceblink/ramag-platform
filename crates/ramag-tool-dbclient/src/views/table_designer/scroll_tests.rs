use super::*;
use gpui_kit::{Modifiers, ScrollDelta, ScrollEvent, TouchPhase, point};

#[gpui_kit::test]
fn field_editor_and_actions_stay_inside_compact_dialog(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let columns = (0..8)
        .map(|index| column(&format!("field_{index}"), "VARCHAR(255)", true))
        .collect();
    let mut designer_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let designer = cx.new(|cx| {
            TableDesigner::new(
                TableDesignerConfig {
                    driver: DriverKind::Mysql,
                    schema: "public".into(),
                    table: "users".into(),
                    columns,
                    table_has_rows: None,
                    loading: false,
                    ddl_loading: false,
                    on_execute: Rc::new(|_, _, _, _| true),
                    on_rename: Rc::new(|_, _, _, _, _, _| true),
                },
                window,
                cx,
            )
        });
        designer_entity = Some(designer.clone());
        let host = cx.new(|_| DesignerDialogTestHost { designer });
        gpui_kit::component::Root::new(host, window, cx)
    });
    let Some(designer) = designer_entity else {
        unreachable!("测试窗口应创建表设计器")
    };
    cx.update(|window, app| {
        let content = designer.clone();
        window.open_dialog(app, move |dialog, window, _| {
            let content = content.clone();
            dialog
                .title(
                    div()
                        .debug_selector(|| "table-designer-dialog-title".into())
                        .child("修改表 · public.users"),
                )
                .close_button(false)
                .width(ramag_ui::responsive_dialog_width(window, 1080.0))
                .max_h(ramag_ui::responsive_dialog_max_height(window))
                .margin_top(ramag_ui::responsive_dialog_top(window))
                .content(move |body, _, _| body.child(content.clone()))
        });
    });
    cx.run_until_parked();

    for (width, height) in [(360.0, 240.0), (360.0, 620.0), (1024.0, 620.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        cx.run_until_parked();

        let title = cx
            .debug_bounds("table-designer-dialog-title")
            .expect("表设计器弹框标题应渲染");
        let fields = cx
            .debug_bounds("table-designer-field-editor-viewport")
            .expect("字段编辑器滚动视口应渲染");
        let table = cx
            .debug_bounds("table-designer-fields-h-scroll")
            .expect("字段表格应渲染");
        let toolbar = cx
            .debug_bounds("table-designer-top-toolbar")
            .expect("表设计器顶部工具栏应渲染");
        let actions = cx
            .debug_bounds("table-designer-bottom-toolbar")
            .expect("表设计器底部操作区应渲染");
        if height == 240.0 {
            let add_field = cx
                .debug_bounds("field-add")
                .expect("紧凑布局的新增字段按钮应渲染");
            assert!(add_field.origin.y >= title.bottom());
            assert!(add_field.bottom() <= height.into());
        }
        assert!(title.origin.y >= px(0.0));
        assert!(title.bottom() <= px(height));
        assert!(fields.origin.y >= title.bottom());
        assert!(
            fields.bottom() <= px(height),
            "字段滚动区不能越出窗口：toolbar={toolbar:?}, table={table:?}, fields={fields:?}, actions={actions:?}"
        );
        assert!(
            actions.bottom() <= px(height),
            "表设计器底部操作区不能越出窗口：title={title:?}, fields={fields:?}, actions={actions:?}, window={width}x{height}"
        );
        assert!(
            fields.bottom() <= actions.origin.y,
            "字段编辑区可视区域不能覆盖底部操作：table={table:?}, fields={fields:?}, actions={actions:?}, window={width}x{height}"
        );
        if height == 240.0 {
            assert_eq!(
                fields.size.height,
                px(48.0),
                "selector 应测量 48px 滚动视口，而不是完整字段内容：fields={fields:?}"
            );
            assert!(
                table.bottom() > fields.bottom(),
                "紧凑高度下字段内容应通过内部滚动保持可达：table={table:?}, viewport={fields:?}"
            );
            let row_viewport = cx
                .debug_bounds("table-designer-fields-row-viewport")
                .expect("字段行应有独立滚动视口");
            let last_row = cx
                .debug_bounds("table-designer-field-row-7")
                .expect("字段目录末行应渲染");
            assert!(last_row.bottom() > row_viewport.bottom());
            let (scroll_offset_before, scroll_max) = cx.update(|_, app| {
                let designer = designer.read(app);
                (
                    designer.field_scroll.offset(),
                    designer.field_scroll.max_offset(),
                )
            });

            // 首次事件滚动外层以露出字段行视口；第二次事件滚动字段行到末尾。
            cx.simulate_mouse_move(fields.center(), None, Default::default());
            cx.run_until_parked();
            cx.simulate_event(ScrollEvent {
                position: fields.center(),
                delta: ScrollDelta::Pixels(point(px(0.0), px(-1500.0))),
                touch_phase: TouchPhase::Moved,
                ..Default::default()
            });
            cx.run_until_parked();

            let row_viewport_visible = cx
                .debug_bounds("table-designer-fields-row-viewport")
                .expect("外层滚动后字段行视口应渲染");
            let row_scroll_position = point(fields.center().x, row_viewport_visible.center().y);
            cx.simulate_mouse_move(row_scroll_position, None, Default::default());
            cx.run_until_parked();
            cx.simulate_event(ScrollEvent {
                position: row_scroll_position,
                delta: ScrollDelta::Pixels(point(px(0.0), px(-1500.0))),
                touch_phase: TouchPhase::Moved,
                ..Default::default()
            });
            cx.run_until_parked();

            let fields_after = cx
                .debug_bounds("table-designer-field-editor-viewport")
                .expect("滚动后字段编辑器视口应渲染");
            let row_viewport_after = cx
                .debug_bounds("table-designer-fields-row-viewport")
                .expect("滚动后字段行视口应渲染");
            let last_row_after = cx
                .debug_bounds("table-designer-field-row-7")
                .expect("滚动后字段目录末行应渲染");
            let scroll_offset_after = cx.update(|_, app| designer.read(app).field_scroll.offset());
            let actions_after = cx
                .debug_bounds("table-designer-bottom-toolbar")
                .expect("滚动后底部操作区应渲染");
            assert_ne!(
                scroll_offset_after.y, scroll_offset_before.y,
                "滚轮事件应推动字段行滚动句柄：before={scroll_offset_before:?}, after={scroll_offset_after:?}, max={scroll_max:?}"
            );
            assert!(
                last_row_after.origin.y >= row_viewport_after.origin.y
                    && last_row_after.bottom() <= row_viewport_after.bottom(),
                "滚动后字段末行应进入行视口：row={last_row_after:?}, viewport={row_viewport_after:?}"
            );
            assert!(
                last_row_after.origin.y >= fields_after.origin.y
                    && last_row_after.bottom() <= fields_after.bottom(),
                "滚动后字段末行应完整显示在编辑器视口内：row={last_row_after:?}, viewport={fields_after:?}"
            );
            assert!(fields_after.bottom() <= actions_after.origin.y);
            assert!(actions_after.bottom() <= px(height));

            cx.simulate_event(ScrollEvent {
                position: row_scroll_position,
                delta: ScrollDelta::Pixels(point(px(0.0), px(160.0))),
                touch_phase: TouchPhase::Started,
                ..Default::default()
            });
            let vertical_before_shift =
                cx.update(|_, app| designer.read(app).field_scroll.offset().y);
            assert!(
                vertical_before_shift < px(0.0) && vertical_before_shift > -scroll_max.y,
                "Shift 滚轮前，字段行滚动位置应处于中间：offset={vertical_before_shift:?}, max={scroll_max:?}"
            );
            let horizontal_before_shift =
                cx.update(|_, app| designer.read(app).field_horizontal_scroll.offset().x);
            cx.simulate_event(ScrollEvent {
                position: row_scroll_position,
                delta: ScrollDelta::Pixels(point(px(0.0), px(-80.0))),
                modifiers: Modifiers {
                    shift: true,
                    ..Default::default()
                },
                touch_phase: TouchPhase::Moved,
                ..Default::default()
            });
            let horizontal_after_shift =
                cx.update(|_, app| designer.read(app).field_horizontal_scroll.offset().x);
            let vertical_after_shift =
                cx.update(|_, app| designer.read(app).field_scroll.offset().y);
            assert_ne!(
                horizontal_after_shift, horizontal_before_shift,
                "Shift 加垂直滚轮应继续横向滚动字段表格"
            );
            assert_eq!(
                vertical_after_shift, vertical_before_shift,
                "Shift 加垂直滚轮不应移动字段行滚动位置"
            );

            let horizontal_before_trackpad = horizontal_after_shift;
            cx.simulate_event(ScrollEvent {
                position: row_scroll_position,
                delta: ScrollDelta::Pixels(point(px(-60.0), px(0.0))),
                touch_phase: TouchPhase::Started,
                ..Default::default()
            });
            let horizontal_after_trackpad =
                cx.update(|_, app| designer.read(app).field_horizontal_scroll.offset().x);
            assert_ne!(
                horizontal_after_trackpad, horizontal_before_trackpad,
                "横向触控板事件应继续滚动字段表格"
            );

            let offset_before_wheel_lines =
                cx.update(|_, app| designer.read(app).field_scroll.offset());
            for _ in 0..32 {
                // 使用按行计量的滚轮事件，验证字段列表可以滚动到末尾。
                cx.simulate_event(ScrollEvent {
                    position: row_scroll_position,
                    delta: ScrollDelta::Lines(point(0.0, -3.0)),
                    touch_phase: TouchPhase::Moved,
                    ..Default::default()
                });
                cx.run_until_parked();

                let last_row = cx
                    .debug_bounds("table-designer-field-row-7")
                    .expect("滚轮滚动后末行应保持渲染");
                let row_viewport = cx
                    .debug_bounds("table-designer-fields-row-viewport")
                    .expect("滚轮滚动后行视口应保持渲染");
                if last_row.origin.y >= row_viewport.origin.y
                    && last_row.bottom() <= row_viewport.bottom()
                {
                    break;
                }
            }
            let scroll_offset_for_input =
                cx.update(|_, app| designer.read(app).field_scroll.offset());
            assert_eq!(scroll_offset_for_input.y, -scroll_max.y);
            assert_ne!(
                scroll_offset_for_input, offset_before_wheel_lines,
                "Lines 格式的鼠标滚轮事件应推动字段行到末尾"
            );
            let last_row_after_wheel_lines = cx
                .debug_bounds("table-designer-field-row-7")
                .expect("鼠标滚轮后末行应渲染");
            let row_viewport_after_wheel_lines = cx
                .debug_bounds("table-designer-fields-row-viewport")
                .expect("鼠标滚轮后行视口应渲染");
            assert!(
                last_row_after_wheel_lines.origin.y >= row_viewport_after_wheel_lines.origin.y
                    && last_row_after_wheel_lines.bottom()
                        <= row_viewport_after_wheel_lines.bottom(),
                "Lines 格式的鼠标滚轮应让字段末行完整进入行视口：row={last_row_after_wheel_lines:?}, viewport={row_viewport_after_wheel_lines:?}"
            );

            let nullable = cx
                .debug_bounds("field-nullable-7")
                .expect("末行的 NULL 选项应可交互");
            let nullable_before = cx.update(|_, app| designer.read(app).fields[7].nullable);
            cx.simulate_click(nullable.center(), Modifiers::default());
            let nullable_after = cx.update(|_, app| designer.read(app).fields[7].nullable);
            assert_ne!(
                nullable_after, nullable_before,
                "滚动遮罩不应阻止字段复选框点击"
            );

            // 将横向内容滚回首列，再点击末行的实际文本输入框并输入字符。
            cx.simulate_event(ScrollEvent {
                position: row_scroll_position,
                delta: ScrollDelta::Pixels(point(px(1500.0), px(0.0))),
                touch_phase: TouchPhase::Started,
                ..Default::default()
            });
            let row_for_input = cx
                .debug_bounds("table-designer-field-row-7")
                .expect("末行输入框应在滚动后渲染");
            let field_name_before = cx.update(|_, app| {
                designer.read(app).fields[7]
                    .name
                    .read(app)
                    .value()
                    .to_string()
            });
            let name_input_position =
                point(row_for_input.origin.x + px(100.0), row_for_input.center().y);
            cx.simulate_click(name_input_position, Modifiers::default());
            cx.simulate_keystrokes("x");
            let field_name_after = cx.update(|_, app| {
                designer.read(app).fields[7]
                    .name
                    .read(app)
                    .value()
                    .to_string()
            });
            assert_ne!(
                field_name_after, field_name_before,
                "字段末行的文本输入框应获得焦点并接收键盘输入"
            );
            assert!(field_name_after.contains('x'));
        }
    }
}
