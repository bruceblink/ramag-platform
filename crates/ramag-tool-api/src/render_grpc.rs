use super::*;

use gpui::{ClickEvent, FontWeight};
use gpui_component::{Disableable as _, button::ButtonVariants as _};

pub(super) fn render_editor(
    view: &ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
    let descriptor_label = match &view.grpc_descriptor {
        ApiGrpcDescriptor::Reflection => "Server Reflection".to_string(),
        ApiGrpcDescriptor::FileDescriptorSet { bytes } => {
            format!("FileDescriptorSet · {} bytes", bytes.len())
        }
    };

    v_flex()
        .id("api-grpc-fields")
        .debug_selector(|| "api-grpc-fields".into())
        .w_full()
        .min_w_0()
        .gap(px(10.0))
        .child(
            row().child(field(
                "Descriptor",
                h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().flex_1().min_w_0().text_sm().child(descriptor_label))
                    .child(
                        ramag_ui::clickable_button("api-grpc-import-proto")
                            .debug_selector(|| "api-grpc-import-proto".into())
                            .xsmall()
                            .ghost()
                            .label("导入 .proto")
                            .disabled(view.importing || view.grpc_discovering)
                            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                                view.import_grpc_proto(window, cx);
                            })),
                    )
                    .child(
                        ramag_ui::clickable_button("api-grpc-import-descriptor")
                            .debug_selector(|| "api-grpc-import-descriptor".into())
                            .xsmall()
                            .ghost()
                            .label("导入 DescriptorSet")
                            .disabled(view.importing || view.grpc_discovering)
                            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                                view.import_grpc_descriptor(window, cx);
                            })),
                    ),
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
        .child(view.auth_editor.render(cx, theme))
        .child(
            v_flex()
                .gap(px(5.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Protobuf JSON（流式请求每行一个对象）"),
                )
                .child(Input::new(&view.grpc_message).small()),
        )
        .child(render_catalog(view, cx, theme))
        .into_any_element()
}

fn render_catalog(
    view: &ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
    let mut catalog = v_flex()
        .id("api-grpc-catalog")
        .debug_selector(|| "api-grpc-catalog".into())
        .w_full()
        .min_w_0()
        .gap(px(6.0))
        .border_1()
        .border_color(theme.border)
        .rounded(px(5.0))
        .p(px(8.0))
        .child(
            h_flex()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Service / Method"),
                )
                .child(div().text_xs().text_color(theme.muted_foreground).child(
                    if view.grpc_discovering {
                        "发现中"
                    } else {
                        "Server Reflection"
                    },
                )),
        );

    if view.grpc_services.is_empty() {
        catalog = catalog.child(div().text_xs().text_color(theme.muted_foreground).child(
            if view.grpc_discovering {
                "正在读取 Service 目录"
            } else {
                "暂无 Service 目录"
            },
        ));
    } else {
        for (service_index, service) in view.grpc_services.iter().take(32).enumerate() {
            catalog = catalog.child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(service.name.clone()),
            );
            let mut methods = h_flex().w_full().min_w_0().flex_wrap().gap(px(4.0));
            for (method_index, method) in service.methods.iter().take(64).enumerate() {
                let service_name = service.name.clone();
                let method_name = method.name.clone();
                let method_kind = match (method.client_streaming, method.server_streaming) {
                    (false, false) => "Unary",
                    (false, true) => "Server",
                    (true, false) => "Client",
                    (true, true) => "Bidi",
                };
                methods = methods.child(
                    ramag_ui::clickable_button(gpui::SharedString::from(format!(
                        "api-grpc-method-{service_index}-{method_index}"
                    )))
                    .debug_selector(|| "api-grpc-method".into())
                    .xsmall()
                    .ghost()
                    .label(format!("{} · {method_kind}", method.name))
                    .on_click(cx.listener(
                        move |view, _: &ClickEvent, window, cx| {
                            view.grpc_service.update(cx, |input, cx| {
                                input.set_value(service_name.clone(), window, cx)
                            });
                            view.grpc_method.update(cx, |input, cx| {
                                input.set_value(method_name.clone(), window, cx)
                            });
                            view.notice = Some(("已选择 gRPC Method".into(), false));
                            cx.notify();
                        },
                    )),
                );
            }
            catalog = catalog.child(methods);
        }
    }
    catalog.into_any_element()
}
