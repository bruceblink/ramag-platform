use std::sync::Arc;

use super::{DisplayViewCache, DisplayViewCacheKey, RowFilter, build_display_view};
use crate::views::result_panel::{ResultPanel, ResultState};
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext as _, TestAppContext, px};
use ramag_domain::entities::{Column, ColumnKind, ColumnType, QueryResult, Row, Value};

/// 标准和紧凑档都应保留查询结果文字、复选框及新增输入框的完整行内边界。
#[gpui_kit::test]
fn result_rows_follow_system_content_density(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let result = Arc::new(QueryResult {
        columns: vec!["id".into(), "name".into()],
        column_types: vec!["INTEGER".into(), "TEXT".into()],
        rows: vec![Row {
            values: vec![Value::Int(1), Value::Text("one".into())],
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
    let mut pending_input_entity = None;
    let (panel, cx) = cx.add_window_view(|window, cx| {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("TEXT · 可空"));
        pending_input_entity = Some(input.clone());
        let mut panel = ResultPanel::new(window, cx);
        panel.state = ResultState::Ok(result);
        panel.display_view_cache = Some(DisplayViewCache {
            key: display_view_key,
            view: display_view,
        });
        panel.start_insert(
            vec![Column {
                name: "name".into(),
                data_type: ColumnType {
                    kind: ColumnKind::Text,
                    raw_type: "TEXT".into(),
                },
                nullable: true,
                default_value: None,
                is_primary_key: false,
                comment: None,
                ordinal_position: Some(2),
                is_auto_increment: false,
                generation_expression: None,
                generated_storage: None,
                identity_generation: None,
            }],
            vec![input],
            cx,
        );
        panel
    });
    let pending_input_state = pending_input_entity.expect("新增草稿输入状态应创建");

    for density in ramag_ui::ContentDensity::ALL {
        let text_size = if density.is_compact() {
            ramag_ui::InterfaceTextSize::Large
        } else {
            ramag_ui::InterfaceTextSize::Standard
        };
        cx.update(|_, app| {
            ramag_ui::set_system_settings(
                ramag_ui::SystemSettings {
                    content_density: density,
                    text_size,
                    ..Default::default()
                },
                app,
            );
        });
        panel.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();

        let row = cx.debug_bounds("result-row-0").expect("查询结果首行应渲染");
        assert_eq!(row.size.height, px(density.row_height(32.0, 26.0)));
        let header = cx
            .debug_bounds("result-header")
            .expect("查询结果表头应渲染");
        assert_eq!(header.size.height, px(34.0));

        for (cell_selector, value_selector) in [
            ("result-cell-0-0", "result-cell-value-0-0"),
            ("result-cell-0-1", "result-cell-value-0-1"),
        ] {
            let cell = cx
                .debug_bounds(cell_selector)
                .expect("真实数据单元格应渲染");
            let value = cx
                .debug_bounds(value_selector)
                .expect("真实单元格文字应渲染");
            assert!(
                value.origin.x >= cell.origin.x
                    && value.right() <= cell.right()
                    && value.origin.y >= row.origin.y
                    && value.bottom() <= row.bottom(),
                "字号 {text_size:?} 时单元格文字必须留在行内：row={row:?}, cell={cell:?}, value={value:?}"
            );
        }

        let checkbox = cx
            .debug_bounds("result-row-checkbox-0")
            .expect("真实行选择复选框应渲染");
        assert!(
            checkbox.origin.y >= row.origin.y && checkbox.bottom() <= row.bottom(),
            "字号 {text_size:?} 时行选择复选框必须留在行内：row={row:?}, checkbox={checkbox:?}"
        );

        let pending_row = cx
            .debug_bounds("result-row-pending")
            .expect("新增草稿行应渲染");
        let pending_input_bounds = cx
            .debug_bounds("result-pending-input-1")
            .expect("新增草稿的真实输入框应渲染");
        assert_eq!(pending_row.size.height, px(density.row_height(32.0, 26.0)));
        assert!(
            pending_input_bounds.origin.y >= pending_row.origin.y
                && pending_input_bounds.bottom() <= pending_row.bottom(),
            "字号 {text_size:?} 时新增输入框必须留在草稿行内：row={pending_row:?}, input={pending_input_bounds:?}"
        );

        if density.is_compact() {
            // 点击真实输入框并输入内容，确认紧凑行没有覆盖或禁用编辑器。
            cx.simulate_click(
                pending_input_bounds.center(),
                gpui_kit::Modifiers::default(),
            );
            cx.run_until_parked();
            cx.simulate_keystrokes("x");
            cx.run_until_parked();
            assert_eq!(
                pending_input_state.read_with(cx, |state, _| state.value().to_string()),
                "x"
            );
        }
    }
}
