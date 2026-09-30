use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, WindowExt as _, button::ButtonVariants as _,
    h_flex, input::InputState, notification::Notification,
};
use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, Styled, div, px,
};

use super::QueryTab;
use crate::views::result_panel::MAX_INSERT_COLUMNS;
use ramag_domain::entities::MAX_SQL_QUERY_BYTES;

/// Render the insert action from existing edit guards; asynchronously load bounded columns
/// before creating a local draft. Loading errors are shown without creating an insert draft.
pub(super) fn render_insert_button(
    plan_visible: bool,
    insert_reason: Option<&'static str>,
    has_pending_insert: bool,
    pending_cell_edit_count: usize,
    cx: &mut Context<QueryTab>,
) -> impl IntoElement {
    let can_insert = !plan_visible
        && insert_reason.is_none()
        && !has_pending_insert
        && pending_cell_edit_count == 0;
    let insert_tip: gpui_kit::SharedString = if let Some(reason) = insert_reason {
        reason.into()
    } else if has_pending_insert {
        "请先处理草稿".into()
    } else if pending_cell_edit_count > 0 {
        "请先提交或撤销未提交单元格修改".into()
    } else {
        "新增行".into()
    };
    ramag_ui::clickable_button("toolbar-insert")
        .debug_selector(|| "sql-result-insert".into())
        .ghost()
        .small()
        .icon(gpui_kit::component::IconName::Plus)
        .tooltip(insert_tip)
        .disabled(!can_insert)
        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
            let Some(conn) = this.connection.clone() else {
                return;
            };
            let Some((schema, table)) = this.pinned_target.clone() else {
                return;
            };
            let svc = this.service.clone();
            let panel = this.active_result();
            let handle = window.window_handle();
            cx.spawn(async move |_, cx| {
                let cols = svc.list_columns(&conn, &schema, &table).await;
                let _ = cx.update_window(handle, |_, window, app| match cols {
                    Ok(cols) => {
                        if cols.len() > MAX_INSERT_COLUMNS {
                            ramag_ui::push_responsive_notification(
                                window,
                                Notification::warning(format!(
                                    "该表有 {} 列，超过行内新增的 {} 列上限；请使用 INSERT SQL",
                                    cols.len(),
                                    MAX_INSERT_COLUMNS
                                ))
                                .autohide(true),
                                app,
                            );
                            return;
                        }
                        let inputs: Vec<Entity<InputState>> = cols
                            .iter()
                            .map(|col| {
                                let placeholder = format!(
                                    "{} · {}",
                                    col.data_type.raw_type,
                                    if col.nullable { "可空" } else { "必填" }
                                );
                                app.new(|cx_inner| {
                                    InputState::new(window, cx_inner)
                                        .validate(|value, _| value.len() <= MAX_SQL_QUERY_BYTES)
                                        .placeholder(placeholder)
                                })
                            })
                            .collect();
                        let first_input = inputs.first().cloned();
                        panel.update(app, |r, cx| {
                            r.start_insert(cols, inputs, cx);
                        });
                        if let Some(input) = first_input {
                            input.update(app, |state, cx_inner| {
                                state.focus(window, cx_inner);
                            });
                        }
                    }
                    Err(e) => {
                        ramag_ui::push_responsive_notification(
                            window,
                            Notification::error(format!("拉取表结构失败：{e}")).autohide(true),
                            app,
                        );
                    }
                });
            })
            .detach();
        }))
}

pub(super) fn render_delete_button(
    plan_visible: bool,
    has_selected: bool,
    modify_reason: Option<&'static str>,
    cx: &mut Context<QueryTab>,
) -> impl IntoElement {
    let delete_tip: gpui_kit::SharedString = match (modify_reason, has_selected) {
        (Some(reason), _) => reason.into(),
        (None, false) => "请先选择数据".into(),
        (None, true) => "删除选中行".into(),
    };
    ramag_ui::clickable_button("toolbar-delete")
        .debug_selector(|| "sql-result-delete".into())
        .ghost()
        .small()
        .icon(gpui_kit::component::IconName::Minus)
        .tooltip(delete_tip)
        .disabled(plan_visible || !has_selected || modify_reason.is_some())
        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
            let panel_ref = this.active_result().read(cx);
            let multi = panel_ref.delete_preview_multi(cx);
            let single = if multi.is_none() {
                panel_ref.delete_preview(cx)
            } else {
                None
            };
            let _ = panel_ref;
            if let Some((indices, _)) = &multi
                && !this.active_result().update(cx, |panel, cx| {
                    panel.guard_batch_delete_count(indices.len(), cx)
                })
            {
                return;
            }
            let result = this.active_result();
            let (title, preview, on_ok_indices, on_ok_single): (
                &'static str,
                String,
                Option<Vec<usize>>,
                Option<usize>,
            ) = match (multi, single) {
                (Some((ids, summary)), _) => ("删除选中行？", summary, Some(ids), None),
                (None, Some((ri, p))) => ("删除此行？", format!("将删除：{p}"), None, Some(ri)),
                _ => return,
            };
            window.open_dialog(cx, move |dialog, window, _| {
                let result_btn = result.clone();
                let preview_for_content = preview.clone();
                let on_ok_indices = on_ok_indices.clone();
                let on_ok_single = on_ok_single;
                let cancel = ramag_ui::clickable_button("del-row-cancel")
                    .ghost()
                    .small()
                    .label("取消")
                    .on_click(|_: &ClickEvent, window, app| {
                        window.close_dialog(app);
                    });
                let ok = ramag_ui::clickable_button("del-row-ok")
                    .danger()
                    .small()
                    .label("删除")
                    .on_click({
                        let result = result_btn.clone();
                        let indices = on_ok_indices.clone();
                        let single = on_ok_single;
                        move |_: &ClickEvent, window, app| {
                            let started = result.update(app, |r, cx| {
                                if let Some(ids) = indices.clone() {
                                    r.execute_delete_rows_async(ids, cx)
                                } else if let Some(ri) = single {
                                    r.execute_delete_row_async(ri, cx)
                                } else {
                                    false
                                }
                            });
                            if started {
                                window.close_dialog(app);
                            }
                        }
                    });
                dialog
                    .title(ramag_ui::closable_dialog_title(
                        "delete-row-close",
                        title,
                        |_, _| {},
                    ))
                    .close_button(false)
                    .width(ramag_ui::responsive_dialog_width(window, 520.0))
                    .max_h(ramag_ui::responsive_dialog_max_height(window))
                    .margin_top(ramag_ui::responsive_dialog_top(window))
                    .content(move |c, _, cx| {
                        let muted_fg = cx.theme().muted_foreground;
                        let p = preview_for_content.clone();
                        c.child(div().text_sm().text_color(muted_fg).child(p))
                    })
                    .footer(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_end()
                            .gap(px(8.0))
                            .child(cancel)
                            .child(ok),
                    )
            });
        }))
}
