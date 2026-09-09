use std::time::Duration;

use gpui::{AnyElement, ElementId, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{
    Theme, animation::Transition, animation::ease_out_cubic, h_flex, skeleton::Skeleton, v_flex,
};

/// 让骨架和新数据以轻微位移、淡入进入，避免空快照切换时整块跳动。
pub(super) fn loading_transition<E>(element: E, id: impl Into<ElementId>) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    Transition::new(Duration::from_millis(220))
        .ease(ease_out_cubic)
        .slide_y(px(3.0), px(0.0))
        .fade(0.55, 1.0)
        .apply(element, id)
        .into_any_element()
}

pub(super) fn skeleton_bar(theme: &Theme, width: f32, height: f32) -> gpui::Div {
    skeleton_line(theme, Some(width), height)
}

fn skeleton_cell(theme: &Theme, width: Option<f32>) -> gpui::Div {
    skeleton_line(theme, width, 8.0)
}

/// 使用 gpui-component 的循环呼吸动画；外层固定尺寸负责约束表格布局。
fn skeleton_line(theme: &Theme, width: Option<f32>, height: f32) -> gpui::Div {
    let mut line = div()
        .h(px(height))
        .overflow_hidden()
        .rounded(px(4.0))
        .bg(theme.muted.opacity(0.24));
    if let Some(width) = width {
        line = line.w(px(width)).flex_none();
    } else {
        line = line.flex_1().min_w_0();
    }
    line.child(
        Skeleton::new()
            .secondary()
            .w_full()
            .h(px(height))
            .rounded(px(4.0)),
    )
}

/// 使用与正式表格一致的固定行高和弹性列，保证加载态不会把内容挤成竖排。
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
            .overflow_hidden()
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

/// 为带右侧详情的页面保留标题、摘要和明细行的稳定几何。
pub(super) fn skeleton_detail_panel(
    theme: &Theme,
    title_width: f32,
    subtitle_width: f32,
    row_count: usize,
    columns: &[Option<f32>],
) -> gpui::Div {
    v_flex()
        .w_full()
        .min_w_0()
        .gap(px(10.0))
        .child(skeleton_section_heading(theme, title_width, subtitle_width))
        .child(
            v_flex()
                .w_full()
                .border_1()
                .border_color(theme.border)
                .rounded(px(6.0))
                .child(skeleton_table(theme, row_count, columns)),
        )
}
