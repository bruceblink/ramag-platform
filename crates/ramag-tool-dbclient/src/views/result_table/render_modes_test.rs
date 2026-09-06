#![allow(clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use gpui::{TestAppContext, px, size};
use ramag_domain::entities::{QueryResult, Row, Value};

use super::{DisplayViewCache, DisplayViewCacheKey, build_display_view};
use crate::views::result_panel::{
    ResultPagination, ResultPanel, ResultState, ResultViewMode, TotalRows,
};

/// Alternate result modes keep their status and pagination controls inside narrow windows.
#[gpui::test]
fn alternate_result_status_keeps_paging_controls_visible_in_small_window(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    cx.set_global(ramag_ui::DatabaseResultSettingsGlobal::new(
        ramag_ui::DatabaseResultSettings {
            show_horizontal_scrollbar: true,
            display_binary_16_as_uuid: true,
        },
    ));
    let result = Arc::new(QueryResult {
        columns: vec!["id".into(), "description_".repeat(40)],
        column_types: vec!["BIGINT".into(), "TEXT".into()],
        rows: vec![Row {
            values: vec![Value::Int(1), Value::Text("状态信息".repeat(120))],
        }],
        affected_rows: 0,
        elapsed_ms: 123,
        warnings: Vec::new(),
        truncated: false,
    });
    let display_view = build_display_view(&result, None, "", "");
    let display_view_key = DisplayViewCacheKey {
        result_identity: Arc::as_ptr(&result) as usize,
        result_revision: 0,
        sort_by: None,
        column_filter: String::new(),
        row_filter: super::RowFilter::Text(String::new()),
        display_binary_16_as_uuid: true,
    };
    let (panel, cx) = cx.add_window_view(|window, cx| {
        let mut panel = ResultPanel::new(window, cx);
        panel.state = ResultState::Ok(result.clone());
        panel.selected_cell = Some((0, 1));
        panel.pagination = Some(ResultPagination {
            page: 0,
            page_size: 100,
            has_more: true,
            total: TotalRows::Known(10_000),
        });
        panel.display_view_cache = Some(DisplayViewCache {
            key: display_view_key,
            view: display_view,
        });
        panel
    });
    panel.update(cx, |panel, cx| {
        panel.set_view_mode(ResultViewMode::Tree, cx)
    });
    cx.run_until_parked();

    for width in [280.0, 360.0, 1024.0] {
        cx.simulate_resize(size(px(width), px(420.0)));
        panel.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();

        let status_bar = cx
            .debug_bounds("result-alternate-status")
            .expect("替代结果视图状态栏应渲染");
        let status_context = cx
            .debug_bounds("result-alternate-status-context")
            .expect("替代结果视图状态摘要应渲染");
        let next_page = cx
            .debug_bounds("result-alternate-page-next")
            .expect("替代结果视图下一页按钮应渲染");
        let tree = cx
            .debug_bounds("result-tree-scroll")
            .expect("树形结果视图应渲染");
        assert!(status_context.size.width >= px(160.0));
        assert!(
            status_context.right() <= status_bar.right()
                && status_context.bottom() <= status_bar.bottom(),
            "替代结果状态摘要不能越出状态栏"
        );
        assert!(status_bar.right() <= px(width));
        assert!(tree.right() <= px(width));
        assert!(next_page.right() <= status_bar.right());
    }
}
