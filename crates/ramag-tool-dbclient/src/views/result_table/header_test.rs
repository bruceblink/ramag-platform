use super::{DisplayViewCache, DisplayViewCacheKey, RowFilter, build_display_view};
use crate::views::result_panel::{ResultPanel, ResultState};
use gpui_kit::{
    Modifiers, MouseButton, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase, point, px,
    size,
};
use ramag_domain::entities::{QueryResult, Row, Value};
use std::sync::Arc;

/// A vertical result scroll must move rows without moving the horizontally scrollable header.
#[gpui_kit::test]
fn result_header_stays_fixed_during_vertical_scroll(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let result = Arc::new(QueryResult {
        columns: (0..16).map(|index| format!("column_{index}")).collect(),
        column_types: vec!["TEXT".into(); 16],
        rows: (0..80)
            .map(|row_index| Row {
                values: (0..16)
                    .map(|column_index| Value::Text(format!("{row_index}-{column_index}")))
                    .collect(),
            })
            .collect(),
        affected_rows: 0,
        elapsed_ms: 1,
        warnings: Vec::new(),
        truncated: false,
    });
    let display_view = build_display_view(&result, None, "", "");
    let display_view_key = DisplayViewCacheKey {
        result_identity: Arc::as_ptr(&result) as usize,
        result_revision: 0,
        sort_by: None,
        column_filter: String::new(),
        row_filter: RowFilter::Text(String::new()),
        display_binary_16_as_uuid: true,
    };
    let (panel, cx) = cx.add_window_view(|window, cx| {
        let mut panel = ResultPanel::new(window, cx);
        panel.state = ResultState::Ok(result);
        panel.display_view_cache = Some(DisplayViewCache {
            key: display_view_key,
            view: display_view,
        });
        panel
    });
    cx.simulate_resize(size(px(640.0), px(360.0)));
    panel.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    let table = cx.debug_bounds("result-h-scroll").expect("结果视口应渲染");
    let header_y = cx
        .debug_bounds("result-header")
        .expect("结果表头应渲染")
        .origin
        .y;
    cx.simulate_event(ScrollWheelEvent {
        position: point(table.origin.x + px(80.0), table.origin.y + px(80.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-80.0))),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    cx.run_until_parked();

    let scrolled_header = cx
        .debug_bounds("result-header")
        .expect("纵向滚动后表头仍应渲染");
    assert_eq!(
        scrolled_header.origin.y, header_y,
        "结果表头不能随结果行纵向滚动"
    );
}

/// A restored or dragged width must stay bounded while preserving a usable horizontal range.
#[gpui_kit::test]
fn result_column_width_is_clamped_before_layout(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let result = Arc::new(QueryResult {
        columns: vec!["id".into(), "payload".into()],
        column_types: vec!["BIGINT".into(), "TEXT".into()],
        rows: vec![Row {
            values: vec![Value::Int(1), Value::Text("wide payload".into())],
        }],
        affected_rows: 0,
        elapsed_ms: 1,
        warnings: Vec::new(),
        truncated: false,
    });
    let display_view = build_display_view(&result, None, "", "");
    let display_view_key = DisplayViewCacheKey {
        result_identity: Arc::as_ptr(&result) as usize,
        result_revision: 0,
        sort_by: None,
        column_filter: String::new(),
        row_filter: RowFilter::Text(String::new()),
        display_binary_16_as_uuid: true,
    };
    let (panel, cx) = cx.add_window_view(|window, cx| {
        let mut panel = ResultPanel::new(window, cx);
        panel.state = ResultState::Ok(result.clone());
        panel.display_view_cache = Some(DisplayViewCache {
            key: display_view_key,
            view: display_view,
        });
        panel
    });
    cx.simulate_resize(size(px(640.0), px(360.0)));
    panel.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    let initial_column = cx
        .debug_bounds("result-header-col-1")
        .expect("结果列应渲染独立表头锚点");
    let resize_handle = cx
        .debug_bounds("result-column-resize-1")
        .expect("结果列应渲染拖拽调整手柄");
    let drag_start = point(
        resize_handle.right() - px(0.5),
        resize_handle.origin.y + resize_handle.size.height / 2.0,
    );
    let drag_end = point(drag_start.x + px(20.0), drag_start.y);
    cx.simulate_mouse_move(drag_start, None, Modifiers::default());
    cx.simulate_mouse_down(drag_start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        point(drag_start.x + px(12.0), drag_start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(drag_end, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(drag_end, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();

    panel.read_with(cx, |panel, _| {
        let resized_width = panel.col_width_override(1).expect("拖拽应保存列宽覆盖值");
        assert!(
            resized_width > initial_column.size.width,
            "拖拽列宽应从当前渲染宽度向右增加：initial={:?}, resized={resized_width:?}",
            initial_column.size.width
        );
        assert!(
            resized_width - initial_column.size.width <= px(20.0),
            "首次拖拽不能从固定默认宽度跳变：initial={:?}, resized={resized_width:?}",
            initial_column.size.width
        );
    });

    panel.update(cx, |panel, cx| {
        panel.set_col_width_override(1, px(12.0));
        assert_eq!(panel.col_width_override(1), Some(px(60.0)));
        cx.notify();
    });
    cx.run_until_parked();
    let narrow_column = cx
        .debug_bounds("result-header-col-1")
        .expect("窄列应继续渲染");
    assert_eq!(narrow_column.size.width, px(60.0));

    panel.update(cx, |panel, cx| {
        panel.set_col_width_override(1, px(1200.0));
        assert_eq!(panel.col_width_override(1), Some(px(800.0)));
        cx.notify();
    });
    cx.run_until_parked();
    let column = cx
        .debug_bounds("result-header-col-1")
        .expect("宽列应渲染独立表头锚点");
    assert_eq!(column.size.width, px(800.0));
    panel.read_with(cx, |panel, _| {
        assert!(
            panel.h_scroll.max_offset().x > px(0.0),
            "宽列必须产生可用的横向滚动范围"
        );
    });
}
