//! API 工作区页面标题与顶部操作区。

use super::*;

use gpui_kit::ClickEvent;
use gpui_kit::component::{Disableable as _, Selectable as _, button::ButtonVariants as _};
use ramag_ui::PointerDropdownMenu as _;

/// 把请求名称和工作区上下文提升到页面标题区，同时保留协议、导入和保存入口。
pub(super) fn render_request_heading(
    view: &mut ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
    compact: bool,
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
            button.secondary().selected(true)
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
    let protocol = match view.protocol {
        ApiProtocol::Http => "HTTP",
        ApiProtocol::Grpc => "gRPC",
    };
    h_flex()
        .id("api-page-heading")
        .debug_selector(|| "api-page-heading".into())
        .w_full()
        .min_w_0()
        .flex_none()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(
            v_flex()
                .id("api-page-title")
                .debug_selector(|| "api-page-title".into())
                .flex_1()
                .min_w(px(180.0))
                .gap(px(2.0))
                .child(
                    Input::new(&view.request_name)
                        .large()
                        .bg(theme.background)
                        .border_color(theme.background)
                        .aria_label("请求名称")
                        .min_w_0(),
                )
                .child(
                    div()
                        .id("api-request-context")
                        .debug_selector(|| "api-request-context".into())
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("API 工作台 / {protocol}")),
                ),
        )
        .child(
            h_flex()
                .id("api-page-actions")
                .debug_selector(|| "api-page-actions".into())
                .flex_none()
                .flex_wrap()
                .items_center()
                .gap(px(4.0))
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
                )
                .when(compact, |actions| {
                    actions.child(super::render::collection_dialog::trigger(view, cx))
                })
                .child(import)
                .child(save),
        )
        .into_any_element()
}
