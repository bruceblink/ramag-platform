//! 表属性中的触发器元数据列表。

use gpui::{AnyElement, ParentElement, SharedString, Styled, div, prelude::*, px};
use gpui_component::{Icon, IconName, Sizable as _, Theme, h_flex, v_flex};
use ramag_domain::entities::Trigger;

use super::TablePropertiesDialog;

/// Render the trigger metadata above the DDL preview.
/// The list is independently scrollable so a long trigger definition never hides the DDL.
pub(super) fn render(dialog: &TablePropertiesDialog, theme: &Theme) -> AnyElement {
    let body = if dialog.triggers_loading {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child("正在读取触发器…")
            .into_any_element()
    } else if let Some(error) = &dialog.triggers_error {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .text_color(theme.danger)
            .child(error.clone())
            .into_any_element()
    } else if let Some(triggers) = &dialog.triggers {
        let content = if triggers.is_empty() {
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("没有触发器")
                .into_any_element()
        } else {
            v_flex()
                .w_full()
                .gap(px(1.0))
                .children(
                    triggers
                        .iter()
                        .enumerate()
                        .map(|(index, trigger)| render_row(index, trigger, theme)),
                )
                .into_any_element()
        };
        div()
            .id("table-properties-triggers-scroll")
            .debug_selector(|| "table-properties-triggers-scroll".into())
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&dialog.triggers_vertical_scroll)
            .child(content)
            .into_any_element()
    } else {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child("暂无触发器信息")
            .into_any_element()
    };

    v_flex()
        .id("table-properties-triggers")
        .debug_selector(|| "table-properties-triggers".into())
        .w_full()
        .h(px(172.0))
        .flex_none()
        .min_h_0()
        .overflow_hidden()
        .border_1()
        .border_color(theme.border)
        .rounded(px(5.0))
        .bg(theme.secondary)
        .child(
            h_flex()
                .w_full()
                .h(px(32.0))
                .flex_none()
                .items_center()
                .gap(px(6.0))
                .px(px(8.0))
                .bg(theme.muted.opacity(0.45))
                .border_b_1()
                .border_color(theme.border)
                .child(
                    Icon::new(IconName::Network)
                        .small()
                        .text_color(theme.accent),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("触发器"),
                )
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(dialog.triggers.as_ref().map_or_else(
                            || "-".to_string(),
                            |triggers| triggers.len().to_string(),
                        )),
                ),
        )
        .child(div().flex_1().min_h_0().child(body))
        .into_any_element()
}

fn render_row(index: usize, trigger: &Trigger, theme: &Theme) -> AnyElement {
    let selector = SharedString::from(format!("table-properties-trigger-{index}"));
    let definition = if trigger.definition.trim().is_empty() {
        "（无定义）".to_string()
    } else {
        trigger.definition.clone()
    };
    v_flex()
        .id(selector.clone())
        .debug_selector(move || selector.to_string())
        .w_full()
        .gap(px(3.0))
        .px(px(10.0))
        .py(px(6.0))
        .border_b_1()
        .border_color(theme.border.opacity(0.45))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .items_center()
                .gap(px(8.0))
                .child(
                    Icon::new(IconName::Network)
                        .small()
                        .text_color(theme.warning),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(theme.foreground)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(trigger.name.clone()),
                )
                .child(
                    div()
                        .w(px(96.0))
                        .flex_none()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(trigger.timing.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(theme.info)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(trigger.event.clone()),
                ),
        )
        .child(
            div()
                .w_full()
                .pl(px(28.0))
                .font_family(theme.mono_font_family.clone())
                .text_xs()
                .text_color(theme.muted_foreground)
                .whitespace_normal()
                .child(definition),
        )
        .into_any_element()
}
