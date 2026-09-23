use super::render_helpers::{
    render_assertion_results, render_collection_summary, render_extracted_variables,
};
use super::*;
use gpui_kit::FontWeight;
use gpui_kit::component::tab::{Tab, TabBar};

pub(super) fn render_response(
    view: &ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let body = match &view.response {
        Some(snapshot) => {
            let selected = match view.response_tab {
                ApiResponseTab::Body => render_response_body(snapshot, theme),
                ApiResponseTab::Headers => response_parameters(snapshot, theme),
                ApiResponseTab::Timing => render_response_timing(snapshot, theme),
                ApiResponseTab::Assertions => render_response_assertions(view, theme),
            };
            v_flex()
                .id("api-response-content")
                .debug_selector(|| "api-response-content".into())
                .w_full()
                .min_w_0()
                .gap(px(8.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(status_text(snapshot)),
                )
                .child(render_response_tabs(view, cx))
                .child(selected)
                .into_any_element()
        }
        None => v_flex()
            .id("api-response-empty")
            .debug_selector(|| "api-response-empty".into())
            .w_full()
            .p(px(14.0))
            .child(if view.loading {
                "正在等待响应…"
            } else {
                "发送请求后显示响应"
            })
            .into_any_element(),
    };
    v_flex()
        .id("api-response")
        .debug_selector(|| "api-response".into())
        .flex_1()
        .h_full()
        .min_h(px(160.0))
        .min_w_0()
        .overflow_y_scroll()
        .track_scroll(&view.response_scroll)
        .gap(px(8.0))
        .p(px(12.0))
        .border_1()
        .border_color(theme.border)
        .rounded(px(6.0))
        .child(
            h_flex()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("响应"),
                )
                .child(div().text_xs().text_color(theme.muted_foreground).child(
                    match &view.notice {
                        Some((message, _)) => message.clone(),
                        None => "".into(),
                    },
                )),
        )
        .child(body)
        .into_any_element()
}

fn render_response_tabs(view: &ApiView, cx: &mut Context<ApiView>) -> gpui_kit::AnyElement {
    div()
        .id("api-response-tabs")
        .debug_selector(|| "api-response-tabs".into())
        .w_full()
        .min_w_0()
        .flex_none()
        .child(
            TabBar::new("api-response-tab-bar")
                .underline()
                .w_full()
                .min_w_0()
                .selected_index(view.response_tab.index())
                .on_click(cx.listener(|view, index: &usize, _, cx| {
                    view.response_tab = ApiResponseTab::from_index(*index);
                    cx.notify();
                }))
                .child(
                    Tab::new()
                        .label("正文")
                        .debug_selector(|| "api-response-tab-body".into()),
                )
                .child(
                    Tab::new()
                        .label("Headers / Metadata")
                        .debug_selector(|| "api-response-tab-headers".into()),
                )
                .child(
                    Tab::new()
                        .label("耗时")
                        .debug_selector(|| "api-response-tab-timing".into()),
                )
                .child(
                    Tab::new()
                        .label("断言 / 变量")
                        .debug_selector(|| "api-response-tab-assertions".into()),
                ),
        )
        .into_any_element()
}

fn render_response_body(
    snapshot: &ApiResponseSnapshot,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    v_flex()
        .id("api-response-body")
        .debug_selector(|| "api-response-body".into())
        .w_full()
        .min_w_0()
        .gap(px(5.0))
        .child(
            h_flex()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("正文"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(body_format_label(snapshot)),
                ),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .p(px(10.0))
                .bg(theme.secondary)
                .rounded(px(5.0))
                .text_xs()
                .whitespace_normal()
                .child(body_preview(snapshot)),
        )
        .into_any_element()
}

fn render_response_timing(
    snapshot: &ApiResponseSnapshot,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let mut section = v_flex()
        .id("api-response-timing")
        .debug_selector(|| "api-response-timing".into())
        .w_full()
        .min_w_0()
        .gap(px(5.0))
        .child(response_detail("状态", status_text(snapshot), theme))
        .child(response_detail(
            "耗时",
            format!("{} ms", snapshot.elapsed_millis),
            theme,
        ))
        .child(response_detail(
            "响应大小",
            format!("{} bytes", snapshot.size_bytes),
            theme,
        ))
        .child(response_detail(
            "正文状态",
            if snapshot.truncated {
                "正文已截断".into()
            } else {
                "正文完整".into()
            },
            theme,
        ));
    if let Some(error) = &snapshot.error {
        section = section.child(
            div()
                .text_xs()
                .text_color(theme.danger)
                .whitespace_normal()
                .child(format!("错误：{error}")),
        );
    }
    section.into_any_element()
}

fn response_detail(
    label: &'static str,
    value: String,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    h_flex()
        .w_full()
        .min_w_0()
        .gap(px(8.0))
        .child(
            div()
                .w(px(96.0))
                .flex_none()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_xs()
                .whitespace_normal()
                .child(value),
        )
        .into_any_element()
}

fn render_response_assertions(
    view: &ApiView,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    v_flex()
        .id("api-response-assertions")
        .debug_selector(|| "api-response-assertions".into())
        .w_full()
        .min_w_0()
        .gap(px(8.0))
        .child(render_collection_summary(view, theme))
        .child(render_assertion_results(&view.assertion_results, theme))
        .child(render_extracted_variables(&view.extracted_variables, theme))
        .into_any_element()
}

fn response_parameters(
    snapshot: &ApiResponseSnapshot,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    v_flex()
        .id("api-response-parameters")
        .debug_selector(|| "api-response-parameters".into())
        .w_full()
        .min_w_0()
        .gap(px(8.0))
        .child(response_parameter_table(
            "api-response-headers",
            "Headers",
            &snapshot.headers,
            theme,
        ))
        .child(response_parameter_table(
            "api-response-metadata",
            "Metadata",
            &snapshot.metadata,
            theme,
        ))
        .into_any_element()
}

fn response_parameter_table(
    id: &'static str,
    label: &'static str,
    parameters: &[ApiParameter],
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let mut section = v_flex()
        .id(id)
        .debug_selector(move || id.into())
        .w_full()
        .min_w_0()
        .gap(px(3.0))
        .child(
            h_flex()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(label),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("{} 项", parameters.len())),
                ),
        );

    if parameters.is_empty() {
        section = section.child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(format!("无 {}", label)),
        );
    } else {
        section = section.child(
            h_flex()
                .w_full()
                .min_w_0()
                .gap(px(8.0))
                .child(
                    div()
                        .w(px(150.0))
                        .flex_none()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("名称"),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("值"),
                ),
        );
        for parameter in parameters.iter().take(16) {
            section = section.child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(150.0))
                            .flex_none()
                            .truncate()
                            .text_xs()
                            .child(parameter.name.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(parameter.value.clone()),
                    ),
            );
        }
        if parameters.len() > 16 {
            section = section.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("…其余 {} 项未展开", parameters.len() - 16)),
            );
        }
    }
    section.into_any_element()
}
