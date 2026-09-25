use super::{DisplayViewCache, DisplayViewCacheKey, RowFilter, build_display_view};
use crate::views::result_panel::{ResultPanel, ResultState};
use gpui_kit::{ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase, point, px, size};
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
