#![allow(clippy::expect_used, clippy::panic)]

use super::{DisplayViewCache, DisplayViewCacheKey, RowFilter, build_display_view};
use crate::views::result_panel::{ResultPanel, ResultState};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, Modifiers, ParentElement as _, Render,
    Styled as _, TestAppContext, Window, div, px, size,
};
use ramag_domain::entities::{QueryResult, Row, Value};
use std::sync::Arc;

struct ResultPanelTestHost {
    panel: Entity<ResultPanel>,
}

impl Render for ResultPanelTestHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = gpui_kit::component::Root::render_dialog_layer(window, cx);
        div()
            .relative()
            .size_full()
            .child(self.panel.clone())
            .children(dialog_layer)
    }
}

/// Verifies that the selected-edit action stays visible and removes only its local draft.
#[gpui_kit::test]
fn selected_pending_edit_reverts_without_touching_other_drafts(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let result = Arc::new(QueryResult {
        columns: vec!["id".into(), "name".into()],
        column_types: vec!["BIGINT".into(), "TEXT".into()],
        rows: vec![Row {
            values: vec![Value::Int(1), Value::Text("original".into())],
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
    let mut panel_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let panel = cx.new(|cx| {
            let mut panel = ResultPanel::new(window, cx);
            panel.state = ResultState::Ok(result.clone());
            panel.display_view_cache = Some(DisplayViewCache {
                key: display_view_key,
                view: display_view,
            });
            panel.seed_pending_cell_edit_for_test_at(0, 0);
            panel.seed_pending_cell_edit_for_test_at(0, 1);
            panel.selected_cell = Some((0, 0));
            panel
        });
        panel_entity = Some(panel.clone());
        let host = cx.new(|_| ResultPanelTestHost { panel });
        gpui_kit::component::Root::new(host, window, cx)
    });
    let panel = panel_entity.expect("result panel should be initialized");

    for width in [280.0, 360.0, 1024.0] {
        cx.simulate_resize(size(px(width), px(420.0)));
        panel.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        let status = cx
            .debug_bounds("result-status-bar")
            .expect("结果状态栏应渲染");
        let actions = cx
            .debug_bounds("result-mutation-actions")
            .expect("待提交操作区应渲染");
        let selected = cx
            .debug_bounds("cell-edit-revert-selected-bar")
            .expect("撤销选中按钮应渲染");
        assert!(
            actions.origin.x >= status.origin.x
                && actions.right() <= status.right()
                && actions.origin.y >= status.origin.y
                && actions.bottom() <= status.bottom()
        );
        assert!(
            selected.origin.x >= actions.origin.x
                && selected.right() <= actions.right()
                && selected.origin.y >= actions.origin.y
                && selected.bottom() <= actions.bottom()
        );
    }

    let submit = cx
        .debug_bounds("cell-edits-submit-bar")
        .expect("提交修改按钮应渲染");
    cx.simulate_click(submit.center(), Modifiers::default());
    cx.run_until_parked();
    assert!(cx.debug_bounds("ramag-confirm-ok").is_some());
    let cancel = cx
        .debug_bounds("ramag-confirm-cancel")
        .expect("确认框取消按钮应渲染");
    cx.simulate_click(cancel.center(), Modifiers::default());
    cx.run_until_parked();
    panel.read_with(cx, |panel, _| {
        assert_eq!(panel.pending_cell_edit_count(), 2);
    });

    panel.read_with(cx, |panel, _| {
        assert!(panel.has_selected_pending_cell_edit())
    });
    panel.update(cx, |panel, cx| panel.clear_selected_pending_cell_edit(cx));
    {
        panel.read_with(cx, |panel, _| {
            assert_eq!(panel.pending_cell_edit_count(), 1);
            assert!(!panel.has_pending_cell_edit(0, 0));
            assert!(panel.has_pending_cell_edit(0, 1));
        });
    }
    panel.update(cx, |panel, _| panel.set_selected_cell(Some((0, 1))));
    panel.read_with(cx, |panel, _| {
        assert!(panel.has_selected_pending_cell_edit())
    });
}
