use super::*;

use gpui::FontWeight;

pub(super) fn render_context_editor(
    view: &ApiView,
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
    v_flex()
        .id("api-context-editor")
        .debug_selector(|| "api-context-editor".into())
        .w_full()
        .min_w_0()
        .gap(px(8.0))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("Environment / Assertions"),
        )
        .child(
            row()
                .child(field(
                    "环境变量",
                    Input::new(&view.environment_variables).small().h(px(78.0)),
                ))
                .child(field(
                    "敏感变量名",
                    Input::new(&view.environment_sensitive).small().h(px(78.0)),
                ))
                .child(field(
                    "断言",
                    Input::new(&view.assertions).small().h(px(78.0)),
                )),
        )
        .into_any_element()
}

pub(super) fn render_assertion_results(
    results: &[ApiAssertionResult],
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
    let mut section = v_flex()
        .id("api-assertion-results")
        .debug_selector(|| "api-assertion-results".into())
        .gap(px(3.0))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!("断言结果 · {} 项", results.len())),
        );
    if results.is_empty() {
        section = section.child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("未配置断言"),
        );
    } else {
        for result in results {
            section = section.child(
                div()
                    .text_xs()
                    .text_color(if result.passed {
                        theme.success
                    } else {
                        theme.danger
                    })
                    .child(format!(
                        "{} {}",
                        if result.passed {
                            "[通过]"
                        } else {
                            "[失败]"
                        },
                        result.message
                    )),
            );
        }
    }
    section.into_any_element()
}

pub(super) fn render_history(
    history: &[ApiHistoryRecord],
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
    let mut section = v_flex()
        .id("api-history")
        .debug_selector(|| "api-history".into())
        .gap(px(4.0))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("执行历史"),
        );
    if history.is_empty() {
        section = section.child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("暂无执行记录"),
        );
    } else {
        for item in history.iter().take(8) {
            let status = item
                .status
                .as_ref()
                .map(|status| format!("{status:?}"))
                .unwrap_or_else(|| "失败".into());
            section = section.child(
                div()
                    .debug_selector(|| "api-history-item".into())
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(if item.passed {
                        theme.success
                    } else {
                        theme.muted_foreground
                    })
                    .child(format!(
                        "{} · {} · {} ms",
                        if item.passed { "通过" } else { "失败" },
                        status,
                        item.elapsed_millis
                    )),
            );
        }
    }
    section.into_any_element()
}
