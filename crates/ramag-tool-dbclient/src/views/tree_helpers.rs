//! TableTreePanel 的辅助渲染 / 工具函数（从 table_tree.rs 拆出，避免单文件过大）

use gpui_kit::component::{ActiveTheme as _, h_flex, tooltip::Tooltip};
use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    Styled, div, prelude::*, px,
};
use ramag_domain::entities::Column;

use super::table_tree::TableTreePanel;

/// 列子节点：名称占用剩余行宽，类型保持有界；截断内容可通过悬停查看。
pub(super) fn render_column_row(
    col: &Column,
    element_id: SharedString,
    fg: gpui_kit::Hsla,
    muted_fg: gpui_kit::Hsla,
    cx: &mut Context<TableTreePanel>,
) -> AnyElement {
    let pk_label = if col.is_primary_key { "🔑 " } else { "" };
    let null_mark = if col.nullable { "" } else { " *" };
    let name_for_copy = col.name.clone();
    let name_for_tooltip = col.name.clone();
    let raw_type_for_tooltip = col.data_type.raw_type.clone();
    let name_wrapper_id = SharedString::from(format!("{element_id}-name-wrapper"));
    let name_selector = SharedString::from(format!("{element_id}-field-name-frame"));
    let raw_type_element_id = SharedString::from(format!("{element_id}-raw-type"));
    let row_height = ramag_ui::content_density(cx).row_height(28.0, 24.0);
    h_flex()
        .id(element_id)
        .w_full()
        .h(px(row_height))
        .flex_none()
        .overflow_hidden()
        .pl(px(44.0))
        .pr_2()
        .gap(px(6.0))
        .items_center()
        .cursor_pointer()
        .on_click(cx.listener(move |_, event: &ClickEvent, window, cx| {
            if ramag_ui::is_primary_modifier_double_click(event) {
                ramag_ui::copy_text_with_notification(name_for_copy.clone(), window, cx);
            }
        }))
        .child(
            div()
                .text_xs()
                .text_color(fg)
                .whitespace_nowrap()
                .child(pk_label),
        )
        .child(
            div()
                .id(name_wrapper_id)
                .debug_selector(move || name_selector.to_string())
                .flex_1()
                .min_w_0()
                .h(px(20.0))
                .items_center()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .tooltip(move |window, cx| Tooltip::new(name_for_tooltip.clone()).build(window, cx))
                .child(
                    div()
                        .text_xs()
                        .text_color(fg)
                        .whitespace_nowrap()
                        .child(col.name.clone()),
                ),
        )
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(fg)
                .whitespace_nowrap()
                .child(null_mark),
        )
        .child(
            div()
                .id(raw_type_element_id)
                .flex_none()
                .min_w_0()
                .max_w(px(96.0))
                .overflow_hidden()
                .text_ellipsis()
                .text_xs()
                .text_color(muted_fg)
                .whitespace_nowrap()
                .tooltip(move |window, cx| {
                    Tooltip::new(raw_type_for_tooltip.clone()).build(window, cx)
                })
                .child(col.data_type.raw_type.clone()),
        )
        .into_any_element()
}

/// 索引、外键等详情行：显示说明文本，复制可直接用于 SQL 的对象名。
pub(super) fn render_copyable_detail_line(
    element_id: SharedString,
    text: impl Into<SharedString>,
    copy_value: String,
    color: gpui_kit::Hsla,
    cx: &mut Context<TableTreePanel>,
) -> AnyElement {
    let row_height = ramag_ui::content_density(cx).row_height(28.0, 24.0);
    div()
        .id(element_id)
        .w_full()
        .h(px(row_height))
        .flex_none()
        .pl(px(56.0))
        .pr_2()
        .pt(px(6.0))
        .text_xs()
        .text_color(color)
        .whitespace_nowrap()
        .overflow_hidden()
        .text_ellipsis()
        .cursor_pointer()
        .on_click(cx.listener(move |_, event: &ClickEvent, window, cx| {
            if ramag_ui::is_primary_modifier_double_click(event) {
                ramag_ui::copy_text_with_notification(copy_value.clone(), window, cx);
            }
        }))
        .child(text.into())
        .into_any_element()
}

/// 根据加载错误状态选择主题色；占位文案缩进同列子节点并保持单行截断。
pub(super) fn render_columns_placeholder(
    text: impl Into<SharedString>,
    is_error: bool,
    cx: &mut Context<TableTreePanel>,
) -> AnyElement {
    let theme = cx.theme();
    let color = if is_error {
        theme.danger
    } else {
        theme.muted_foreground
    };
    let row_height = ramag_ui::content_density(cx).row_height(28.0, 24.0);
    div()
        .w_full()
        .h(px(row_height))
        .flex_none()
        .pl(px(56.0))
        .pr_2()
        .pt(px(6.0))
        .text_xs()
        .text_color(color)
        .whitespace_nowrap()
        .overflow_hidden()
        .text_ellipsis()
        .child(text.into())
        .into_any_element()
}

#[cfg(test)]
mod tests;
