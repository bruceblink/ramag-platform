use super::*;

use gpui_kit::ClickEvent;
use gpui_kit::FontWeight;
use gpui_kit::component::{Disableable as _, IconName, button::ButtonVariants as _};
use ramag_ui::PointerDropdownMenu as _;

pub(super) fn render_collection_button(
    view: &ApiView,
    cx: &mut Context<ApiView>,
) -> gpui_kit::AnyElement {
    ramag_ui::clickable_button("api-run-collection")
        .debug_selector(|| "api-run-collection".into())
        .xsmall()
        .label("运行 Collection")
        .disabled(view.loading || view.saving || view.importing || view.grpc_discovering)
        .ghost()
        .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
            view.run_collection(window, cx);
        }))
        .into_any_element()
}

pub(super) fn render_request_toolbar(
    view: &mut ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
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
    let save = ramag_ui::clickable_button("api-save")
        .debug_selector(|| "api-save".into())
        .xsmall()
        .label(if view.saving { "保存中" } else { "保存" })
        .disabled(
            view.workspace_load_state.save_block_message().is_some()
                || view.saving
                || view.importing
                || view.loading
                || view.grpc_discovering,
        )
        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| view.save(cx)));
    let import = {
        let current_protocol = view.protocol;
        let view_entity = cx.entity();
        ramag_ui::clickable_button("api-import")
            .debug_selector(|| "api-import".into())
            .xsmall()
            .label(if view.importing {
                "导入中"
            } else {
                "导入"
            })
            .disabled(
                view.workspace_load_state.save_block_message().is_some()
                    || view.importing
                    || view.saving
                    || view.loading
                    || view.grpc_discovering,
            )
            .ghost()
            .dropdown_caret(true)
            .pointer_dropdown_menu(move |mut menu, _, _| {
                let view = view_entity.clone();
                menu = menu.item(ramag_ui::menu_item("导入工作区 JSON").on_click(
                    move |_: &ClickEvent, window, cx| {
                        view.update(cx, |view, cx| view.import(window, cx));
                    },
                ));
                if current_protocol == ApiProtocol::Grpc {
                    let view = view_entity.clone();
                    menu = menu.item(ramag_ui::menu_item("导入 .proto").on_click(
                        move |_: &ClickEvent, window, cx| {
                            view.update(cx, |view, cx| view.import_grpc_proto(window, cx));
                        },
                    ));
                    let view = view_entity.clone();
                    menu = menu.item(ramag_ui::menu_item("导入 DescriptorSet").on_click(
                        move |_: &ClickEvent, window, cx| {
                            view.update(cx, |view, cx| view.import_grpc_descriptor(window, cx));
                        },
                    ));
                }
                menu
            })
    };
    let send = ramag_ui::clickable_button("api-send")
        .debug_selector(|| "api-send".into())
        .xsmall()
        .label(if view.loading { "发送中" } else { "发送" })
        .disabled(view.loading || view.saving || view.importing || view.grpc_discovering)
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
        .gap(px(8.0))
        .px(px(14.0))
        .py(px(10.0))
        .border_b_1()
        .border_color(theme.border)
        .child(
            h_flex()
                .id("api-request-tabbar")
                .debug_selector(|| "api-request-tabbar".into())
                .w_full()
                .min_w_0()
                .gap(px(8.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("当前请求"),
                )
                .child(Input::new(&view.request_name).small().w(px(240.0)))
                .child(div().flex_1().min_w_0())
                .child(
                    h_flex()
                        .id("api-protocol-switcher")
                        .debug_selector(|| "api-protocol-switcher".into())
                        .flex_none()
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
                ),
        )
        .child(
            h_flex()
                .id("api-request-command-row")
                .debug_selector(|| "api-request-command-row".into())
                .w_full()
                .min_w_0()
                .flex_wrap()
                .items_center()
                .gap(px(6.0))
                .child(render_request_target(view))
                .child(
                    h_flex()
                        .id("api-request-actions")
                        .debug_selector(|| "api-request-actions".into())
                        .flex_none()
                        .gap(px(6.0))
                        .child(import)
                        .child(save)
                        .child(discover)
                        .child(send)
                        .child(cancel)
                        .child(render_collection_button(view, cx)),
                ),
        )
        .into_any_element()
}

fn render_request_target(view: &ApiView) -> gpui_kit::AnyElement {
    let target = match view.protocol {
        ApiProtocol::Http => h_flex()
            .w_full()
            .min_w_0()
            .child(Input::new(&view.http_method).small().w(px(92.0)))
            .child(Input::new(&view.http_url).small().flex_1().min_w(px(180.0))),
        ApiProtocol::Grpc => h_flex()
            .w_full()
            .min_w_0()
            .child(
                Input::new(&view.grpc_endpoint)
                    .small()
                    .flex_1()
                    .min_w(px(180.0)),
            )
            .child(
                Input::new(&view.grpc_service)
                    .small()
                    .flex_1()
                    .min_w(px(160.0)),
            )
            .child(Input::new(&view.grpc_method).small().w(px(140.0))),
    };
    h_flex()
        .id("api-request-target")
        .debug_selector(|| "api-request-target".into())
        .flex_1()
        .min_w_0()
        .gap(px(8.0))
        .child(target)
        .into_any_element()
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
                .child("Environment / Assertions"),
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
