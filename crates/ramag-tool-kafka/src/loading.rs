use std::time::Duration;

use gpui::{AnyElement, ElementId, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{Theme, animation::Transition, animation::ease_out_cubic, h_flex, v_flex};

/// 将加载中的占位内容轻量淡入，避免空快照切换到真实数据时整块跳动。
pub(super) fn loading_transition<E>(element: E, id: impl Into<ElementId>) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    Transition::new(Duration::from_millis(180))
        .ease(ease_out_cubic)
        .fade(0.35, 1.0)
        .apply(element, id)
        .into_any_element()
}

pub(super) fn skeleton_bar(theme: &Theme, width: f32, height: f32) -> gpui::Div {
    div()
        .w(px(width))
        .h(px(height))
        .flex_none()
        .rounded(px(4.0))
        .bg(theme.muted.opacity(0.52))
}

fn skeleton_cell(theme: &Theme, width: Option<f32>) -> gpui::Div {
    let mut cell = div()
        .h(px(8.0))
        .rounded(px(4.0))
        .bg(theme.muted.opacity(0.52));
    if let Some(width) = width {
        cell = cell.w(px(width)).flex_none();
    } else {
        cell = cell.flex_1().min_w_0();
    }
    cell
}

/// 使用与正式表格一致的固定行高和弹性首列，保证加载态不会把内容挤成竖排。
pub(super) fn skeleton_table(
    theme: &Theme,
    row_count: usize,
    columns: &[Option<f32>],
) -> gpui::Div {
    let mut table = v_flex().w_full().min_w_0();
    for _ in 0..row_count {
        let mut row = h_flex()
            .w_full()
            .min_w_0()
            .h(px(40.0))
            .flex_none()
            .items_center()
            .gap(px(10.0))
            .px(px(12.0))
            .border_b_1()
            .border_color(theme.border);
        for width in columns {
            row = row.child(skeleton_cell(theme, *width));
        }
        table = table.child(row);
    }
    table
}

pub(super) fn skeleton_metric_card(theme: &Theme) -> gpui::Div {
    v_flex()
        .flex_1()
        .min_w_0()
        .gap(px(8.0))
        .p(px(14.0))
        .border_1()
        .border_color(theme.border)
        .rounded(px(6.0))
        .bg(theme.secondary.opacity(0.45))
        .child(skeleton_bar(theme, 68.0, 8.0))
        .child(skeleton_bar(theme, 48.0, 22.0))
}

pub(super) fn skeleton_section_heading(
    theme: &Theme,
    title_width: f32,
    subtitle_width: f32,
) -> gpui::Div {
    v_flex()
        .w_full()
        .gap(px(5.0))
        .child(skeleton_bar(theme, title_width, 10.0))
        .child(skeleton_bar(theme, subtitle_width, 8.0))
}
