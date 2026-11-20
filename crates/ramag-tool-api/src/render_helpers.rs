use super::*;

use gpui_kit::ClickEvent;
use gpui_kit::FontWeight;
use gpui_kit::component::{Disableable as _, IconName, button::ButtonVariants as _};

pub(super) fn render_collection_button(
    view: &ApiView,
    cx: &mut Context<ApiView>,
) -> gpui_kit::AnyElement {
    ramag_ui::clickable_button("api-run-collection")
        .debug_selector(|| "api-run-collection".into())
        .xsmall()
        .label("运行 Collection")
        .disabled(view.request_execution_disabled())
        .ghost()
        .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
            view.run_collection(window, cx);
        }))
        .into_any_element()
}

/// 请求目标与执行操作独立成工作台顶栏，保留原有 HTTP、gRPC 和 Collection 流程。
pub(super) fn render_request_toolbar(
    view: &mut ApiView,
    window: &Window,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let send = ramag_ui::clickable_button("api-send")
        .debug_selector(|| "api-send".into())
        .xsmall()
        .label(if view.loading { "发送中" } else { "发送" })
        .disabled(view.request_execution_disabled())
        .primary()
        .on_click(cx.listener(|view, _: &ClickEvent, window, cx| view.send(window, cx)));
    let discover = ramag_ui::clickable_button("api-grpc-discover")
        .debug_selector(|| "api-grpc-discover".into())
        .xsmall()
        .label(if view.grpc_discovering {
            "发现中"
        } else {
            "发现"
        })
        .disabled(
            view.protocol != ApiProtocol::Grpc
                || view.grpc_discovering
                || view.loading
                || view.saving
                || view.importing,
        )
        .ghost()
        .on_click(cx.listener(|view, _: &ClickEvent, window, cx| view.discover_grpc(window, cx)));
    let cancel = ramag_ui::clickable_button("api-cancel")
        .debug_selector(|| "api-cancel".into())
        .xsmall()
        .label("取消")
        .disabled(!view.loading && !view.grpc_discovering)
        .ghost()
        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| view.cancel(cx)));
    v_flex()
        .id("api-request-toolbar")
        .debug_selector(|| "api-request-toolbar".into())
        .w_full()
        .min_w_0()
        .flex_none()
        .gap(px(4.0))
        .px(px(12.0))
        .py(px(8.0))
        .border_b_1()
        .border_color(theme.border)
        .child(
            h_flex()
                .id("api-request-command-row")
                .debug_selector(|| "api-request-command-row".into())
                .w_full()
                .min_w_0()
                .flex_wrap()
                .items_center()
                .gap(px(6.0))
                .child(render_request_target(
                    view,
                    ApiView::main_content_width(window) < API_REQUEST_TARGET_COMPACT_BREAKPOINT,
                ))
                .child(
                    h_flex()
                        .id("api-request-actions")
                        .debug_selector(|| "api-request-actions".into())
                        .flex_none()
                        .gap(px(6.0))
                        .when(view.protocol == ApiProtocol::Grpc, |actions| {
                            actions.child(discover)
                        })
                        .child(send)
                        .when(view.loading || view.grpc_discovering, |actions| {
                            actions.child(cancel)
                        })
                        .child(render_collection_button(view, cx)),
                ),
        )
        .into_any_element()
}

fn render_request_target(view: &ApiView, compact: bool) -> gpui_kit::AnyElement {
    let (target, minimum_width) = match view.protocol {
        ApiProtocol::Http => (
            h_flex()
                .w_full()
                .min_w_0()
                .gap(px(8.0))
                .child(
                    request_target_field(
                        "api-http-method",
                        Input::new(&view.http_method).small().w_full(),
                    )
                    .w(px(92.0))
                    .flex_none(),
                )
                .child(
                    request_target_field(
                        "api-http-url",
                        Input::new(&view.http_url).small().w_full(),
                    )
                    .flex_1()
                    .min_w(px(180.0)),
                )
                .into_any_element(),
            280.0,
        ),
        ApiProtocol::Grpc => {
            let field_minimum = if compact { 88.0 } else { 160.0 };
            let endpoint_minimum = if compact { field_minimum } else { 180.0 };
            let service_minimum = field_minimum;
            let method_minimum = if compact { field_minimum } else { 140.0 };
            let method = request_target_field(
                "api-grpc-method-input",
                Input::new(&view.grpc_method).small().w_full(),
            );
            let method = if compact {
                method.flex_1().min_w(px(method_minimum))
            } else {
                method.w(px(method_minimum)).flex_none()
            };
            let minimum_width = endpoint_minimum + service_minimum + method_minimum + 16.0;
            (
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap(px(8.0))
                    .child(
                        request_target_field(
                            "api-grpc-endpoint",
                            Input::new(&view.grpc_endpoint).small().w_full(),
                        )
                        .flex_1()
                        .min_w(px(endpoint_minimum)),
                    )
                    .child(
                        request_target_field(
                            "api-grpc-service",
                            Input::new(&view.grpc_service).small().w_full(),
                        )
                        .flex_1()
                        .min_w(px(service_minimum)),
                    )
                    .child(method)
                    .into_any_element(),
                minimum_width,
            )
        }
    };
    h_flex()
        .id("api-request-target")
        .debug_selector(|| "api-request-target".into())
        .flex_1()
        .min_w_0()
        .min_w(px(minimum_width))
        .child(target)
        .into_any_element()
}

/// 为真实输入框提供可测量边界，保持窄屏各字段的最小可用宽度。
fn request_target_field(
    id: &'static str,
    input: impl IntoElement,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .min_w_0()
        .child(input)
}

pub(super) fn render_collection_summary(
    view: &ApiView,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let label = match &view.last_collection_run {
        Some(summary) => format!(
            "Collection：{} · {} 通过 · {} 失败 · {} 取消{}",
            summary.collection_name,
            summary.passed,
            summary.failed,
            summary.cancelled,
            if summary.history_persist_failed {
                " · 历史写入失败，已停止"
            } else if summary.stopped {
                " · 已停止"
            } else {
                ""
            }
        ),
        None => "Collection 尚未运行".into(),
    };
    div()
        .id("api-collection-summary")
        .debug_selector(|| "api-collection-summary".into())
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(label)
        .into_any_element()
}

pub(super) fn render_context_editor(
    view: &ApiView,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
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
                .child("环境变量 / 断言"),
        )
        .child(
            row()
                .child(field(
                    "环境变量",
                    Textarea::new(&view.environment_variables).h(px(78.0)),
                ))
                .child(field(
                    "敏感变量名",
                    Textarea::new(&view.environment_sensitive).h(px(78.0)),
                ))
                .child(field("断言", Textarea::new(&view.assertions).h(px(78.0)))),
        )
        .child(
            v_flex()
                .id("api-response-variables")
                .debug_selector(|| "api-response-variables".into())
                .gap(px(5.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("响应变量"),
                )
                .child(Textarea::new(&view.response_variables).h(px(96.0))),
        )
        .child(render_tls_editor(view, theme))
        .child(render_proxy_editor(view, theme))
        .into_any_element()
}

pub(super) fn render_tls_editor(
    view: &ApiView,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    v_flex()
        .id("api-tls-editor")
        .debug_selector(|| "api-tls-editor".into())
        .gap(px(5.0))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("TLS / mTLS（路径留在本机，不会写入请求日志）"),
        )
        .child(
            row()
                .child(field("校验模式", Input::new(&view.tls_verify).small()))
                .child(field(
                    "CA 证书路径",
                    Input::new(&view.tls_ca_cert_path).small(),
                )),
        )
        .child(
            row()
                .child(field(
                    "客户端证书路径",
                    Input::new(&view.tls_client_cert_path).small(),
                ))
                .child(field(
                    "客户端密钥路径",
                    Input::new(&view.tls_client_key_path).small(),
                )),
        )
        .into_any_element()
}

pub(super) fn render_proxy_editor(
    view: &ApiView,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    v_flex()
        .id("api-proxy-editor")
        .debug_selector(|| "api-proxy-editor".into())
        .gap(px(5.0))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("显式 HTTP 代理（HTTPS/gRPC 使用 CONNECT；不读取系统代理）"),
        )
        .child(
            row()
                .child(field("代理 URL", Input::new(&view.proxy_url).small()))
                .child(field(
                    "代理用户名",
                    Input::new(&view.proxy_username).small(),
                )),
        )
        .child(field(
            "代理密码",
            Input::new(&view.proxy_password).small().mask_toggle(),
        ))
        .into_any_element()
}

pub(super) fn render_assertion_results(
    results: &[ApiAssertionResult],
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
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

pub(super) fn render_extracted_variables(
    variables: &[ApiExtractedVariable],
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let mut section = v_flex()
        .id("api-extracted-variables")
        .debug_selector(|| "api-extracted-variables".into())
        .gap(px(3.0))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!("已提取变量 · {} 项", variables.len())),
        );
    if variables.is_empty() {
        section = section.child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("未提取响应变量"),
        );
    } else {
        for variable in variables {
            section = section.child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(if variable.sensitive {
                        format!("{} · 已写入（敏感）", variable.name)
                    } else {
                        format!("{} · 已写入", variable.name)
                    }),
            );
        }
    }
    section.into_any_element()
}

pub(super) fn render_history(
    view: &ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let history = &view.history;
    let mut section = v_flex()
        .id("api-history")
        .debug_selector(|| "api-history".into())
        .gap(px(4.0))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("执行历史"),
                )
                .child(
                    ramag_ui::clickable_button("api-history-clear")
                        .debug_selector(|| "api-history-clear".into())
                        .small()
                        .ghost()
                        .icon(IconName::Delete)
                        .label("清空")
                        .tooltip("清空执行历史")
                        .disabled(
                            view.service.is_none()
                                || view.history.is_empty()
                                || view.clearing_history
                                || view.workspace_load_state.save_block_message().is_some(),
                        )
                        .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                            view.confirm_clear_history(window, cx);
                        })),
                ),
        );
    if history.is_empty() {
        section = section.child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("暂无执行记录"),
        );
    } else {
        for (index, item) in history.iter().take(8).enumerate() {
            let result = if item.passed {
                "通过"
            } else if item.assertion_failed > 0 && item.status.is_some() {
                "断言失败"
            } else {
                "失败"
            };
            let selector = format!("api-history-item-{index}");
            section = section.child(
                div()
                    .debug_selector(move || selector.clone())
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(if item.passed {
                        theme.success
                    } else {
                        theme.muted_foreground
                    })
                    .child(format!(
                        "{} · {} · {} · {} · {} ms",
                        result,
                        history_protocol_label(item.protocol),
                        item.request_name,
                        history_status_label(item.status.as_ref()),
                        item.elapsed_millis
                    )),
            );
        }
    }
    section.into_any_element()
}

fn history_status_label(status: Option<&ApiResponseStatus>) -> String {
    match status {
        Some(ApiResponseStatus::Http { code }) => format!("HTTP {code}"),
        Some(ApiResponseStatus::Grpc { code }) => format!("gRPC {code}"),
        Some(ApiResponseStatus::TransportError) => "传输错误".into(),
        None => "无响应".into(),
    }
}

fn history_protocol_label(protocol: ApiProtocol) -> &'static str {
    match protocol {
        ApiProtocol::Http => "HTTP",
        ApiProtocol::Grpc => "gRPC",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_status_uses_user_facing_protocol_labels() {
        assert_eq!(
            history_status_label(Some(&ApiResponseStatus::Http { code: 200 })),
            "HTTP 200"
        );
        assert_eq!(
            history_status_label(Some(&ApiResponseStatus::Grpc { code: "ok".into() })),
            "gRPC ok"
        );
        assert_eq!(
            history_status_label(Some(&ApiResponseStatus::TransportError)),
            "传输错误"
        );
        assert_eq!(history_status_label(None), "无响应");
        assert_eq!(history_protocol_label(ApiProtocol::Http), "HTTP");
        assert_eq!(history_protocol_label(ApiProtocol::Grpc), "gRPC");
    }
}
