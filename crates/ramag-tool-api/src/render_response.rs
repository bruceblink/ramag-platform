use super::render_helpers::{
    render_assertion_results, render_collection_summary, render_extracted_variables,
};
use super::*;
use gpui_kit::ClickEvent;
use gpui_kit::FontWeight;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{Icon, IconName, button::ButtonVariants as _};

pub(super) fn render_response(
    view: &ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let body = match &view.response {
        Some(snapshot) => {
            let selected = match view.response_tab {
                ApiResponseTab::Body => render_response_body(snapshot, cx, theme),
                ApiResponseTab::Headers => response_parameters(snapshot, theme),
                ApiResponseTab::Timing => render_response_timing(snapshot, theme),
                ApiResponseTab::Assertions => render_response_assertions(view, theme),
            };
            v_flex()
                .id("api-response-content")
                .debug_selector(|| "api-response-content".into())
                .w_full()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .gap(px(8.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(status_text(snapshot)),
                )
                .child(selected)
                .into_any_element()
        }
        None => v_flex()
            .id("api-response-empty")
            .debug_selector(|| "api-response-empty".into())
            .flex_1()
            .min_w_0()
            .min_h_0()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .child(
                div().p(px(14.0)).rounded_full().bg(theme.secondary).child(
                    Icon::new(IconName::File)
                        .size(px(26.0))
                        .text_color(theme.muted_foreground),
                ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(if view.loading {
                        "正在等待响应…"
                    } else {
                        "尚未发送"
                    }),
            )
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
        .child(render_scrollbar_overlay(
            &view.response_scroll,
            "api-response-scrollbar",
        ))
        .gap(px(6.0))
        .pl(px(8.0))
        .py(px(8.0))
        .border_l_1()
        .border_color(theme.border)
        .child(
            h_flex()
                .id("api-response-heading")
                .debug_selector(|| "api-response-heading".into())
                .min_w_0()
                .items_center()
                .gap(px(8.0))
                .pb(px(6.0))
                .border_b_1()
                .border_color(theme.border)
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("响应"),
                )
                .child(render_response_tabs(view, cx))
                .child(
                    div()
                        .max_w(px(120.0))
                        .truncate()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(match &view.notice {
                            Some((message, _)) => message.clone(),
                            None => "".into(),
                        }),
                ),
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
        .flex_1()
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
                        .label("响应头")
                        .debug_selector(|| "api-response-tab-headers".into()),
                )
                .child(
                    Tab::new()
                        .label("耗时")
                        .debug_selector(|| "api-response-tab-timing".into()),
                )
                .child(
                    Tab::new()
                        .label("测试")
                        .debug_selector(|| "api-response-tab-assertions".into()),
                ),
        )
        .into_any_element()
}

fn render_response_body(
    snapshot: &ApiResponseSnapshot,
    cx: &mut Context<ApiView>,
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
                .items_center()
                .gap(px(6.0))
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("正文"),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(body_format_label(snapshot)),
                )
                .child(
                    ramag_ui::clickable_button("api-response-copy")
                        .debug_selector(|| "api-response-copy".into())
                        .xsmall()
                        .ghost()
                        .icon(IconName::Copy)
                        .tooltip(if snapshot.truncated {
                            "复制已保留正文（正文已截断）"
                        } else {
                            "复制正文"
                        })
                        .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                            view.copy_response_body(window, cx);
                        })),
                ),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .px(px(8.0))
                .py(px(6.0))
                .bg(theme.background)
                .text_xs()
                .font_family(theme.mono_font_family.clone())
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
            "响应头",
            &snapshot.headers,
            theme,
        ))
        .child(response_parameter_table(
            "api-response-metadata",
            "元数据",
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
