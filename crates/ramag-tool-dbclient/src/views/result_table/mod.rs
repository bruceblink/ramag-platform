use std::ops::Range;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, button::ButtonVariants as _, h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement, SharedString, div,
    prelude::*, px, uniform_list,
};

use ramag_domain::entities::{QueryResult, contains_case_insensitive};

use super::result_panel::{
    MAX_ROWS_DISPLAY, ResultPanel, ResultPanelEvent, RowFilter, SortDir, TotalRows,
};

/// 连续输入筛选词时先等待短暂停顿，避免每个按键都占用共享 CPU 工作池。
const DISPLAY_VIEW_DEBOUNCE: Duration = Duration::from_millis(160);
/// 横向表格未做列虚拟化；限制交互式列数，避免异常宽结果创建数千个控件。
const MAX_COLUMNS_DISPLAY: usize = 512;
/// 固定表头高度；垂直滚动条视口从表头底部开始。
const RESULT_HEADER_HEIGHT: gpui_kit::Pixels = px(34.0);

/// 单帧共享数据，供虚拟列表闭包读取。
struct TableRowFrame {
    result: Arc<QueryResult>,
    display_indices: Arc<Vec<usize>>,
    visible_col_indices: Arc<Vec<usize>>,
    col_widths: Vec<gpui_kit::Pixels>,
    display_binary_16_as_uuid: bool,
    right_align: Arc<Vec<bool>>,
    row_number_offset: usize,
    row_num_width: gpui_kit::Pixels,
    checkbox_col_width: gpui_kit::Pixels,
    total_content_width: gpui_kit::Pixels,
    mono_font: SharedString,
    fg: gpui_kit::Hsla,
    muted_fg: gpui_kit::Hsla,
    border: gpui_kit::Hsla,
    muted_bg: gpui_kit::Hsla,
    accent: gpui_kit::Hsla,
}

#[derive(Clone)]
pub(crate) struct DisplayView {
    pub(crate) visible_col_indices: Arc<Vec<usize>>,
    /// 列过滤命中总数；可能大于交互式显示上限。
    pub(crate) matched_col_count: usize,
    /// 是否因 MAX_COLUMNS_DISPLAY 仅显示命中列前缀。
    pub(crate) columns_truncated: bool,
    pub(crate) display_indices: Arc<Vec<usize>>,
    /// 基于当前显示行样本估算的默认列宽；手动覆盖在渲染时叠加。
    default_col_widths: Arc<Vec<gpui_kit::Pixels>>,
    /// 基于当前显示行样本识别的数值列。
    right_align: Arc<Vec<bool>>,
    /// 是否因 MAX_ROWS_DISPLAY 截断未分页结果。
    pub(crate) truncated: bool,
    pub(crate) cols_filtered: bool,
    pub(crate) row_filtering: bool,
    pub(crate) pre_filter_count: usize,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct DisplayViewCacheKey {
    result_identity: usize,
    result_revision: u64,
    sort_by: Option<(usize, SortDir)>,
    column_filter: String,
    row_filter: RowFilter,
    display_binary_16_as_uuid: bool,
}

/// SQL 结果表派生视图缓存。内容不持有 QueryResult，避免延长旧结果的生命周期。
pub(crate) struct DisplayViewCache {
    key: DisplayViewCacheKey,
    view: DisplayView,
}

impl DisplayViewCache {
    fn get(&self, key: &DisplayViewCacheKey) -> Option<DisplayView> {
        (self.key == *key).then(|| self.view.clone())
    }
}

impl DisplayViewCacheKey {
    fn only_filters_differ_from(&self, previous: &Self) -> bool {
        self.result_identity == previous.result_identity
            && self.result_revision == previous.result_revision
            && self.sort_by == previous.sort_by
            && self.display_binary_16_as_uuid == previous.display_binary_16_as_uuid
            && (self.column_filter != previous.column_filter
                || self.row_filter != previous.row_filter)
    }
}

fn display_view_key(
    panel: &ResultPanel,
    result: &QueryResult,
    cx: &gpui_kit::App,
) -> DisplayViewCacheKey {
    let column_filter = panel.column_filter_text(cx);
    let row_filter = panel.effective_row_filter(cx);
    let display_binary_16_as_uuid =
        ramag_ui::database_result_settings(cx).display_binary_16_as_uuid;
    DisplayViewCacheKey {
        result_identity: result as *const QueryResult as usize,
        result_revision: panel.result_revision,
        sort_by: panel.sort_by(),
        column_filter,
        row_filter,
        display_binary_16_as_uuid,
    }
}

/// 只读取已完成且与当前输入严格匹配的派生视图；用户操作不得同步回退扫描大结果集。
pub(crate) fn cached_display_view(
    panel: &ResultPanel,
    result: &QueryResult,
    cx: &gpui_kit::App,
) -> Option<DisplayView> {
    let key = display_view_key(panel, result, cx);
    panel
        .display_view_cache
        .as_ref()
        .and_then(|cache| cache.get(&key))
}

/// 确保当前排序 / 筛选视图在受限工作池中计算。缓存未就绪时返回 None，渲染层显示进度态。
pub(super) fn ensure_display_view(
    panel: &mut ResultPanel,
    result: &Arc<QueryResult>,
    cx: &mut Context<ResultPanel>,
) -> Option<DisplayView> {
    let key = display_view_key(panel, result, cx);
    if let Some(view) = panel
        .display_view_cache
        .as_ref()
        .and_then(|cache| cache.get(&key))
    {
        return Some(view);
    }
    if panel.display_view_build_key.as_ref() == Some(&key) {
        return None;
    }

    let previous_key = panel
        .display_view_build_key
        .as_ref()
        .or_else(|| panel.display_view_cache.as_ref().map(|cache| &cache.key));
    let debounce = previous_key.is_some_and(|previous| key.only_filters_differ_from(previous));
    panel.cancel_display_view_build();
    panel.display_view_request_seq = panel.display_view_request_seq.wrapping_add(1);
    let request_seq = panel.display_view_request_seq;
    panel.display_view_cache = None;
    panel.display_view_build_key = Some(key.clone());
    panel.display_view_building = true;
    panel.display_view_error = None;
    let cancelled = Arc::new(AtomicBool::new(false));
    panel.display_view_cancel = Some(cancelled.clone());

    let result = result.clone();
    let request_key = key.clone();
    cx.spawn(async move |this, cx| {
        if debounce {
            cx.background_executor().timer(DISPLAY_VIEW_DEBOUNCE).await;
        }
        let current = this
            .update(cx, |this, _| {
                this.display_view_request_seq == request_seq
                    && this.display_view_build_key.as_ref() == Some(&request_key)
                    && this
                        .display_view_cancel
                        .as_ref()
                        .is_some_and(|token| Arc::ptr_eq(token, &cancelled))
            })
            .unwrap_or(false);
        if !current {
            return;
        }

        let worker_cancelled = cancelled.clone();
        let worker_key = request_key.clone();
        let built = ramag_app::run_blocking(move || {
            Ok(build_display_view_cancellable(
                &result,
                worker_key.sort_by,
                &worker_key.column_filter,
                &worker_key.row_filter,
                worker_key.display_binary_16_as_uuid,
                &worker_cancelled,
            ))
        })
        .await;
        let _ = this.update(cx, |this, cx| {
            if this.display_view_request_seq != request_seq
                || this.display_view_build_key.as_ref() != Some(&request_key)
                || !this
                    .display_view_cancel
                    .as_ref()
                    .is_some_and(|token| Arc::ptr_eq(token, &cancelled))
            {
                return;
            }
            this.display_view_cancel = None;
            this.display_view_building = false;
            match built {
                Ok(Some(view)) => {
                    this.display_view_cache = Some(DisplayViewCache {
                        key: request_key.clone(),
                        view,
                    });
                    this.display_view_build_key = None;
                }
                Ok(None) => {
                    this.display_view_build_key = None;
                }
                Err(error) => {
                    this.display_view_error = Some(format!("构建结果视图失败：{error}"));
                }
            }
            cx.notify();
        });
    })
    .detach();

    None
}

/// Prepare the cache synchronously so layout tests never start a real worker thread.
#[cfg(test)]
pub(in crate::views) fn prepare_display_view_for_test(
    panel: &mut ResultPanel,
    result: &QueryResult,
    cx: &gpui_kit::App,
) {
    panel.display_view_cache = Some(DisplayViewCache {
        key: display_view_key(panel, result, cx),
        view: build_display_view(result, panel.sort_by(), "", ""),
    });
}

#[cfg(test)]
fn build_display_view(
    result: &QueryResult,
    sort_by: Option<(usize, SortDir)>,
    column_filter: &str,
    row_filter_lower: &str,
) -> DisplayView {
    let row_filter = RowFilter::Text(row_filter_lower.to_string());
    build_display_view_cancellable(
        result,
        sort_by,
        column_filter,
        &row_filter,
        true,
        &AtomicBool::new(false),
    )
    .expect("non-cancelled display view build should finish")
}

fn collect_matching_col_indices(
    result: &QueryResult,
    col_tokens: &[String],
    retain_all_matches: bool,
    cancelled: &AtomicBool,
) -> Option<(usize, Vec<usize>)> {
    let capacity = if retain_all_matches {
        result.columns.len()
    } else {
        result.columns.len().min(MAX_COLUMNS_DISPLAY)
    };
    let mut matching_col_indices = Vec::with_capacity(capacity);
    let mut matched_col_count = 0;
    for (index, column) in result.columns.iter().enumerate() {
        if index % 64 == 0 && cancelled.load(Ordering::Relaxed) {
            return None;
        }
        let matches = col_tokens.is_empty()
            || col_tokens
                .iter()
                .any(|token| contains_case_insensitive(column, token));
        if matches {
            matched_col_count += 1;
            if retain_all_matches || matching_col_indices.len() < MAX_COLUMNS_DISPLAY {
                matching_col_indices.push(index);
            }
        }
    }
    Some((matched_col_count, matching_col_indices))
}

fn build_display_view_cancellable(
    result: &QueryResult,
    sort_by: Option<(usize, SortDir)>,
    column_filter: &str,
    row_filter: &RowFilter,
    display_binary_16_as_uuid: bool,
    cancelled: &AtomicBool,
) -> Option<DisplayView> {
    if cancelled.load(Ordering::Relaxed) {
        return None;
    }
    let mut display_indices = (0..result.rows.len().min(MAX_ROWS_DISPLAY)).collect::<Vec<_>>();
    let truncated = result.rows.len() > MAX_ROWS_DISPLAY;

    if let Some((sort_col, dir)) = sort_by {
        display_indices.sort_by(|&a_index, &b_index| {
            if cancelled.load(Ordering::Relaxed) {
                return std::cmp::Ordering::Equal;
            }
            let a = &result.rows[a_index];
            let b = &result.rows[b_index];
            let av = a.values.get(sort_col);
            let bv = b.values.get(sort_col);
            let ord = compare_values(av, bv);
            if matches!(dir, SortDir::Desc) {
                ord.reverse()
            } else {
                ord
            }
        });
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }
    }

    let col_tokens: Vec<String> = column_filter
        .split(',')
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    let cols_filtered = !col_tokens.is_empty();
    let pre_filter_count = display_indices.len();
    let row_filtering = row_filter.is_active();
    let (matched_col_count, matching_col_indices) =
        collect_matching_col_indices(result, &col_tokens, row_filtering, cancelled)?;
    if row_filtering {
        let mut filtered = Vec::with_capacity(display_indices.len());
        for (position, source_idx) in display_indices.into_iter().enumerate() {
            if position % 64 == 0 && cancelled.load(Ordering::Relaxed) {
                return None;
            }
            let row = &result.rows[source_idx];
            if matching_col_indices.iter().any(|&ci| {
                row.values
                    .get(ci)
                    .map(|value| row_filter.matches(value))
                    .unwrap_or(false)
            }) {
                filtered.push(source_idx);
            }
        }
        display_indices = filtered;
    }

    let columns_truncated = matched_col_count > MAX_COLUMNS_DISPLAY;
    let visible_col_indices = matching_col_indices
        .into_iter()
        .take(MAX_COLUMNS_DISPLAY)
        .collect::<Vec<_>>();

    let mut default_col_widths = vec![px(100.0); result.columns.len()];
    let mut right_align = vec![false; result.columns.len()];
    for (position, &ci) in visible_col_indices.iter().enumerate() {
        if position % 16 == 0 && cancelled.load(Ordering::Relaxed) {
            return None;
        }
        default_col_widths[ci] = estimate_col_width(
            ci,
            &result.columns,
            &result.column_types,
            result,
            &display_indices,
            display_binary_16_as_uuid,
        );
        right_align[ci] = detect_numeric_column(ci, result, &display_indices);
    }

    Some(DisplayView {
        visible_col_indices: Arc::new(visible_col_indices),
        matched_col_count,
        columns_truncated,
        display_indices: Arc::new(display_indices),
        default_col_widths: Arc::new(default_col_widths),
        right_align: Arc::new(right_align),
        truncated,
        cols_filtered,
        row_filtering,
        pre_filter_count,
    })
}

mod cells;
mod page_size;
mod pagination;
mod render;
mod states;

pub(super) use page_size::render_page_size_selector;
pub(super) use render::render_table;
#[cfg(test)]
mod header_test;
mod helpers;
#[cfg(test)]
mod quality_tests;
#[cfg(test)]
mod render_test;
#[cfg(test)]
mod selected_revert_test;
#[cfg(test)]
mod virtual_rows_test;

use cells::{render_data_row, render_header_cell, render_pending_row};

/// Renders every query result with the supported table surface.
pub(in crate::views) fn render_result_view(
    panel: &mut ResultPanel,
    result: &Arc<QueryResult>,
    cx: &mut Context<ResultPanel>,
) -> AnyElement {
    let theme = cx.theme();
    render_table(
        panel,
        result,
        theme.foreground,
        theme.muted_foreground,
        theme.secondary,
        theme.border,
        theme.muted,
        theme.accent,
        theme.danger,
        cx,
    )
}
use helpers::{compare_values, detect_numeric_column, estimate_col_width};
