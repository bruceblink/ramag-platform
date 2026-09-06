use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::*, px, uniform_list,
};
use gpui_component::{Icon, IconName, h_flex, v_flex};
use ramag_domain::entities::Value;
use ramag_ui::RestrictScrollToAxisExt as _;

use crate::views::result_panel::ResultPanel;
use crate::views::result_value::display_cell_value;

use super::{AlternateFrame, DisplayView, wrap_alternate_scroll};

const MAX_TREE_DETAIL_ENTRIES: usize = 50_000;

#[derive(Clone, Copy)]
struct TreeEntry {
    row_index: usize,
    column_index: Option<usize>,
}

struct TreeEntries {
    entries: Arc<Vec<TreeEntry>>,
    details_truncated: bool,
}

/// Renders rows with expandable fields while keeping row and column selection stable.
pub(super) fn render_tree_view(
    panel: &mut ResultPanel,
    frame: Rc<AlternateFrame>,
    view: &DisplayView,
    cx: &mut Context<ResultPanel>,
) -> AnyElement {
    let tree_entries = build_tree_entries(panel, view);
    let frame_for_rows = frame.clone();
    let entries_for_rows = tree_entries.entries.clone();
    let body = uniform_list(
        "result-tree-rows",
        entries_for_rows.len(),
        cx.processor(move |this, range: Range<usize>, _window, cx| {
            range
                .filter_map(|index| entries_for_rows.get(index).copied())
                .map(|entry| render_tree_entry(this, &frame_for_rows, entry, cx))
                .collect::<Vec<_>>()
        }),
    )
    .track_scroll(panel.uniform_scroll())
    .w(frame.content_width)
    .flex_1()
    .restrict_scroll_to_axis();
    let mut content = v_flex().w(frame.content_width).h_full().child(body);
    if tree_entries.details_truncated {
        content = content.child(
            div()
                .flex_none()
                .px_3()
                .py_1()
                .text_xs()
                .text_color(frame.muted_fg)
                .child(format!(
                    "树形明细已限制为 {MAX_TREE_DETAIL_ENTRIES} 项，请收起部分行后继续展开"
                )),
        );
    }
    let scroll = wrap_alternate_scroll(
        panel,
        content.into_any_element(),
        frame.content_width,
        "result-tree-scroll",
        "result-tree-v-scrollbar",
        "result-tree-h-scrollbar",
        cx,
    );
    v_flex()
        .size_full()
        .min_w_0()
        .child(scroll)
        .into_any_element()
}

/// Flattens expanded rows into bounded virtual-list entries without copying result values.
fn build_tree_entries(panel: &ResultPanel, view: &DisplayView) -> TreeEntries {
    let mut entries = Vec::with_capacity(view.display_indices.len());
    let mut detail_count = 0usize;
    let mut details_truncated = false;
    for &row_index in view.display_indices.iter() {
        entries.push(TreeEntry {
            row_index,
            column_index: None,
        });
        if !panel.tree_row_expanded(row_index) {
            continue;
        }
        for &column_index in view.visible_col_indices.iter() {
            if detail_count >= MAX_TREE_DETAIL_ENTRIES {
                details_truncated = true;
                break;
            }
            entries.push(TreeEntry {
                row_index,
                column_index: Some(column_index),
            });
            detail_count += 1;
        }
    }
    TreeEntries {
        entries: Arc::new(entries),
        details_truncated,
    }
}

/// Renders a row header or one field and maps clicks back to source coordinates.
fn render_tree_entry(
    panel: &mut ResultPanel,
    frame: &AlternateFrame,
    entry: TreeEntry,
    cx: &mut Context<ResultPanel>,
) -> AnyElement {
    if frame.result.rows.get(entry.row_index).is_none() {
        return div().into_any_element();
    }
    match entry.column_index {
        None => {
            let expanded = panel.tree_row_expanded(entry.row_index);
            let first_column = frame.visible_col_indices.first().copied();
            let preview = frame
                .visible_col_indices
                .iter()
                .take(3)
                .filter_map(|&column_index| {
                    panel
                        .cell_value(entry.row_index, column_index)
                        .map(|value| {
                            display_cell_value(Some(value), 48, frame.display_binary_16_as_uuid)
                        })
                })
                .collect::<Vec<_>>()
                .join("  |  ");
            let panel_entity = cx.entity();
            div()
                .id(SharedString::from(format!(
                    "result-tree-row-{}",
                    entry.row_index
                )))
                .w(frame.content_width)
                .h(px(32.0))
                .flex_none()
                .items_center()
                .gap_2()
                .px_3()
                .bg(frame.muted_bg.opacity(0.12))
                .border_b_1()
                .border_color(frame.border)
                .cursor_pointer()
                .on_click(move |_: &ClickEvent, _, app| {
                    panel_entity.update(app, |panel, cx| {
                        panel.toggle_tree_row(entry.row_index, cx);
                        panel.set_selected_cell(
                            first_column.map(|column| (entry.row_index, column)),
                        );
                    });
                })
                .child(Icon::new(if expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                }))
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(frame.fg)
                        .child(format!(
                            "第 {} 行",
                            frame
                                .row_number_offset
                                .saturating_add(entry.row_index)
                                .saturating_add(1)
                        )),
                )
                .child(
                    div()
                        .min_w_0()
                        .text_xs()
                        .text_color(frame.muted_fg)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(preview),
                )
                .into_any_element()
        }
        Some(column_index) => {
            let name = frame
                .result
                .columns
                .get(column_index)
                .cloned()
                .unwrap_or_default();
            let type_name = frame
                .result
                .column_types
                .get(column_index)
                .filter(|value| !value.is_empty())
                .cloned();
            let value = display_cell_value(
                panel.cell_value(entry.row_index, column_index),
                400,
                frame.display_binary_16_as_uuid,
            );
            let is_null = matches!(
                panel.cell_value(entry.row_index, column_index),
                Some(Value::Null)
            );
            let row_index = entry.row_index;
            let panel_entity = cx.entity();
            h_flex()
                .id(SharedString::from(format!(
                    "result-tree-cell-{row_index}-{column_index}"
                )))
                .w(frame.content_width)
                .h(px(32.0))
                .flex_none()
                .items_center()
                .gap_2()
                .pl(px(34.0))
                .pr_3()
                .border_b_1()
                .border_color(frame.border)
                .cursor_pointer()
                .on_click(move |_: &ClickEvent, _, app| {
                    panel_entity.update(app, |panel, cx| {
                        panel.set_selected_cell(Some((row_index, column_index)));
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .w(px(180.0))
                        .flex_none()
                        .text_xs()
                        .text_color(frame.fg)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(name),
                )
                .when_some(type_name, |this, type_name| {
                    this.child(
                        div()
                            .w(px(100.0))
                            .flex_none()
                            .text_xs()
                            .text_color(frame.muted_fg)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(type_name),
                    )
                })
                .child(
                    div()
                        .min_w_0()
                        .font_family(frame.mono_font.clone())
                        .text_xs()
                        .text_color(if is_null { frame.muted_fg } else { frame.fg })
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(value),
                )
                .into_any_element()
        }
    }
}
