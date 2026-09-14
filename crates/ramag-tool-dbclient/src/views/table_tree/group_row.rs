//! 表和视图分组标题的交互渲染。

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::*, px,
};
use gpui_component::{Icon, IconName, Sizable as _, h_flex};

use super::TableTreePanel;

/// Render a clickable tables/views group header and persist its collapsed state.
pub(super) fn render(
    schema: &str,
    is_view: bool,
    text: &str,
    is_expanded: bool,
    muted_fg: gpui::Hsla,
    muted_bg: gpui::Hsla,
    cx: &mut Context<TableTreePanel>,
) -> AnyElement {
    let row_id = SharedString::from(format!(
        "table-group-{schema}-{}",
        if is_view { "views" } else { "tables" }
    ));
    let schema_for_click = schema.to_string();
    let chevron = if is_expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    let folder = if is_expanded {
        IconName::FolderOpen
    } else {
        IconName::FolderClosed
    };

    h_flex()
        .id(row_id.clone())
        .debug_selector(move || row_id.to_string())
        .w_full()
        .h(px(28.0))
        .flex_none()
        .items_center()
        .gap_1()
        .pl(px(20.0))
        .pr_2()
        .rounded_md()
        .cursor_pointer()
        .hover(move |this| this.bg(muted_bg))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.toggle_table_group(schema_for_click.clone(), is_view, cx);
        }))
        .text_xs()
        .text_color(muted_fg)
        .child(
            div()
                .w(px(14.0))
                .child(Icon::new(chevron).xsmall().text_color(muted_fg)),
        )
        .child(Icon::new(folder).small().text_color(muted_fg))
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(text.to_string()),
        )
        .into_any_element()
}
