use super::*;

use gpui_kit::ClickEvent;
use gpui_kit::component::button::ButtonVariants as _;

pub(super) fn render_http_body(
    view: &mut ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let mode_button = |id: &'static str,
                       label: &'static str,
                       mode: ApiBodyMode,
                       view: &mut ApiView,
                       cx: &mut Context<ApiView>| {
        let mut button = ramag_ui::clickable_button(id)
            .debug_selector(move || id.into())
            .xsmall()
            .label(label)
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                view.set_http_body_mode(mode, cx);
            }));
        button = if view.http_body_mode == mode {
            button.primary()
        } else {
            button.ghost()
        };
        button.into_any_element()
    };
    let editor = Editor::new(&view.http_body).h(px(176.0));
    v_flex()
        .id("api-http-body")
        .debug_selector(|| "api-http-body".into())
        .gap(px(5.0))
        .child(
            h_flex()
                .flex_wrap()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Body"),
                )
                .child(mode_button(
                    "api-body-text",
                    "Text",
                    ApiBodyMode::Text,
                    view,
                    cx,
                ))
                .child(mode_button(
                    "api-body-multipart",
                    "Multipart",
                    ApiBodyMode::Multipart,
                    view,
                    cx,
                ))
                .child(div().flex_1().min_w_0())
                .child(div().text_xs().text_color(theme.muted_foreground).child(
                    if view.http_body_mode == ApiBodyMode::Text {
                        view.http_body_content_type.clone()
                    } else {
                        "自动生成 boundary".into()
                    },
                )),
        )
        .child(editor)
        .into_any_element()
}
