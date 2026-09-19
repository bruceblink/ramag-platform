use super::*;

use gpui::{ClickEvent, FontWeight};
use gpui_component::{Disableable as _, button::ButtonVariants as _};

pub(super) fn render(
    view: &mut ApiView,
    window: &mut Window,
    cx: &mut Context<ApiView>,
) -> gpui::AnyElement {
    let theme = cx.theme().clone();
    let stacked = ApiView::is_stacked(window);
    let sidebar = render_sidebar(view, cx, &theme);
    let editor = render_editor(view, window, cx, &theme);
    let content = if stacked {
        v_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(sidebar)
            .child(editor)
    } else {
        h_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(sidebar)
            .child(editor)
    };
    v_flex()
        .id("api-root")
        .debug_selector(|| "api-root".into())
        .size_full()
        .min_w_0()
        .min_h_0()
        .bg(theme.background)
        .child(render_header(view, cx, &theme))
        .child(content)
        .into_any_element()
}

fn render_header(
    view: &mut ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
    let protocol_button = |id: &'static str,
                           label: &'static str,
                           protocol: ApiProtocol,
                           view: &mut ApiView,
                           cx: &mut Context<ApiView>| {
        let mut button = ramag_ui::clickable_button(id)
            .debug_selector(move || id.into())
            .xsmall()
            .label(label)
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                view.set_protocol(protocol, cx);
            }));
        button = if view.protocol == protocol {
            button.primary()
        } else {
            button.ghost()
        };
        button
    };
    let mut header = h_flex()
        .id("api-header")
        .debug_selector(|| "api-header".into())
        .w_full()
        .min_w_0()
        .flex_wrap()
        .items_center()
        .gap(px(8.0))
        .px(px(14.0))
        .py(px(10.0))
        .border_b_1()
        .border_color(theme.border)
        .child(
            v_flex()
                .flex_1()
                .min_w(px(160.0))
                .gap(px(2.0))
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("API 测试"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("HTTP 与 gRPC 请求工作区"),
                ),
        )
        .child(
            h_flex()
                .debug_selector(|| "api-protocol-switcher".into())
                .gap(px(4.0))
                .child(protocol_button(
                    "api-protocol-http",
                    "HTTP",
                    ApiProtocol::Http,
                    view,
                    cx,
                ))
                .child(protocol_button(
                    "api-protocol-grpc",
                    "gRPC",
                    ApiProtocol::Grpc,
                    view,
                    cx,
                )),
        );
    let save = ramag_ui::clickable_button("api-save")
        .debug_selector(|| "api-save".into())
        .xsmall()
        .label(if view.saving { "保存中" } else { "保存" })
        .disabled(view.saving || view.loading)
        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| view.save(cx)));
    let send = ramag_ui::clickable_button("api-send")
        .debug_selector(|| "api-send".into())
        .xsmall()
        .label(if view.loading { "发送中" } else { "发送" })
        .disabled(view.loading || view.saving)
        .primary()
        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| view.send(cx)));
    let cancel = ramag_ui::clickable_button("api-cancel")
        .debug_selector(|| "api-cancel".into())
        .xsmall()
        .label("取消")
        .disabled(!view.loading)
        .ghost()
        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| view.cancel(cx)));
    header = header.child(save).child(send).child(cancel);
    header.into_any_element()
}

fn render_sidebar(
    view: &ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
    let requests = view
        .workspace
        .collections
        .iter()
        .flat_map(|collection| collection.requests.iter())
        .take(50)
        .map(|request| {
            div()
                .debug_selector(|| "api-request-item".into())
                .min_w_0()
                .truncate()
                .child(format!(
                    "{} · {}",
                    protocol_label(request.protocol),
                    request.name
                ))
                .into_any_element()
        })
        .collect::<Vec<_>>();
    v_flex()
        .id("api-sidebar")
        .debug_selector(|| "api-sidebar".into())
        .w(px(API_SIDEBAR_WIDTH))
        .flex_none()
        .min_h_0()
        .min_w_0()
        .gap(px(10.0))
        .p(px(12.0))
        .border_r_1()
        .border_color(theme.border)
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("当前请求"),
        )
        .child(
            div()
                .debug_selector(|| "api-request-name-preview".into())
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .truncate()
                .child(view.request_name.read(cx).value()),
        )
        .child(
            div()
                .debug_selector(|| "api-request-protocol-preview".into())
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(match view.protocol {
                    ApiProtocol::Http => "HTTP 请求",
                    ApiProtocol::Grpc => "gRPC Unary 请求",
                }),
        )
        .child(div().h(px(1.0)).w_full().bg(theme.border))
        .child(
            v_flex()
                .gap(px(4.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("本地工作区"),
                )
                .child(div().text_sm().child(view.workspace.name.clone()))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "{} 个 Collection · {} 个请求",
                            view.workspace.collections.len(),
                            view.workspace
                                .collections
                                .iter()
                                .map(|collection| collection.requests.len())
                                .sum::<usize>()
                        )),
                ),
        )
        .child(
            v_flex()
                .gap(px(4.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("请求列表"),
                )
                .children(requests),
        )
        .into_any_element()
}

fn render_editor(
    view: &mut ApiView,
    _window: &mut Window,
    cx: &mut Context<ApiView>,
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
    let request_editor = match view.protocol {
        ApiProtocol::Http => render_http_editor(view, theme),
        ApiProtocol::Grpc => render_grpc_editor(view, theme),
    };
    v_flex()
        .id("api-editor")
        .debug_selector(|| "api-editor".into())
        .flex_1()
        .min_w_0()
        .min_h_0()
        .gap(px(10.0))
        .p(px(14.0))
        .child(
            v_flex()
                .id("api-request-editor")
                .debug_selector(|| "api-request-editor".into())
                .w_full()
                .min_w_0()
                .gap(px(10.0))
                .child(field("请求名称", Input::new(&view.request_name).small()))
                .child(request_editor),
        )
        .child(render_response(view, cx, theme))
        .into_any_element()
}

fn render_http_editor(view: &ApiView, theme: &gpui_component::Theme) -> gpui::AnyElement {
    v_flex()
        .id("api-http-fields")
        .debug_selector(|| "api-http-fields".into())
        .w_full()
        .min_w_0()
        .gap(px(10.0))
        .child(
            row()
                .child(field("Method", Input::new(&view.http_method).small()))
                .child(field("URL", Input::new(&view.http_url).small())),
        )
        .child(
            v_flex()
                .id("api-http-headers")
                .debug_selector(|| "api-http-headers".into())
                .gap(px(5.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Headers"),
                )
                .child(Input::new(&view.http_headers).small().h(px(112.0))),
        )
        .child(
            v_flex()
                .id("api-http-body")
                .debug_selector(|| "api-http-body".into())
                .gap(px(5.0))
                .child(
                    h_flex()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("Body"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("JSON"),
                        ),
                )
                .child(Input::new(&view.http_body).small().h(px(176.0))),
        )
        .into_any_element()
}

fn render_grpc_editor(view: &ApiView, theme: &gpui_component::Theme) -> gpui::AnyElement {
    v_flex()
        .id("api-grpc-fields")
        .debug_selector(|| "api-grpc-fields".into())
        .w_full()
        .min_w_0()
        .gap(px(10.0))
        .child(
            row()
                .child(field("Endpoint", Input::new(&view.grpc_endpoint).small()))
                .child(field("Service", Input::new(&view.grpc_service).small())),
        )
        .child(
            row()
                .child(field("Method", Input::new(&view.grpc_method).small()))
                .child(field(
                    "Descriptor",
                    div().text_sm().child("Server Reflection"),
                )),
        )
        .child(
            row()
                .child(field(
                    "Metadata name",
                    Input::new(&view.grpc_metadata_name).small(),
                ))
                .child(field(
                    "Metadata value",
                    Input::new(&view.grpc_metadata_value).small(),
                )),
        )
        .child(
            v_flex()
                .gap(px(5.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Protobuf JSON"),
                )
                .child(Input::new(&view.grpc_message).small()),
        )
        .into_any_element()
}

fn render_response(
    view: &ApiView,
    _cx: &mut Context<ApiView>,
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
    let body = match &view.response {
        Some(snapshot) => v_flex()
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
            .child(response_parameters(snapshot, theme))
            .child(
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
                    ),
            )
            .into_any_element(),
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

fn protocol_label(protocol: ApiProtocol) -> &'static str {
    match protocol {
        ApiProtocol::Http => "HTTP",
        ApiProtocol::Grpc => "gRPC",
    }
}

fn response_parameters(
    snapshot: &ApiResponseSnapshot,
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
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
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
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
