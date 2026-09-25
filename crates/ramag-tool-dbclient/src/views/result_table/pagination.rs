use gpui_kit::component::{
    Disableable as _, Icon, IconName, Sizable as _, button::ButtonVariants as _, h_flex,
};
use gpui_kit::{Entity, IntoElement, ParentElement, Styled, div, prelude::*};

use super::{ResultPanel, ResultPanelEvent, render_page_size_selector};
use crate::views::result_panel::{ResultPagination, TotalRows};

/// Formats the visible row interval and the strongest available total-row bound.
/// Unknown totals use the next-page lower bound so users can distinguish `501+` from `501`.
pub(super) fn format_result_page_range(
    pagination: ResultPagination,
    row_number_offset: usize,
    total_rows: usize,
) -> String {
    let range_start = if total_rows == 0 {
        0
    } else {
        row_number_offset.saturating_add(1)
    };
    let range_end = if total_rows == 0 {
        0
    } else {
        row_number_offset.saturating_add(total_rows)
    };
    let total = match pagination.total {
        TotalRows::Known(total) => total.to_string(),
        TotalRows::Counting => "…".to_string(),
        TotalRows::Unavailable if pagination.has_more => {
            format!("{}+", range_end.saturating_add(1))
        }
        TotalRows::Unavailable => range_end.to_string(),
    };
    format!("{range_start}-{range_end} of {total}")
}

/// Renders the compact pager used by the result status bar.
/// Page changes remain ResultPanel events, so query cancellation and edit guards stay centralized.
pub(super) fn render_pagination_controls(
    panel_entity: Entity<ResultPanel>,
    pagination: ResultPagination,
    total_pages: Option<u64>,
    row_number_offset: usize,
    total_rows: usize,
    pending_edit_count: usize,
    dml_busy: bool,
) -> impl IntoElement {
    let blocked = pending_edit_count > 0 || dml_busy;
    let has_previous_page = pagination.page > 0;
    let previous_page = pagination.page.saturating_sub(1);
    let next_page = pagination.page.saturating_add(1);
    let has_last_page = total_pages
        .and_then(|pages| usize::try_from(pages).ok())
        .is_some_and(|pages| pagination.page.saturating_add(1) < pages);
    let last_page = total_pages.and_then(|pages| usize::try_from(pages.saturating_sub(1)).ok());
    let page_range = format_result_page_range(pagination, row_number_offset, total_rows);
    let panel_for_first = panel_entity.clone();
    let panel_for_previous = panel_entity.clone();
    let panel_for_next = panel_entity.clone();
    let panel_for_last = panel_entity.clone();

    h_flex()
        .id("result-pagination-controls")
        .debug_selector(|| "result-pagination-controls".into())
        .flex_1()
        .min_w_0()
        .flex_wrap()
        .justify_end()
        .items_center()
        .gap_1()
        .child(render_page_size_selector(
            pagination.page_size,
            panel_entity.clone(),
            blocked,
        ))
        .child(
            ramag_ui::clickable_button("result-page-first")
                .debug_selector(|| "result-page-first".into())
                .ghost()
                .small()
                .icon(Icon::new(gpui_kit::assets::IconName::SkipBack))
                .tooltip("第一页")
                .disabled(!has_previous_page || blocked)
                .on_click(move |_, _, app| {
                    panel_for_first.update(app, |_, cx| {
                        cx.emit(ResultPanelEvent::PageRequested(0));
                    });
                }),
        )
        .child(
            ramag_ui::clickable_button("result-page-previous")
                .debug_selector(|| "result-page-previous".into())
                .ghost()
                .small()
                .icon(IconName::ChevronLeft)
                .tooltip("上一页")
                .disabled(!has_previous_page || blocked)
                .on_click(move |_, _, app| {
                    panel_for_previous.update(app, |_, cx| {
                        cx.emit(ResultPanelEvent::PageRequested(previous_page));
                    });
                }),
        )
        .child(
            div()
                .id("result-page-range")
                .debug_selector(|| "result-page-range".into())
                .flex_none()
                .text_xs()
                .child(page_range),
        )
        .when_some(
            total_pages.filter(|pages| *pages > 1),
            |this, total_pages| {
                let panel_for_jump = panel_entity.clone();
                let current_page = pagination.page.saturating_add(1);
                this.child(
                    ramag_ui::clickable_button("result-page-jump")
                        .debug_selector(|| "result-page-jump".into())
                        .ghost()
                        .small()
                        .icon(IconName::Ellipsis)
                        .tooltip(format!("输入 1-{total_pages} 的页码"))
                        .disabled(blocked)
                        .on_click(move |_, window, app| {
                            let panel = panel_for_jump.clone();
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
                )
            },
        )
        .child(
            ramag_ui::clickable_button("result-page-next")
                .debug_selector(|| "result-page-next".into())
                .ghost()
                .small()
                .icon(IconName::ChevronRight)
                .tooltip("下一页")
                .disabled(!pagination.has_more || blocked)
                .on_click(move |_, _, app| {
                    panel_for_next.update(app, |_, cx| {
                        cx.emit(ResultPanelEvent::PageRequested(next_page));
                    });
                }),
        )
        .child(
            ramag_ui::clickable_button("result-page-last")
                .debug_selector(|| "result-page-last".into())
                .ghost()
                .small()
                .icon(Icon::new(gpui_kit::assets::IconName::SkipForward))
                .tooltip("最后一页")
                .disabled(!has_last_page || blocked)
                .on_click(move |_, _, app| {
                    if let Some(page) = last_page {
                        panel_for_last.update(app, |_, cx| {
                            cx.emit(ResultPanelEvent::PageRequested(page));
                        });
                    }
                }),
        )
}

pub(super) fn parse_result_page(value: &str, total_pages: u64) -> Result<usize, String> {
    let page = value
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("页码必须是 1-{total_pages} 的整数"))?;
    if page == 0 || page > total_pages {
        return Err(format!("页码必须在 1-{total_pages} 范围内"));
    }
    usize::try_from(page - 1).map_err(|_| "页码超出当前平台可定位范围".into())
}

#[cfg(test)]
mod tests {
    use super::{format_result_page_range, parse_result_page};
    use crate::views::result_panel::{ResultPagination, TotalRows};

    #[test]
    fn formats_unknown_total_as_a_lower_bound_when_another_page_exists() {
        let pagination = ResultPagination {
            page: 0,
            page_size: 500,
            has_more: true,
            total: TotalRows::Unavailable,
        };
        assert_eq!(
            format_result_page_range(pagination, 0, 500),
            "1-500 of 501+"
        );
    }

    #[test]
    fn formats_known_total_and_empty_page_without_fabricating_rows() {
        let pagination = ResultPagination {
            page: 1,
            page_size: 500,
            has_more: false,
            total: TotalRows::Known(500),
        };
        assert_eq!(format_result_page_range(pagination, 500, 0), "0-0 of 500");
    }

    #[test]
    fn formats_counting_total_without_claiming_a_final_count() {
        let pagination = ResultPagination {
            page: 1,
            page_size: 100,
            has_more: true,
            total: TotalRows::Counting,
        };
        assert_eq!(
            format_result_page_range(pagination, 100, 100),
            "101-200 of …"
        );
    }

    #[test]
    fn parses_one_based_page_into_zero_based_index() {
        assert_eq!(parse_result_page(" 3 ", 5).unwrap(), 2);
    }

    #[test]
    fn rejects_page_outside_known_total() {
        assert!(parse_result_page("0", 5).is_err());
        assert!(parse_result_page("6", 5).is_err());
        assert!(parse_result_page("next", 5).is_err());
    }
}
