use super::pagination::format_result_page_range;
use super::{
    DisplayViewCache, DisplayViewCacheKey, MAX_ROWS_DISPLAY, RowFilter, build_display_view,
};
use crate::views::result_panel::{ResultPagination, ResultPanel, ResultState, TotalRows};
use gpui_kit::{ScrollStrategy, TestAppContext, px, size};
use ramag_domain::entities::{QueryResult, Row, Value};
use std::sync::Arc;

/// Verifies the final virtual row and paging controls remain in their own visible regions.
#[gpui_kit::test]
fn final_virtual_row_and_pagination_stay_inside_result_regions(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    cx.set_global(ramag_ui::DatabaseResultSettingsGlobal::new(
        ramag_ui::DatabaseResultSettings {
            show_horizontal_scrollbar: true,
            display_binary_16_as_uuid: true,
        },
    ));

    let result = Arc::new(QueryResult {
        columns: vec![
            format!("long_column_header_{}", "segment_".repeat(40)),
            "wide_payload".into(),
        ],
        column_types: vec!["BIGINT".into(), "TEXT".into()],
        rows: (0..MAX_ROWS_DISPLAY)
            .map(|index| Row {
                values: vec![
                    Value::Int(index as i64 + 1),
                    Value::Text(format!("payload-{index}")),
                ],
            })
            .collect(),
        affected_rows: 0,
        elapsed_ms: 25,
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
        panel.pagination = Some(ResultPagination {
            page: 1,
            page_size: MAX_ROWS_DISPLAY,
            has_more: false,
            total: TotalRows::Known((MAX_ROWS_DISPLAY * 2) as u64),
        });
        panel.set_col_width_override(1, px(800.0));
        panel.display_view_cache = Some(DisplayViewCache {
            key: display_view_key,
            view: display_view,
        });
        panel
    });
    cx.simulate_resize(size(px(640.0), px(480.0)));
    panel.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    panel.update(cx, |panel, cx| {
        panel
            .uniform_scroll
            .scroll_to_item(MAX_ROWS_DISPLAY - 1, ScrollStrategy::Bottom);
        cx.notify();
    });
    cx.run_until_parked();

    let viewport = cx.debug_bounds("result-h-scroll").expect("结果视口应渲染");
    let header = cx.debug_bounds("result-header").expect("结果表头应渲染");
    let long_header = cx
        .debug_bounds("result-header-col-0")
        .expect("长列标题应渲染");
    let wide_header = cx
        .debug_bounds("result-header-col-1")
        .expect("宽列标题应渲染");
    let final_row = cx
        .debug_bounds("result-row-9999")
        .expect("跳转到结果末端后最后一行应仍在虚拟列表中");
    let vertical_scrollbar = cx
        .debug_bounds("result-v-scrollbar")
        .expect("应渲染垂直滚动条");
    let horizontal_scrollbar = cx
        .debug_bounds("result-h-scrollbar")
        .expect("应渲染水平滚动条");
    let status_bar = cx
        .debug_bounds("result-status-bar")
        .expect("分页状态栏应渲染");
    let page_range = cx
        .debug_bounds("result-page-range")
        .expect("分页范围应渲染");
    let pagination_controls = cx
        .debug_bounds("result-pagination-controls")
        .expect("分页按钮组应渲染");

    assert_eq!(
        format_result_page_range(
            ResultPagination {
                page: 1,
                page_size: MAX_ROWS_DISPLAY,
                has_more: false,
                total: TotalRows::Known((MAX_ROWS_DISPLAY * 2) as u64),
            },
            MAX_ROWS_DISPLAY,
            MAX_ROWS_DISPLAY,
        ),
        "10001-20000 of 20000"
    );
    assert!(long_header.size.width <= px(380.0));
    assert_eq!(wide_header.size.width, px(800.0));
    assert!(
        final_row.origin.y >= header.bottom() && final_row.bottom() <= viewport.bottom(),
        "末行必须留在结果行视口内：row={final_row:?}, header={header:?}, viewport={viewport:?}"
    );
    assert!(
        vertical_scrollbar.bottom() <= horizontal_scrollbar.origin.y,
        "垂直滚动条不能覆盖水平滚动条：vertical={vertical_scrollbar:?}, horizontal={horizontal_scrollbar:?}"
    );
    assert!(
        horizontal_scrollbar.bottom() <= status_bar.origin.y,
        "水平滚动条不能覆盖分页状态栏：horizontal={horizontal_scrollbar:?}, status={status_bar:?}"
    );
    for (label, bounds) in [
        ("page range", page_range),
        ("pagination controls", pagination_controls),
    ] {
        assert!(
            bounds.origin.x >= status_bar.origin.x
                && bounds.right() <= status_bar.right()
                && bounds.origin.y >= status_bar.origin.y
                && bounds.bottom() <= status_bar.bottom(),
            "{label} 不能越出状态栏：element={bounds:?}, status={status_bar:?}"
        );
    }
    panel.read_with(cx, |panel, _| {
        assert!(
            panel.h_scroll.max_offset().x >= px(400.0),
            "长标题和宽列应保留可用的横向滚动范围：{:?}",
            panel.h_scroll.max_offset()
        );
        assert!(
            panel.uniform_scroll.0.borrow().base_handle.offset().y < px(0.0),
            "跳转至末行后虚拟列表应有正向滚动偏移"
        );
    });
}

/// Exercises the ten-column, mixed-value shape used by the local MySQL bulk table.
#[gpui_kit::test]
fn mixed_bulk_table_rows_render_in_the_result_grid(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let columns = vec![
        "id".into(),
        "group_id".into(),
        "status".into(),
        "amount".into(),
        "title".into(),
        "body".into(),
        "payload".into(),
        "binary_token".into(),
        "created_at".into(),
        "updated_at".into(),
    ];
    let column_types = vec![
        "bigint unsigned".into(),
        "int unsigned".into(),
        "enum".into(),
        "decimal".into(),
        "varchar".into(),
        "text".into(),
        "json".into(),
        "binary".into(),
        "datetime".into(),
        "timestamp".into(),
    ];
    let date_time = chrono::DateTime::<chrono::Utc>::from_timestamp(1_700_000_000, 123_000)
        .expect("fixed test timestamp is valid");
    let result = Arc::new(QueryResult {
        columns,
        column_types,
        rows: (0..100)
            .map(|index| Row {
                values: vec![
                    Value::Int(index + 1),
                    Value::Int(index % 1000),
                    Value::Text(["active", "pending", "archived"][index as usize % 3].into()),
                    Value::Text(format!("{}.1234", index * 1234)),
                    Value::Text(format!("record-{index:06}")),
                    Value::Text(format!("Bulk row {index}：中文、日本語、한국어、emoji 🙂")),
                    Value::Json(serde_json::json!({
                        "id": index + 1,
                        "group": index % 1000,
                        "flags": [index % 2 == 0, index % 5 == 0],
                        "nullable": format!("value-{index}"),
                    })),
                    Value::Bytes(vec![index as u8; 16]),
                    Value::DateTime(date_time),
                    Value::DateTime(date_time),
                ],
            })
            .collect(),
        affected_rows: 0,
        elapsed_ms: 9,
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
        panel.pagination = Some(ResultPagination {
            page: 0,
            page_size: 100,
            has_more: true,
            total: TotalRows::Known(100_000),
        });
        panel.display_view_cache = Some(DisplayViewCache {
            key: display_view_key,
            view: display_view,
        });
        panel
    });
    cx.simulate_resize(size(px(1886.0), px(880.0)));
    panel.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    assert!(
        cx.debug_bounds("result-header-col-9").is_some(),
        "mixed-value table should render all ten headers"
    );
    assert!(
        cx.debug_bounds("result-row-0").is_some(),
        "mixed-value table should render its first visible row"
    );
}
