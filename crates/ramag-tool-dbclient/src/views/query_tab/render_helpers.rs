use gpui_kit::component::{
    Disableable as _, IconName, Sizable as _, button::ButtonVariants as _, h_flex,
    input::InputState,
};
use gpui_kit::{ClickEvent, Entity, IntoElement, ParentElement, Styled, div, prelude::*, px};
use ramag_ui::PointerDropdownMenu as _;

use super::{QueryTab, QueryTabEvent};
use crate::views::result_panel::{ResultPanel, RowSearchConversionStatus, RowSearchMode, SortDir};

pub(super) struct TransactionSavepointState {
    pub(super) transaction_busy: bool,
    pub(super) running: bool,
    pub(super) dml_busy: bool,
    pub(super) pending_cell_edits: bool,
    pub(super) savepoint_count: usize,
    pub(super) latest_savepoint: Option<String>,
    pub(super) max_savepoints: usize,
}

pub(super) struct TransactionToolbarState {
    pub(super) accent: gpui_kit::Hsla,
    pub(super) running: bool,
    pub(super) dml_busy: bool,
    pub(super) pending_cell_edits: bool,
    pub(super) compact_toolbar: bool,
    pub(super) ddl_target: Option<(String, String)>,
    pub(super) ddl_is_view: bool,
    pub(super) plan_visible: bool,
}

/// Builds the result-grid ORDER BY menu from column metadata without accepting raw SQL text.
/// The selected column index and direction are sent back through ResultPanel's existing sort path.
pub(super) fn order_by_menu(
    result: Entity<ResultPanel>,
    current_sort: Option<(usize, SortDir)>,
    columns: Vec<String>,
    accent: gpui_kit::Hsla,
) -> impl IntoElement {
    let order_by_label = current_sort
        .and_then(|(index, direction)| columns.get(index).map(|name| (name, direction)))
        .map_or_else(
            || "ORDER BY".to_string(),
            |(name, direction)| {
                format!(
                    "ORDER BY {name} {}",
                    match direction {
                        SortDir::Asc => "ASC",
                        SortDir::Desc => "DESC",
                    }
                )
            },
        );
    let selected_sort = current_sort;
    let mut control = ramag_ui::clickable_button("sql-order-by")
        .debug_selector(|| "sql-order-by".into())
        .text()
        .small()
        .child(div().flex_none().text_color(accent).child(order_by_label))
        .dropdown_caret(true)
        .tooltip("按结果列选择升序或降序");
    if columns.is_empty() {
        control = control.disabled(true);
    }
    control.pointer_dropdown_menu(move |mut menu, _, _| {
        for (index, name) in columns.iter().enumerate() {
            for direction in [SortDir::Asc, SortDir::Desc] {
                let result = result.clone();
                let selected = selected_sort == Some((index, direction));
                let label = format!(
                    "{name} {}",
                    match direction {
                        SortDir::Asc => "ASC",
                        SortDir::Desc => "DESC",
                    }
                );
                menu = menu.item(ramag_ui::menu_item(label).checked(selected).on_click(
                    move |_, _, app| {
                        result.update(app, |panel, cx| {
                            panel.set_sort_by(Some((index, direction)), cx);
                        });
                    },
                ));
            }
        }
        menu
    })
}

/// Builds the Tx mode menu while routing every mutating action through QueryTab's transaction API.
/// Auto-commit remains a non-action item when a manual transaction is open, preventing an implicit commit.
pub(super) fn transaction_mode_menu(
    query_tab: Entity<QueryTab>,
    tab: &QueryTab,
    accent: gpui_kit::Hsla,
    running: bool,
    dml_busy: bool,
    pending_cell_edits: bool,
) -> impl IntoElement {
    let label = tab.transaction_label();
    let has_connection = tab.connection.is_some();
    let supports_transactions = tab
        .connection
        .as_ref()
        .is_some_and(|connection| connection.driver.supports_transactions());
    let transaction_active = tab.transaction.is_some();
    let transaction_busy = tab.transaction_busy;
    let blocked = transaction_busy || running || dml_busy || pending_cell_edits;
    let can_begin = has_connection && supports_transactions && !transaction_active && !blocked;
    let can_finish = transaction_active && !blocked;
    let begin_tab = query_tab.clone();
    let commit_tab = query_tab.clone();
    let rollback_tab = query_tab;
    ramag_ui::clickable_button("sql-transaction-mode")
        .debug_selector(|| "sql-transaction-mode".into())
        .text()
        .small()
        .child(div().flex_none().text_color(accent).child(label))
        .dropdown_caret(true)
        .tooltip("查看事务模式并执行提交或回滚")
        .disabled(!has_connection && !transaction_active)
        .pointer_dropdown_menu(move |mut menu, _, _| {
            menu = menu.item(
                ramag_ui::menu_item_with_disabled(
                    "自动提交",
                    transaction_active || transaction_busy,
                )
                .checked(!transaction_active && !transaction_busy),
            );
            let begin_tab_for_item = begin_tab.clone();
            menu = menu.item(
                ramag_ui::menu_item_with_disabled(
                    "手动事务",
                    !has_connection || !supports_transactions || transaction_active || blocked,
                )
                .checked(transaction_active)
                .on_click(move |_: &ClickEvent, _, app| {
                    if can_begin {
                        begin_tab_for_item.update(app, |tab, cx| tab.begin_transaction(cx));
                    }
                }),
            );
            menu = menu.separator();
            let commit_tab_for_item = commit_tab.clone();
            menu = menu.item(
                ramag_ui::menu_item_with_disabled("提交事务", !can_finish).on_click(
                    move |_: &ClickEvent, _, app| {
                        if can_finish {
                            commit_tab_for_item
                                .update(app, |tab, cx| tab.finish_transaction(true, cx));
                        }
                    },
                ),
            );
            let rollback_tab_for_item = rollback_tab.clone();
            menu.item({
                ramag_ui::menu_item_with_disabled("回滚事务", !can_finish).on_click(
                    move |_: &ClickEvent, _, app| {
                        if can_finish {
                            rollback_tab_for_item
                                .update(app, |tab, cx| tab.finish_transaction(false, cx));
                        }
                    },
                )
            })
        })
}

/// Builds the toolbar entry for a pinned table's read-only DDL preview.
/// The preview is opened by the owning connection session so copy and refresh actions share
/// the existing modal lifecycle; this button never executes or edits the returned SQL.
pub(super) fn table_ddl_button(
    query_tab: Entity<QueryTab>,
    target: Option<(String, String)>,
    is_view: bool,
    plan_visible: bool,
    running: bool,
    dml_busy: bool,
    pending_cell_edits: bool,
) -> impl IntoElement {
    let disabled = plan_visible || target.is_none() || running || dml_busy || pending_cell_edits;
    let tooltip = if plan_visible {
        "执行计划只读，无法定位 DDL"
    } else if target.is_none() {
        "请先从对象树打开单表"
    } else if pending_cell_edits {
        "请先提交或撤销未提交单元格修改"
    } else if running || dml_busy {
        "查询或写操作执行中，请稍候"
    } else {
        "打开当前表的只读 DDL 预览"
    };
    let target_for_click = target.clone();
    ramag_ui::clickable_button("sql-ddl")
        .debug_selector(|| "sql-ddl".into())
        .ghost()
        .small()
        .label("DDL")
        .tooltip(tooltip)
        .disabled(disabled)
        .on_click(move |_, _, app| {
            let Some((schema, table)) = target_for_click.clone() else {
                return;
            };
            query_tab.update(app, |_, cx| {
                cx.emit(QueryTabEvent::ShowTableDdl {
                    schema,
                    table,
                    is_view,
                });
            });
        })
}

/// Groups the transaction mode, transaction actions, and DDL entry as one responsive toolbar unit.
/// Keeping this layout together prevents the DDL button from escaping the same wrap boundary.
pub(super) fn transaction_toolbar_group(
    query_tab: Entity<QueryTab>,
    tab: &QueryTab,
    transaction_controls: impl IntoElement,
    state: TransactionToolbarState,
) -> impl IntoElement {
    h_flex()
        .id("sql-transaction-group")
        .debug_selector(|| "sql-transaction-group".into())
        .min_w_0()
        .flex_wrap()
        .when(state.compact_toolbar, |this| this.w_full())
        .when(!state.compact_toolbar, |this| this.flex_1())
        .items_center()
        .gap_1()
        .child(transaction_mode_menu(
            query_tab.clone(),
            tab,
            state.accent,
            state.running,
            state.dml_busy,
            state.pending_cell_edits,
        ))
        .child(transaction_controls)
        .child(table_ddl_button(
            query_tab,
            state.ddl_target,
            state.ddl_is_view,
            state.plan_visible,
            state.running,
            state.dml_busy,
            state.pending_cell_edits,
        ))
}

/// Renders savepoint actions and disables them while related work is active.
pub(super) fn transaction_savepoint_controls(
    query_tab: Entity<QueryTab>,
    state: TransactionSavepointState,
    muted: gpui_kit::Hsla,
) -> impl IntoElement {
    let has_latest = state.latest_savepoint.is_some();
    let create_tab = query_tab.clone();
    let rollback_tab = query_tab.clone();
    let release_tab = query_tab.clone();
    h_flex()
        .id("sql-transaction-savepoints")
        .debug_selector(|| "sql-transaction-savepoints".into())
        .flex_1()
        .min_w_0()
        .flex_wrap()
        .items_center()
        .gap_1()
        .child(
            ramag_ui::clickable_button("transaction-savepoint-create")
                .debug_selector(|| "transaction-savepoint-create".into())
                .ghost()
                .small()
                .label("保存点")
                .tooltip("创建保存点")
                .disabled(
                    state.transaction_busy
                        || state.running
                        || state.dml_busy
                        || state.pending_cell_edits
                        || state.savepoint_count >= state.max_savepoints,
                )
                .on_click(move |_: &ClickEvent, _, app| {
                    create_tab.update(app, |tab, cx| tab.create_savepoint(cx));
                }),
        )
        .child(
            ramag_ui::clickable_button("transaction-savepoint-rollback")
                .debug_selector(|| "transaction-savepoint-rollback".into())
                .ghost()
                .small()
                .label("回滚最近")
                .tooltip("回滚到最近的保存点")
                .disabled(
                    state.transaction_busy
                        || state.running
                        || state.dml_busy
                        || state.pending_cell_edits
                        || !has_latest,
                )
                .on_click(move |_: &ClickEvent, _, app| {
                    rollback_tab.update(app, |tab, cx| tab.rollback_to_latest_savepoint(cx));
                }),
        )
        .child(
            ramag_ui::clickable_button("transaction-savepoint-release")
                .debug_selector(|| "transaction-savepoint-release".into())
                .ghost()
                .small()
                .label("释放最近")
                .tooltip("释放最近的保存点")
                .disabled(
                    state.transaction_busy
                        || state.running
                        || state.dml_busy
                        || state.pending_cell_edits
                        || !has_latest,
                )
                .on_click(move |_: &ClickEvent, _, app| {
                    release_tab.update(app, |tab, cx| tab.release_latest_savepoint(cx));
                }),
        )
        .when_some(state.latest_savepoint, |controls, name| {
            controls.child(
                div()
                    .flex_none()
                    .text_xs()
                    .text_color(muted)
                    .child(format!("最近 {name}")),
            )
        })
}

pub(super) fn result_view_tabs(
    query_tab: Entity<QueryTab>,
    plan_visible: bool,
    plan_available: bool,
    border: gpui_kit::Hsla,
    secondary_bg: gpui_kit::Hsla,
) -> impl IntoElement {
    h_flex()
        .id("sql-result-view-tabs")
        .debug_selector(|| "sql-result-view-tabs".into())
        .w_full()
        .flex_none()
        .items_center()
        .gap_1()
        .px_2()
        .py(px(3.0))
        .border_b_1()
        .border_color(border)
        .bg(secondary_bg)
        .child(
            ramag_ui::clickable_button("sql-data-result-tab")
                .ghost()
                .small()
                .label("数据结果")
                .when(!plan_visible, |button| button.primary())
                .on_click({
                    let query_tab = query_tab.clone();
                    move |_, _, app| {
                        query_tab.update(app, |tab, cx| {
                            tab.set_plan_visible(false, cx);
                        });
                    }
                }),
        )
        .child(
            ramag_ui::clickable_button("sql-plan-result-tab")
                .ghost()
                .small()
                .label("执行计划")
                .tooltip(if plan_available {
                    "查看最近一次执行计划"
                } else {
                    "先点击工具栏中的执行计划"
                })
                .disabled(!plan_available)
                .when(plan_visible, |button| button.primary())
                .on_click({
                    move |_, _, app| {
                        query_tab.update(app, |tab, cx| {
                            tab.set_plan_visible(true, cx);
                        });
                    }
                }),
        )
        .child(div().flex_1())
}

pub(super) fn row_filter_prefix(
    current: RowSearchMode,
    result: Entity<ResultPanel>,
    accent: gpui_kit::Hsla,
    muted: gpui_kit::Hsla,
    id_conversion_ready: bool,
) -> gpui_kit::AnyElement {
    if id_conversion_ready {
        row_search_mode_button(current, result, accent).into_any_element()
    } else {
        div()
            .flex_none()
            .text_xs()
            .text_color(muted)
            .child("WHERE")
            .into_any_element()
    }
}

pub(super) fn row_search_mode_button(
    current: RowSearchMode,
    result: Entity<ResultPanel>,
    accent: gpui_kit::Hsla,
) -> impl IntoElement {
    let display_label = match current {
        RowSearchMode::Normal => "WHERE",
        RowSearchMode::IdToInteger => current.label(),
        RowSearchMode::IdToString => current.label(),
    };
    ramag_ui::clickable_button("sql-row-search-mode")
        .text()
        .small()
        // 文本自带显式颜色，避免 Text 按下态短暂继承主题前景色。
        .child(div().flex_none().text_color(accent).child(display_label))
        .dropdown_caret(true)
        .text_color(accent)
        .tooltip(match current {
            RowSearchMode::Normal => "WHERE：按 Enter 将条件发送到数据库执行",
            RowSearchMode::IdToInteger => "@ID -> I：将字符串转为整数，精确匹配整数单元格",
            RowSearchMode::IdToString => "@ID -> S：将非负十进制整数转为字符串，精确匹配文本单元格",
        })
        .pointer_dropdown_menu(move |mut menu, _, _| {
            for mode in RowSearchMode::ALL {
                let result = result.clone();
                menu = menu.item(
                    ramag_ui::menu_item(mode.label())
                        .checked(mode == current)
                        .on_click(move |_: &ClickEvent, _, app| {
                            result.update(app, |panel, cx| {
                                panel.set_row_search_mode(mode, cx);
                            });
                        }),
                );
            }
            menu
        })
}

pub(super) fn row_search_input_suffix(
    input: Entity<InputState>,
    status: Option<RowSearchConversionStatus>,
    accent: gpui_kit::Hsla,
    muted: gpui_kit::Hsla,
    danger: gpui_kit::Hsla,
) -> impl IntoElement {
    h_flex()
        .flex_none()
        .gap_1()
        .when_some(status, |suffix, status| {
            suffix.child(row_search_conversion_label(status, accent, muted, danger))
        })
        .child(
            ramag_ui::clickable_button("sql-row-filter-clear")
                .icon(IconName::CircleX)
                .ghost()
                .xsmall()
                .tab_stop(false)
                .text_color(muted)
                .tooltip("清除")
                .on_click(move |_, window, cx| {
                    input.update(cx, |state, cx| {
                        state.set_value("", window, cx);
                        state.focus(window, cx);
                    });
                }),
        )
}

fn row_search_conversion_label(
    status: RowSearchConversionStatus,
    accent: gpui_kit::Hsla,
    muted: gpui_kit::Hsla,
    danger: gpui_kit::Hsla,
) -> gpui_kit::AnyElement {
    let (label, color) = match status {
        RowSearchConversionStatus::Converting => ("→ 转换中…".to_string(), muted),
        RowSearchConversionStatus::Ready(output) => {
            (format!("→ {}", output.display_preview(40)), accent)
        }
        RowSearchConversionStatus::Error(_) => ("→ 转换失败".to_string(), danger),
    };

    div()
        .flex_none()
        .text_xs()
        .text_color(color)
        .child(label)
        .into_any_element()
}
