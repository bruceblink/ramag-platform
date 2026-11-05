use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::TestAppContext;
use ramag_app::ConnectionService;

use super::{NoopStorage, QueryTab};
use crate::sql_completion::SchemaCache;

/// Running a new SQL source must remove the old server-side WHERE chip before the result changes.
#[gpui_kit::test]
fn running_new_sql_clears_previous_row_filter(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let service = Arc::new(ConnectionService::new(
        HashMap::new(),
        Arc::new(NoopStorage),
    ));
    let schema_cache = SchemaCache::new_shared();
    let (tab, cx) = cx.add_window_view(|window, cx| {
        QueryTab::new(
            service,
            "查询 1",
            None,
            schema_cache,
            ramag_ui::ResultMemoryBudget::default(),
            window,
            cx,
        )
    });

    tab.update_in(cx, |tab, window, cx| {
        tab.editor.update(cx, |editor, cx| {
            editor.set_value("SELECT 1".to_string(), window, cx);
        });
        tab.result.update(cx, |result, cx| {
            result.row_filter_entity().update(cx, |state, cx| {
                state.set_value("status = 'active'", window, cx)
            });
        });
        tab.handle_run(window, cx);
    });

    tab.read_with(cx, |tab, cx| {
        assert!(
            tab.result
                .read(cx)
                .row_filter_entity()
                .read(cx)
                .value()
                .is_empty()
        );
    });
}
