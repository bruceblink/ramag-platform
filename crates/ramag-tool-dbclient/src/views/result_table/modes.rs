use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, Styled, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
    button::ButtonVariants as _,
    h_flex,
    scroll::{Scrollbar, ScrollbarShow},
    v_flex,
};
use ramag_domain::entities::QueryResult;
use ramag_ui::{PointerDropdownMenu as _, RestrictScrollToAxisExt as _};

use crate::views::result_panel::{
    ResultPanel, ResultPanelEvent, ResultViewMode, RowSearchBlocker, TotalRows,
};
use crate::views::result_value::{CellCopyFormat, display_cell_value};

use super::pagination::parse_result_page;
use super::{DisplayView, ensure_display_view};

#[path = "text.rs"]
mod text;
#[path = "transpose.rs"]
mod transpose;
#[path = "tree.rs"]
mod tree;

/// Renders the mode selector and dispatches to a local renderer without re-running SQL.
pub(in crate::views) fn render_result_view(
    panel: &mut ResultPanel,
    result: &Arc<QueryResult>,
    cx: &mut Context<ResultPanel>,
) -> AnyElement {
    let theme = cx.theme();
    let mode = panel.view_mode();
    let body = match mode {
        ResultViewMode::Table => super::render_table(
            panel,
            result,
            theme.foreground,
            theme.muted_foreground,
            theme.secondary,
            theme.border,
            theme.muted,
            theme.accent,
            cx,
        ),
        ResultViewMode::Tree | ResultViewMode::Text | ResultViewMode::Transpose => {
            render_alternate_view(panel, result, cx)
        }
    };

    if result.columns.is_empty() {
        return body;
    }

    v_flex()
        .size_full()
        .min_w_0()
        .child(render_mode_toolbar(panel, cx))
        .child(div().flex_1().min_h_0().min_w_0().child(body))
        .into_any_element()
}

/// Builds a wrapping segmented control so every mode remains reachable in a narrow window.
fn render_mode_toolbar(panel: &ResultPanel, cx: &mut Context<ResultPanel>) -> AnyElement {
    let current = panel.view_mode();
    let has_pending_edits = panel.pending_cell_edit_count() > 0;
    let panel_entity = cx.entity();
    let mut modes = h_flex()
        .id("result-view-mode-segment")
        .debug_selector(|| "result-view-mode-segment".into())
        .min_w_0()
        .flex_wrap()
        .gap_1();
    for mode in ResultViewMode::ALL {
        let panel = panel_entity.clone();
        let selected = current == mode;
        let blocked = has_pending_edits && mode != ResultViewMode::Table;
        let id = mode_id(mode);
        let mut button = ramag_ui::clickable_button(id)
            .debug_selector(move || id.into())
            .small()
            .icon(mode_icon(mode))
            .label(mode.label())
            .selected(selected)
            .tooltip(if blocked {
                "请先提交或撤销未提交单元格修改"
            } else {
                mode.description()
            })
            .disabled(blocked);
        button = if selected {
            button.primary()
        } else {
            button.ghost()
        };
        modes = modes.child(button.on_click(move |_: &ClickEvent, _, app| {
            panel.update(app, |panel, cx| panel.set_view_mode(mode, cx));
        }));
    }

    let value_actions = render_result_value_actions(panel, cx);
    let theme = cx.theme();
    ramag_ui::responsive_toolbar()
        .id("result-view-toolbar")
        .debug_selector(|| "result-view-toolbar".into())
        .flex_none()
        .gap_2()
        .px_3()
        .py_1()
        .border_b_1()
        .border_color(theme.border)
        .bg(theme.secondary)
        .child(
            Icon::new(IconName::LayoutDashboard)
                .small()
                .text_color(theme.accent),
        )
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(theme.muted_foreground)
                .whitespace_nowrap()
                .child("结果视图"),
        )
        .child(modes)
        .child(value_actions)
        .child(
            div()
                .debug_selector(|| "result-view-toolbar-help".into())
                .flex_1()
                .min_w(px(120.0))
                .text_xs()
                .text_color(theme.muted_foreground)
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(current.description()),
        )
        .into_any_element()
}

fn render_result_value_actions(panel: &ResultPanel, cx: &mut Context<ResultPanel>) -> AnyElement {
    let panel_entity = cx.entity();
    let has_selection = panel.selected_cell().is_some();
    let trigger = ramag_ui::clickable_button("result-value-actions")
        .debug_selector(|| "result-view-value-actions".into())
        .ghost()
        .small()
        .icon(IconName::Copy)
        .label("复制")
        .dropdown_caret(true)
        .disabled(!has_selection);
    trigger
        .pointer_dropdown_menu(move |mut menu, _, _| {
            for format in CellCopyFormat::ALL {
                let panel = panel_entity.clone();
                menu = menu.item(
                    ramag_ui::menu_item(format!("复制为 {}", format.label()))
                        .icon(IconName::Copy)
                        .on_click(move |_, _, app| {
                            panel.update(app, |panel, cx| {
                                panel.copy_selected_cell_as(format, cx);
                            });
                        }),
                );
            }
            let panel = panel_entity.clone();
            menu.separator()
                .item(ramag_ui::menu_item("查看值").icon(IconName::Eye).on_click(
                    move |_, window, app| {
                        panel.update(app, |panel, cx| {
                            panel.open_selected_cell_viewer(window, cx);
                        });
                    },
                ))
        })
        .into_any_element()
}

fn mode_id(mode: ResultViewMode) -> &'static str {
    match mode {
        ResultViewMode::Table => "result-view-mode-table",
        ResultViewMode::Tree => "result-view-mode-tree",
        ResultViewMode::Text => "result-view-mode-text",
        ResultViewMode::Transpose => "result-view-mode-transpose",
    }
}

fn mode_icon(mode: ResultViewMode) -> IconName {
    match mode {
        ResultViewMode::Table => IconName::LayoutDashboard,
        ResultViewMode::Tree => IconName::Network,
        ResultViewMode::Text => IconName::File,
        ResultViewMode::Transpose => IconName::PanelLeft,
    }
}

/// Builds a read-only mode from the same filtered/sorted cache used by the table view.
fn render_alternate_view(
    panel: &mut ResultPanel,
    result: &Arc<QueryResult>,
    cx: &mut Context<ResultPanel>,
) -> AnyElement {
    if let Some(blocker) = panel.row_search_blocker(cx) {
        let (message, color) = match blocker {
            RowSearchBlocker::Converting => (
                "正在通过外部程序转换 ID…".to_string(),
                cx.theme().muted_foreground,
            ),
            RowSearchBlocker::Error(error) => (format!("ID 转换失败：{error}"), cx.theme().danger),
        };
        return v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .px_4()
            .text_xs()
            .text_color(color)
            .child(message)
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("请修改搜索词，或检查设置中的转换方式。"),
            )
            .into_any_element();
    }

    let Some(view) = ensure_display_view(panel, result, cx) else {
        let error = panel.display_view_error().map(str::to_owned);
        let message = error.clone().unwrap_or_else(|| {
            format!(
                "正在准备结果视图…（最多处理 {} 行）",
                result.rows.len().min(super::MAX_ROWS_DISPLAY)
            )
        });
        let mut placeholder = v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .text_xs()
            .text_color(if error.is_some() {
                cx.theme().danger
            } else {
                cx.theme().muted_foreground
            })
            .child(message);
        if error.is_some() {
            placeholder = placeholder.child(
                ramag_ui::clickable_button("result-view-retry")
                    .ghost()
                    .small()
                    .label("重试")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.invalidate_display_view();
                        cx.notify();
                    })),
            );
        }
        return placeholder.into_any_element();
    };

    let frame = alternate_frame(panel, result, &view, cx);
    let footer = render_alternate_footer(panel, result, &view, cx);
    let body = match panel.view_mode() {
        ResultViewMode::Tree => tree::render_tree_view(panel, frame, &view, cx),
        ResultViewMode::Text => text::render_text_view(panel, frame, cx),
        ResultViewMode::Transpose => transpose::render_transpose_view(panel, frame, &view, cx),
        ResultViewMode::Table => unreachable!("table mode is rendered by render_result_view"),
    };
    v_flex()
        .size_full()
        .min_w_0()
        .child(body)
        .child(footer)
        .into_any_element()
}

pub(super) struct AlternateFrame {
    pub(super) result: Arc<QueryResult>,
    pub(super) display_indices: Arc<Vec<usize>>,
    pub(super) visible_col_indices: Arc<Vec<usize>>,
    pub(super) row_number_offset: usize,
    pub(super) content_width: gpui::Pixels,
    pub(super) display_binary_16_as_uuid: bool,
    pub(super) mono_font: gpui::SharedString,
    pub(super) fg: gpui::Hsla,
    pub(super) muted_fg: gpui::Hsla,
    pub(super) border: gpui::Hsla,
    pub(super) muted_bg: gpui::Hsla,
    pub(super) accent: gpui::Hsla,
}

/// Copies render inputs into a stable frame so virtual-list callbacks do not borrow the panel.
fn alternate_frame(
    panel: &ResultPanel,
    result: &Arc<QueryResult>,
    view: &DisplayView,
    cx: &Context<ResultPanel>,
) -> Rc<AlternateFrame> {
    let row_number_offset = panel
        .pagination()
        .map(|page| page.page.saturating_mul(page.page_size))
        .unwrap_or(0);
    let content_width = px(
        (view.visible_col_indices.len().saturating_mul(150) as f32 + 240.0).clamp(680.0, 8_000.0),
    );
    let display_binary_16_as_uuid =
        ramag_ui::database_result_settings(cx).display_binary_16_as_uuid;
    Rc::new(AlternateFrame {
        result: result.clone(),
        display_indices: view.display_indices.clone(),
        visible_col_indices: view.visible_col_indices.clone(),
        row_number_offset,
        content_width,
        display_binary_16_as_uuid,
        mono_font: cx.theme().mono_font_family.clone(),
        fg: cx.theme().foreground,
        muted_fg: cx.theme().muted_foreground,
        border: cx.theme().border,
        muted_bg: cx.theme().muted,
        accent: cx.theme().accent,
    })
}

/// Renders shared status and pagination controls without issuing a database request.
fn render_alternate_footer(
    panel: &ResultPanel,
    result: &QueryResult,
    view: &DisplayView,
    cx: &mut Context<ResultPanel>,
) -> AnyElement {
    let pagination = panel.pagination();
    let mut status = if view.row_filtering {
        format!(
            "显示 {} / {} 行",
            view.display_indices.len(),
            view.pre_filter_count
        )
    } else if view.truncated {
        format!(
            "显示 {} / {} 行（已截断）",
            view.display_indices.len(),
            result.rows.len()
        )
    } else {
        format!("{} 行", result.rows.len())
    };
    if view.cols_filtered {
        status.push_str(&format!(" · 命中 {} 列", view.matched_col_count));
    }
    let display_binary_16_as_uuid =
        ramag_ui::database_result_settings(cx).display_binary_16_as_uuid;
    if let Some((row, column)) = panel.selected_cell()
        && let Some(name) = result.columns.get(column)
        && let Some(value) = panel.cell_value(row, column)
    {
        status.push_str(&format!(
            " · [{name}] = {}",
            display_cell_value(Some(value), 40, display_binary_16_as_uuid)
        ));
    }
    let status_context = h_flex()
        .id("result-alternate-status-context")
        .debug_selector(|| "result-alternate-status-context".into())
        .flex_1()
        .min_w_0()
        .min_w(px(160.0))
        .overflow_hidden()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .child(status),
        )
        .child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .child(format!("· 耗时 {} ms", result.elapsed_ms)),
        );
    let mut footer = ramag_ui::responsive_toolbar()
        .id("result-alternate-status")
        .debug_selector(|| "result-alternate-status".into())
        .flex_none()
        .border_t_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(status_context);
    if let Some(pagination) = pagination {
        let panel_entity = cx.entity();
        let pending = panel.pending_cell_edit_count() > 0 || panel.dml_busy();
        let previous_page = pagination.page.saturating_sub(1);
        let next_page = pagination.page.saturating_add(1);
        let total_pages = match pagination.total {
            TotalRows::Known(total) if pagination.page_size > 0 => {
                Some(total.div_ceil(pagination.page_size as u64).max(1))
            }
            _ => None,
        };
        footer = footer
            .child(super::render_page_size_selector(
                pagination.page_size,
                panel_entity.clone(),
                pending,
            ))
            .child(
                ramag_ui::clickable_button("result-alternate-page-previous")
                    .debug_selector(|| "result-alternate-page-previous".into())
                    .ghost()
                    .small()
                    .label("上页")
                    .disabled(pending || pagination.page == 0)
                    .on_click({
                        let panel = panel_entity.clone();
                        move |_, _, app| {
                            panel.update(app, |_, cx| {
                                cx.emit(ResultPanelEvent::PageRequested(previous_page));
                            });
                        }
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .child(match total_pages {
                        Some(total_pages) => {
                            format!("第 {} / {} 页", pagination.page + 1, total_pages)
                        }
                        None => format!("第 {} 页", pagination.page + 1),
                    }),
            );

        if let Some(total_pages) = total_pages.filter(|pages| *pages > 1) {
            let current_page = pagination.page.saturating_add(1);
            let panel = panel_entity.clone();
            footer = footer.child(
                ramag_ui::clickable_button("result-alternate-page-jump")
                    .debug_selector(|| "result-alternate-page-jump".into())
                    .ghost()
                    .small()
                    .label("跳页")
                    .tooltip(format!("输入 1-{total_pages} 的页码"))
                    .disabled(pending)
                    .on_click(move |_, window, app| {
                        let panel = panel.clone();
                        ramag_ui::open_bounded_prompt(
                            "跳转到结果页",
                            format!("输入 1-{total_pages} 的页码"),
                            &current_page.to_string(),
                            "跳转",
                            32,
                            move |value, _, app| match parse_result_page(&value, total_pages) {
                                Ok(page) => panel.update(app, |_, cx| {
                                    cx.emit(ResultPanelEvent::PageRequested(page));
                                }),
                                Err(message) => panel.update(app, |panel, cx| {
                                    panel.notify_result_error(message, cx);
                                }),
                            },
                            window,
                            app,
                        );
                    }),
            );
        }
        footer = footer.child(
            ramag_ui::clickable_button("result-alternate-page-next")
                .debug_selector(|| "result-alternate-page-next".into())
                .ghost()
                .small()
                .label("下页")
                .disabled(pending || !pagination.has_more)
                .on_click(move |_, _, app| {
                    panel_entity.update(app, |_, cx| {
                        cx.emit(ResultPanelEvent::PageRequested(next_page));
                    });
                }),
        );
    }
    footer.into_any_element()
}

/// Wraps an alternate renderer with the same two-axis scrolling and scrollbar setting as tables.
pub(super) fn wrap_alternate_scroll(
    panel: &ResultPanel,
    body: AnyElement,
    content_width: gpui::Pixels,
    horizontal_id: &'static str,
    vertical_scrollbar_id: &'static str,
    horizontal_scrollbar_id: &'static str,
    cx: &mut Context<ResultPanel>,
) -> AnyElement {
    let viewport = div()
        .relative()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .child(
            div()
                .id(horizontal_id)
                .debug_selector(move || horizontal_id.into())
                .size_full()
                .overflow_x_scroll()
                .restrict_scroll_to_axis()
                .track_scroll(panel.h_scroll())
                .child(body),
        )
        .child(
            div()
                .id(format!("{horizontal_id}-scroll-input"))
                .absolute()
                .inset_0()
                .on_scroll_wheel(cx.listener(ResultPanel::on_result_scroll)),
        )
        .child(
            div()
                .id(vertical_scrollbar_id)
                .debug_selector(move || vertical_scrollbar_id.into())
                .absolute()
                .top_0()
                .bottom_0()
                .right_0()
                .w(px(16.0))
                .bg(cx.theme().scrollbar)
                .child(
                    Scrollbar::vertical(panel.uniform_scroll())
                        .id(format!("{vertical_scrollbar_id}-control"))
                        .scrollbar_show(ScrollbarShow::Always),
                ),
        );
    let horizontal_scrollbar = div()
        .id(horizontal_scrollbar_id)
        .debug_selector(move || horizontal_scrollbar_id.into())
        .flex_none()
        .w_full()
        .h(px(16.0))
        .relative()
        .bg(cx.theme().scrollbar)
        .child(
            Scrollbar::horizontal(panel.h_scroll())
                .id(format!("{horizontal_scrollbar_id}-control"))
                .scroll_size(gpui::size(content_width, px(16.0)))
                .scrollbar_show(ScrollbarShow::Always),
        );
    let show_horizontal_scrollbar =
        ramag_ui::database_result_settings(cx).show_horizontal_scrollbar;
    v_flex()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .child(viewport)
        .when(show_horizontal_scrollbar, |container| {
            container.child(horizontal_scrollbar)
        })
        .into_any_element()
}
