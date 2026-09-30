//! Result actions must wrap as one row, including disabled and running controls.

use super::*;
use std::sync::atomic::AtomicU64;

/// Check real action bounds at full and sidebar-reduced widths without database requests.
#[gpui_kit::test]
fn result_actions_stay_together_across_widths_and_query_states(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let service = Arc::new(ConnectionService::new(
        HashMap::new(),
        Arc::new(NoopStorage),
    ));
    let (tab, cx) = cx.add_window_view(|window, cx| {
        QueryTab::new(
            service,
            "Toolbar layout",
            None,
            SchemaCache::new_shared(),
            ramag_ui::ResultMemoryBudget::default(),
            window,
            cx,
        )
    });

    for mode in [ramag_ui::Mode::Dark, ramag_ui::Mode::Light] {
        for text_size in [
            ramag_ui::InterfaceTextSize::Standard,
            ramag_ui::InterfaceTextSize::Large,
        ] {
            cx.update(|_, app| {
                ramag_ui::set_system_settings(
                    ramag_ui::SystemSettings {
                        text_size,
                        ..Default::default()
                    },
                    app,
                );
                ramag_ui::apply_theme(mode, app);
            });
            for state in 0..4 {
                tab.update(cx, |tab, cx| {
                    tab.running = state == 3;
                    tab.cancel_handle = tab.running.then(|| Arc::new(AtomicU64::new(0)));
                    tab.transaction_busy = state == 2;
                    tab.transaction = (state == 2).then(|| TransactionSession {
                        id: TransactionId::default(),
                        dirty: true,
                        savepoints: Vec::new(),
                        next_savepoint: 1,
                    });
                    tab.result.update(cx, |panel, cx| {
                        let result = QueryResult {
                            columns: vec!["id".into()],
                            column_types: vec!["BIGINT".into()],
                            rows: vec![Row {
                                values: vec![Value::Int(1)],
                            }],
                            affected_rows: 0,
                            elapsed_ms: 1,
                            warnings: Vec::new(),
                            truncated: false,
                        };
                        panel.set_state(
                            match state {
                                0 => ResultState::Empty,
                                3 => ResultState::Running,
                                _ => ResultState::Ok(Arc::new(result)),
                            },
                            cx,
                        );
                        panel.exporting = state == 2;
                        if let ResultState::Ok(result) = panel.state().clone() {
                            crate::views::result_table::prepare_display_view_for_test(
                                panel, &result, cx,
                            );
                        }
                    });
                    cx.notify();
                });
                for width in [360.0, 560.0, 720.0, 1024.0, 1440.0] {
                    cx.simulate_resize(size(px(width), px(640.0)));
                    tab.update(cx, |_, cx| cx.notify());
                    cx.run_until_parked();
                    let toolbar = cx.debug_bounds("sql-result-toolbar").unwrap();
                    let group = cx.debug_bounds("sql-result-action-group").unwrap();
                    assert!(group.size.width > px(0.0));
                    assert!(group.origin.x >= toolbar.origin.x - px(1.0));
                    assert!(group.origin.y >= toolbar.origin.y - px(1.0));
                    assert!(group.right() <= toolbar.right() + px(1.0));
                    assert!(group.bottom() <= toolbar.bottom() + px(1.0));
                    let execution_selector = if state == 3 {
                        "sql-cancel-query"
                    } else {
                        "sql-run-query"
                    };
                    for selector in [
                        "sql-result-insert",
                        "sql-result-delete",
                        "sql-result-import",
                        "export-btn",
                        execution_selector,
                    ] {
                        let action = cx.debug_bounds(selector).unwrap();
                        assert!(
                            action.size.width > px(0.0),
                            "{selector} must remain visible"
                        );
                        assert!(action.origin.x >= group.origin.x - px(1.0));
                        assert!(action.right() <= group.right() + px(1.0));
                        assert!(action.origin.y >= group.origin.y - px(1.0));
                        assert!(action.bottom() <= group.bottom() + px(1.0));
                        let center = action.origin.y + action.size.height / 2.0;
                        let group_center = group.origin.y + group.size.height / 2.0;
                        assert!(
                            (f32::from(center) - f32::from(group_center)).abs() <= 1.0,
                            "{selector} must share one row at width={width}, state={state}"
                        );
                    }
                    if width == 360.0 {
                        let filters = cx.debug_bounds("sql-result-filter-group").unwrap();
                        assert!(group.origin.y >= filters.bottom());
                    }
                }
            }
        }
    }
}
