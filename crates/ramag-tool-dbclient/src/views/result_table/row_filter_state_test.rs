use crate::views::result_panel::{ResultPanel, RowSearchMode};
use gpui_kit::TestAppContext;

#[gpui_kit::test]
fn clearing_row_filter_resets_server_search_state(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (panel, cx) = cx.add_window_view(|window, cx| {
        let panel = ResultPanel::new(window, cx);
        panel.row_filter_entity().update(cx, |state, cx| {
            state.set_value("status = 'active'", window, cx)
        });
        panel
    });

    panel.update_in(cx, |panel, window, cx| panel.clear_row_filter(window, cx));
    panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.row_search_mode(), RowSearchMode::Normal);
        assert!(panel.row_filter_entity().read(cx).value().is_empty());
    });
}
