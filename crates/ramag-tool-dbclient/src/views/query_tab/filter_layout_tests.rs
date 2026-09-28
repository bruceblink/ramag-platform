//! 列筛选编辑器与 WHERE 单行输入必须共享外框、字号和文本基线。

use super::*;
use gpui_kit::{AppContext as _, MouseButton};

#[gpui_kit::test]
fn result_filter_fields_share_height_and_text_alignment(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let service = Arc::new(ConnectionService::new(
        HashMap::new(),
        Arc::new(NoopStorage),
    ));
    let (tab, cx) = cx.add_window_view(|window, cx| {
        QueryTab::new(
            service,
            "筛选样式验证",
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
            for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
                cx.simulate_resize(size(px(width), px(height)));
                cx.run_until_parked();
                let column = cx
                    .debug_bounds("sql-column-filter-field")
                    .unwrap_or_else(|| panic!("列筛选框必须渲染"));
                let row = cx
                    .debug_bounds("sql-row-filter-field")
                    .unwrap_or_else(|| panic!("WHERE 筛选框必须渲染"));
                assert_eq!(column.origin.y, row.origin.y, "两个输入框顶部一致");
                assert_eq!(column.size.height, row.size.height, "两个输入框高度一致");
                assert!(row.right() <= px(width), "筛选框不能溢出窄窗口");
                tab.read_with(cx, |tab, app| {
                    let panel = tab.result.read(app);
                    let column = panel.column_filter_entity().read(app);
                    let row = panel.row_filter_entity().read(app);
                    assert_eq!(
                        column.line_height(),
                        row.line_height(),
                        "编辑器和输入框使用同样的行高"
                    );
                    let column_text = column
                        .text_bounds()
                        .unwrap_or_else(|| panic!("列输入文字应布局"));
                    let row_text = row
                        .text_bounds()
                        .unwrap_or_else(|| panic!("WHERE 文字应布局"));
                    assert!(
                        (f32::from(column_text.origin.y) - f32::from(row_text.origin.y)).abs()
                            <= 1.0,
                        "两个输入框文本顶边一致：{column_text:?} / {row_text:?}"
                    );
                });
            }
        }
    }
}

/// 清除按钮出现后，两个输入框仍对齐；点击只清除对应字段，不触发数据库操作。
#[gpui_kit::test]
fn populated_result_filters_keep_alignment_and_clear_independently(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let service = Arc::new(ConnectionService::new(
        HashMap::new(),
        Arc::new(NoopStorage),
    ));
    let mut tab_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let tab = cx.new(|cx| {
            QueryTab::new(
                service,
                "筛选交互验证",
                None,
                SchemaCache::new_shared(),
                ramag_ui::ResultMemoryBudget::default(),
                window,
                cx,
            )
        });
        tab_entity = Some(tab.clone());
        gpui_kit::component::Root::new(tab, window, cx)
    });
    let tab = tab_entity.unwrap_or_else(|| panic!("查询视图应初始化"));
    cx.simulate_resize(size(px(1024.0), px(768.0)));
    let (column, row) = tab.read_with(cx, |tab, app| {
        let panel = tab.result.read(app);
        (
            panel.column_filter_entity().clone(),
            panel.row_filter_entity().clone(),
        )
    });
    cx.update(|window, app| {
        column.update(app, |state, cx| state.set_value("id, name", window, cx));
        row.update(app, |state, cx| state.set_value("id > 10", window, cx));
    });
    cx.run_until_parked();
    let column_frame = cx
        .debug_bounds("sql-column-filter-field")
        .unwrap_or_else(|| panic!("列筛选框存在"));
    let row_frame = cx
        .debug_bounds("sql-row-filter-field")
        .unwrap_or_else(|| panic!("WHERE 框存在"));
    assert_eq!(column_frame.origin.y, row_frame.origin.y);
    assert_eq!(column_frame.size.height, row_frame.size.height);
    for selector in ["sql-column-filter-clear", "sql-row-filter-clear"] {
        let button = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("清除按钮存在：{selector}"));
        cx.simulate_mouse_move(button.center(), None, Modifiers::default());
        cx.simulate_mouse_down(button.center(), MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(button.center(), MouseButton::Left, Modifiers::default());
        cx.run_until_parked();
        if selector == "sql-column-filter-clear" {
            assert!(column.read_with(cx, |state, _| state.value().is_empty()));
            assert_eq!(
                row.read_with(cx, |state, _| state.value().to_string()),
                "id > 10"
            );
        }
    }
    assert!(row.read_with(cx, |state, _| state.value().is_empty()));
}
