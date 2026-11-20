//! API 请求配置面板及其独立滚动区域。

use super::*;

/// 固定请求标题，把较长的配置内容放入独立滚动区。
pub(super) fn render(
    view: &ApiView,
    window: &Window,
    request_editor: gpui_kit::AnyElement,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let heading = h_flex()
        .id("api-request-pane-heading")
        .debug_selector(|| "api-request-pane-heading".into())
        .h(px(ramag_ui::WORKBENCH_TOOLBAR_HEIGHT))
        .flex_none()
        .items_center()
        .justify_between()
        .px(px(14.0))
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .text_sm()
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .child("请求"),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(match view.protocol {
                    ApiProtocol::Http => "HTTP",
                    ApiProtocol::Grpc => "gRPC",
                }),
        );
    let content = v_flex()
        .id("api-request-scroll-content")
        .debug_selector(|| "api-request-scroll-content".into())
        .flex_1()
        .min_w_0()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&view.request_scroll)
        .child(super::render_scrollbar_overlay(
            &view.request_scroll,
            "api-request-scrollbar",
        ))
        .p(px(12.0))
        .child(
            v_flex()
                .id("api-request-editor")
                .debug_selector(|| "api-request-editor".into())
                .w_full()
                .min_w_0()
                .flex_1()
                .min_h_0()
                .child(request_editor),
        );
    let pane = v_flex()
        .id("api-request-pane")
        .debug_selector(|| "api-request-pane".into())
        .min_w_0()
        .min_h_0()
        .child(heading)
        .child(content);
    if ApiView::is_stacked(window) {
        pane.flex_1().into_any_element()
    } else {
        pane.w(px(view.current_request_pane_width(window)))
            .flex_none()
            .into_any_element()
    }
}
