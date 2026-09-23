use super::render_body;
use super::render_helpers::{render_context_editor, render_history, render_request_toolbar};
use super::*;
use gpui_kit::FontWeight;

#[path = "render_sidebar_requests.rs"]
mod render_sidebar_requests;
#[path = "render_response.rs"]
mod response_render;

pub(super) fn render(
    view: &mut ApiView,
    window: &mut Window,
    cx: &mut Context<ApiView>,
) -> gpui_kit::AnyElement {
    let theme = cx.theme().clone();
    let stacked = ApiView::is_stacked(window);
    let compact_height = window.viewport_size().height < px(API_COMPACT_HEIGHT);
    if view.compact_height_layout != compact_height {
        view.compact_height_layout = compact_height;
        view.layout_scroll
            .set_offset(gpui_kit::point(px(0.0), px(0.0)));
    }
    let sidebar = render_sidebar(view, window, cx, compact_height, &theme);
    let editor = render_editor(view, window, cx, &theme);
    let content = if stacked {
        v_flex()
            .id("api-content")
            .debug_selector(|| "api-content".into())
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(sidebar)
            .child(editor)
    } else {
        h_flex()
            .id("api-content")
            .debug_selector(|| "api-content".into())
            .flex_1()
            .min_w_0()
            .items_stretch()
            .child(sidebar)
            .child(editor)
    };
    let content = content.when(compact_height, |content| {
        content.flex_none().min_h(px(API_COMPACT_CONTENT_HEIGHT))
    });
    v_flex()
        .id("api-root")
        .debug_selector(|| "api-root".into())
        .size_full()
        .min_w_0()
        .min_h_0()
        .bg(theme.background)
        .when(compact_height, |root| {
            root.overflow_y_scroll().track_scroll(&view.layout_scroll)
        })
        .child(render_header(view, cx, &theme))
        .child(content)
        .into_any_element()
}

fn render_header(
    _view: &mut ApiView,
    _cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    h_flex()
        .id("api-header")
        .debug_selector(|| "api-header".into())
        .w_full()
        .min_w_0()
        .flex_none()
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
        .child(div().flex_1().min_w_0())
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("API Workspace"),
        )
        .into_any_element()
}

fn render_sidebar(
    view: &ApiView,
    window: &Window,
    cx: &mut Context<ApiView>,
    compact_height: bool,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let stacked = ApiView::is_stacked(window);
    v_flex()
        .id("api-sidebar")
        .debug_selector(|| "api-sidebar".into())
        .w(px(API_SIDEBAR_WIDTH))
        .flex_none()
        .min_h_0()
        .min_w_0()
        .when(stacked, |sidebar| {
            sidebar
                .w_full()
                .when(compact_height, |sidebar| {
                    sidebar.h(px(API_COMPACT_SIDEBAR_HEIGHT)).flex_none()
                })
                .when(!compact_height, |sidebar| sidebar.flex_1().max_h(px(240.0)))
        })
        .when(!stacked, |sidebar| sidebar.h_full())
        .overflow_y_scroll()
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
                    ApiProtocol::Grpc => "gRPC 请求（支持流式）",
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
        .child(render_sidebar_requests::render(view, cx, theme))
        .child(render_history(&view.history, theme))
        .into_any_element()
}

fn render_editor(
    view: &mut ApiView,
    window: &mut Window,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let stacked = ApiView::is_stacked(window);
    let request_editor = match view.protocol {
        ApiProtocol::Http => render_http_editor(view, cx, theme),
        ApiProtocol::Grpc => render_grpc::render_editor(view, cx, theme),
    };
    let mut request_pane = v_flex()
        .id("api-request-pane")
        .debug_selector(|| "api-request-pane".into())
        .flex_1()
        .min_w_0()
        .min_h_0()
        .overflow_y_scroll()
        .gap(px(10.0))
        .p(px(14.0))
        .child(
            v_flex()
                .id("api-request-editor")
                .debug_selector(|| "api-request-editor".into())
                .w_full()
                .min_w_0()
                .gap(px(10.0))
                .child(request_editor),
        )
        .child(render_context_editor(view, theme));
    if !stacked {
        request_pane = request_pane.border_r_1().border_color(theme.border);
    }
    let workbench = if stacked {
        v_flex()
            .id("api-request-response")
            .debug_selector(|| "api-request-response".into())
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(request_pane)
            .child(response_render::render_response(view, cx, theme))
    } else {
        h_flex()
            .id("api-request-response")
            .debug_selector(|| "api-request-response".into())
            .flex_1()
            .min_w_0()
            .min_h_0()
            .items_stretch()
            .child(request_pane)
            .child(response_render::render_response(view, cx, theme))
    };
    v_flex()
        .id("api-editor")
        .debug_selector(|| "api-editor".into())
        .flex_1()
        .h_full()
        .min_w_0()
        .min_h_0()
        .child(render_request_toolbar(view, cx, theme))
        .child(workbench)
        .into_any_element()
}

fn render_http_editor(
    view: &mut ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    v_flex()
        .id("api-http-fields")
        .debug_selector(|| "api-http-fields".into())
        .w_full()
        .min_w_0()
        .gap(px(10.0))
        .child(
            v_flex()
                .id("api-http-query")
                .debug_selector(|| "api-http-query".into())
                .gap(px(5.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Params"),
                )
                .child(Textarea::new(&view.http_query).h(px(72.0))),
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
                .child(Textarea::new(&view.http_headers).h(px(112.0))),
        )
        .child(view.auth_editor.render(cx, theme))
        .child(render_body::render_http_body(view, cx, theme))
        .into_any_element()
}

fn protocol_label(protocol: ApiProtocol) -> &'static str {
    match protocol {
        ApiProtocol::Http => "HTTP",
        ApiProtocol::Grpc => "gRPC",
    }
}
