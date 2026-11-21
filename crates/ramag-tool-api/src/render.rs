use super::render_body;
use super::render_helpers::{
    render_context_editor, render_history, render_inline_notice, render_request_toolbar,
};
use super::render_page_heading::render_request_heading;
use super::*;
use gpui_kit::ClickEvent;
use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::component::tab::{Tab, TabBar};

#[path = "render_collection_dialog.rs"]
pub(super) mod collection_dialog;
#[path = "render_sidebar_requests.rs"]
mod render_sidebar_requests;
#[path = "render_request_pane.rs"]
mod request_pane_render;
#[path = "render_response.rs"]
mod response_render;

pub(super) fn render_scrollbar_overlay(
    scroll_handle: &ScrollHandle,
    id: &'static str,
) -> impl IntoElement {
    div().absolute().inset_0().child(
        Scrollbar::vertical(scroll_handle)
            .id(id)
            .mode(ScrollbarMode::Scrolling)
            .viewport_from_layout(),
    )
}

pub(super) fn render(
    view: &mut ApiView,
    window: &mut Window,
    cx: &mut Context<ApiView>,
) -> gpui_kit::AnyElement {
    let theme = cx.theme().clone();
    let compact = ApiView::is_compact(window);
    let show_sidebar = !compact && !view.collection_dialog_open;
    if show_sidebar {
        view.sidebar_width = view
            .sidebar_width
            .clamp(API_SIDEBAR_MIN_WIDTH, ApiView::max_sidebar_width(window));
    }
    let compact_height = window.viewport_size().height < px(API_COMPACT_HEIGHT);
    if view.compact_height_layout != compact_height {
        view.compact_height_layout = compact_height;
        view.layout_scroll
            .set_offset(gpui_kit::point(px(0.0), px(0.0)));
    }
    let page_heading = render_request_heading(view, cx, &theme, compact);
    let editor = render_editor(view, window, cx, &theme, !show_sidebar);
    let content = if !show_sidebar {
        v_flex()
            .id("api-content")
            .debug_selector(|| "api-content".into())
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(editor)
    } else {
        h_flex()
            .id("api-content")
            .debug_selector(|| "api-content".into())
            .flex_1()
            .min_w_0()
            .items_stretch()
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, window, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                let mut changed = false;
                if let Some((start_x, start_width)) = view.sidebar_drag_start {
                    let delta = f32::from(event.position.x - start_x);
                    view.sidebar_width = (start_width + delta)
                        .clamp(API_SIDEBAR_MIN_WIDTH, ApiView::max_sidebar_width(window));
                    changed = true;
                }
                if let Some((start_x, start_width)) = view.request_response_drag_start {
                    let delta = f32::from(event.position.x - start_x);
                    view.request_pane_width = Some((start_width + delta).clamp(
                        API_REQUEST_PANE_MIN_WIDTH,
                        view.max_request_pane_width(window),
                    ));
                    changed = true;
                }
                if changed {
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _, cx| {
                    let sidebar_drag = view.sidebar_drag_start.take().is_some();
                    let request_response_drag = view.request_response_drag_start.take().is_some();
                    if sidebar_drag || request_response_drag {
                        cx.notify();
                    }
                }),
            )
            .child(render_sidebar(view, cx, &theme, false))
            .child(
                div()
                    .id("api-sidebar-divider")
                    .debug_selector(|| "api-sidebar-divider".into())
                    .w(px(API_SIDEBAR_DIVIDER_WIDTH))
                    .h_full()
                    .flex_none()
                    .cursor_col_resize()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|view, event: &MouseDownEvent, _, cx| {
                            view.sidebar_drag_start = Some((event.position.x, view.sidebar_width));
                            cx.notify();
                        }),
                    )
                    .child(v_flex().size_full().items_center().child(
                        div().w(px(API_SIDEBAR_DIVIDER_LINE_WIDTH)).h_full().bg(
                            if view.sidebar_drag_start.is_some() {
                                theme.link
                            } else {
                                theme.border
                            },
                        ),
                    )),
            )
            .child(editor)
    };
    let content = content.when(compact_height, |content| {
        content.flex_none().min_h(px(API_COMPACT_CONTENT_HEIGHT))
    });
    v_flex()
        .id("api-root")
        .debug_selector(|| "api-root".into())
        .track_focus(&view.focus_handle)
        .size_full()
        .min_w_0()
        .min_h_0()
        .gap(px(14.0))
        .p(px(API_PAGE_INSET))
        .bg(theme.background)
        .when(compact_height, |root| {
            root.overflow_y_scroll()
                .track_scroll(&view.layout_scroll)
                .child(render_scrollbar_overlay(
                    &view.layout_scroll,
                    "api-layout-scrollbar",
                ))
        })
        .child(page_heading)
        .when(compact && !view.collection_dialog_open, |root| {
            root.children(render_workspace_load_status(view, cx, &theme))
        })
        .child(
            content
                .border_1()
                .border_color(theme.border)
                .rounded(theme.radius)
                .p(px(API_WORKBENCH_INSET)),
        )
        .into_any_element()
}

fn render_sidebar(
    view: &ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
    dialog: bool,
) -> gpui_kit::AnyElement {
    let workspace_name = if view.workspace.name == "API Workspace" {
        "API 工作台".to_owned()
    } else {
        view.workspace.name.clone()
    };
    let request_count = view
        .workspace
        .collections
        .iter()
        .map(|collection| collection.requests.len())
        .sum::<usize>();
    v_flex()
        .id("api-sidebar")
        .debug_selector(|| "api-sidebar".into())
        .bg(ramag_ui::pulse_ui::pulse_palette(cx).surface)
        .w(px(view.sidebar_width))
        .flex_none()
        .min_h_0()
        .min_w_0()
        .when(dialog, |sidebar| sidebar.w_full().h_full())
        .when(!dialog, |sidebar| {
            sidebar
                .h_full()
                .rounded_tl(theme.radius)
                .rounded_bl(theme.radius)
                .border_r_1()
                .border_color(theme.border)
        })
        .overflow_y_scroll()
        .track_scroll(&view.sidebar_scroll)
        .child(render_scrollbar_overlay(
            &view.sidebar_scroll,
            "api-sidebar-scrollbar",
        ))
        .gap(px(7.0))
        .p(px(10.0))
        .child(
            v_flex()
                .id("api-workspace-heading")
                .debug_selector(|| "api-workspace-heading".into())
                .w_full()
                .min_w_0()
                .gap(px(3.0))
                .when(!dialog, |heading| {
                    heading.child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("请求集合"),
                    )
                })
                .child(
                    h_flex()
                        .min_w_0()
                        .items_center()
                        .justify_between()
                        .gap(px(8.0))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .child(workspace_name),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!(
                                    "{} 个集合 · {request_count} 个请求",
                                    view.workspace.collections.len()
                                )),
                        ),
                ),
        )
        .children(render_workspace_load_status(view, cx, theme))
        .when(dialog, |sidebar| {
            sidebar.children(render_inline_notice(view, cx, theme))
        })
        .child(render_sidebar_requests::render(view, cx, theme))
        .child(render_history(view, cx, theme))
        .into_any_element()
}

fn render_workspace_load_status(
    view: &ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
) -> Option<gpui_kit::AnyElement> {
    use gpui_kit::component::button::ButtonVariants as _;

    if view.workspace_load_state == ApiWorkspaceLoadState::Empty {
        return None;
    }
    let message = view.workspace_load_state.status_message()?;
    let state = view.workspace_load_state;
    let message_color = match state {
        ApiWorkspaceLoadState::Loading => theme.link,
        ApiWorkspaceLoadState::HistoryFailed | ApiWorkspaceLoadState::Failed => theme.danger,
        ApiWorkspaceLoadState::NotStarted
        | ApiWorkspaceLoadState::Empty
        | ApiWorkspaceLoadState::Loaded => theme.muted_foreground,
    };
    let mut status = h_flex()
        .id("api-workspace-load-status")
        .debug_selector(|| "api-workspace-load-status".into())
        .w_full()
        .min_w_0()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .id("api-workspace-load-notice")
                .debug_selector(|| "api-workspace-load-notice".into())
                .flex_1()
                .min_w_0()
                .text_xs()
                .text_color(message_color)
                .whitespace_normal()
                .child(message),
        );
    if matches!(
        state,
        ApiWorkspaceLoadState::Failed | ApiWorkspaceLoadState::HistoryFailed
    ) {
        status = status.child(
            ramag_ui::clickable_button("api-workspace-load-retry")
                .debug_selector(|| "api-workspace-load-retry".into())
                .xsmall()
                .label("重试")
                .ghost()
                .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                    view.load_saved_workspace(window, cx);
                })),
        );
    }
    Some(status.into_any_element())
}

fn render_editor(
    view: &mut ApiView,
    window: &mut Window,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
    compact: bool,
) -> gpui_kit::AnyElement {
    let workbench = if compact && view.compact_pane == ApiCompactPane::Response {
        response_render::render_response(view, cx, theme, true)
    } else {
        render_request_workbench(view, window, cx, theme, compact)
    };
    v_flex()
        .id("api-editor")
        .debug_selector(|| "api-editor".into())
        .flex_1()
        .h_full()
        .rounded_br(theme.radius)
        .when(!compact, |editor| editor.rounded_tr(theme.radius))
        .min_w_0()
        .min_h_0()
        .child(render_request_toolbar(view, window, cx, theme))
        .when(compact, |editor| {
            editor.child(render_compact_tabs(view, cx, theme)).when(
                view.compact_pane == ApiCompactPane::Request,
                |editor| {
                    editor.when_some(render_inline_notice(view, cx, theme), |editor, notice| {
                        editor.child(
                            div()
                                .id("api-compact-request-notice")
                                .debug_selector(|| "api-compact-request-notice".into())
                                .flex_none()
                                .px(px(12.0))
                                .pt(px(8.0))
                                .child(notice),
                        )
                    })
                },
            )
        })
        .child(
            v_flex()
                .id("api-request-response")
                .debug_selector(|| "api-request-response".into())
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(workbench),
        )
        .into_any_element()
}

fn render_request_workbench(
    view: &mut ApiView,
    window: &mut Window,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
    compact: bool,
) -> gpui_kit::AnyElement {
    let request_editor = match view.protocol {
        ApiProtocol::Http => render_http_editor(view, cx, theme),
        ApiProtocol::Grpc => render_grpc::render_editor(view, cx, theme),
    };
    let request_pane = request_pane_render::render(view, window, request_editor, theme, compact);
    if compact {
        return request_pane;
    }
    let divider = div()
        .id("api-request-response-divider")
        .debug_selector(|| "api-request-response-divider".into())
        .w(px(API_REQUEST_RESPONSE_DIVIDER_WIDTH))
        .h_full()
        .flex_none()
        .cursor_col_resize()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|view, event: &MouseDownEvent, window, cx| {
                view.request_response_drag_start =
                    Some((event.position.x, view.current_request_pane_width(window)));
                cx.notify();
            }),
        )
        .child(
            v_flex().size_full().items_center().child(
                div()
                    .w(px(API_REQUEST_RESPONSE_DIVIDER_LINE_WIDTH))
                    .h_full()
                    .bg(if view.request_response_drag_start.is_some() {
                        theme.link
                    } else {
                        theme.border
                    }),
            ),
        );

    h_flex()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .items_stretch()
        .child(request_pane)
        .child(divider)
        .child(response_render::render_response(view, cx, theme, false))
        .into_any_element()
}

fn render_compact_tabs(
    view: &ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
) -> impl IntoElement {
    TabBar::new("api-compact-pane-tabs")
        .underline()
        .w_full()
        .min_w_0()
        .flex_none()
        .px(px(12.0))
        .border_b_1()
        .border_color(theme.border)
        .selected_index(usize::from(view.compact_pane == ApiCompactPane::Response))
        .on_click(cx.listener(|view, index: &usize, _, cx| {
            view.compact_pane = if *index == 0 {
                ApiCompactPane::Request
            } else {
                ApiCompactPane::Response
            };
            cx.notify();
        }))
        .child(
            Tab::new()
                .label("请求")
                .debug_selector(|| "api-compact-request".into()),
        )
        .child(
            Tab::new()
                .label("响应")
                .debug_selector(|| "api-compact-response".into()),
        )
}

pub(super) fn render_request_tabs(
    view: &ApiView,
    cx: &mut Context<ApiView>,
) -> gpui_kit::AnyElement {
    let protocol = view.protocol;
    let tabs = TabBar::new("api-request-config-tab-bar")
        .underline()
        .w_full()
        .min_w_0()
        .selected_index(view.request_tab.index(protocol))
        .on_click(cx.listener(|view, index: &usize, _, cx| {
            view.request_tab = ApiRequestTab::from_index(*index, view.protocol);
            cx.notify();
        }));
    let tabs = match protocol {
        ApiProtocol::Http => tabs
            .child(
                Tab::new()
                    .label("参数")
                    .debug_selector(|| "api-request-tab-params".into()),
            )
            .child(
                Tab::new()
                    .label("请求头")
                    .debug_selector(|| "api-request-tab-headers".into()),
            )
            .child(
                Tab::new()
                    .label("认证")
                    .debug_selector(|| "api-request-tab-auth".into()),
            )
            .child(
                Tab::new()
                    .label("请求体")
                    .debug_selector(|| "api-request-tab-body".into()),
            )
            .child(
                Tab::new()
                    .label("高级")
                    .debug_selector(|| "api-request-tab-advanced".into()),
            ),
        ApiProtocol::Grpc => tabs
            .child(
                Tab::new()
                    .label("请求体")
                    .debug_selector(|| "api-request-tab-body".into()),
            )
            .child(
                Tab::new()
                    .label("元数据")
                    .debug_selector(|| "api-request-tab-metadata".into()),
            )
            .child(
                Tab::new()
                    .label("认证")
                    .debug_selector(|| "api-request-tab-auth".into()),
            )
            .child(
                Tab::new()
                    .label("高级")
                    .debug_selector(|| "api-request-tab-advanced".into()),
            ),
    };
    div()
        .id("api-request-config-tabs")
        .debug_selector(|| "api-request-config-tabs".into())
        .w_full()
        .min_w_0()
        .flex_none()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(tabs)
        .into_any_element()
}

fn render_http_editor(
    view: &mut ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let fields = match view.request_tab {
        ApiRequestTab::Params => v_flex()
            .id("api-http-query")
            .debug_selector(|| "api-http-query".into())
            .w_full()
            .min_w_0()
            .flex_1()
            .min_h_0()
            .gap(px(5.0))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("查询参数"),
            )
            .child(Textarea::new(&view.http_query).h(px(200.0)))
            .into_any_element(),
        ApiRequestTab::Headers => v_flex()
            .id("api-http-headers")
            .debug_selector(|| "api-http-headers".into())
            .w_full()
            .min_w_0()
            .flex_1()
            .min_h_0()
            .gap(px(5.0))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("请求头"),
            )
            .child(Textarea::new(&view.http_headers).h(px(240.0)))
            .into_any_element(),
        ApiRequestTab::Auth => view.auth_editor.render(cx, theme),
        ApiRequestTab::Body => render_body::render_http_body(view, cx, theme),
        ApiRequestTab::Advanced | ApiRequestTab::Metadata => render_context_editor(view, theme),
    };
    v_flex()
        .id("api-http-fields")
        .debug_selector(|| "api-http-fields".into())
        .w_full()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .gap(px(8.0))
        .child(render_request_tabs(view, cx))
        .child(fields)
        .into_any_element()
}

fn protocol_label(protocol: ApiProtocol) -> &'static str {
    match protocol {
        ApiProtocol::Http => "HTTP",
        ApiProtocol::Grpc => "gRPC",
    }
}
