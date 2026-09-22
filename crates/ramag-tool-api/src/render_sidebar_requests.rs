use super::*;

use gpui_kit::ClickEvent;
use gpui_kit::component::button::ButtonVariants as _;
use ramag_domain::entities::{ApiRequestRecord, ApiRequestSpec};

pub(super) fn render(
    view: &ApiView,
    cx: &mut Context<ApiView>,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::AnyElement {
    let current_request_id = view.active_request_id.clone();
    let query = view.request_search.read(cx).value().trim().to_lowercase();
    let total_request_count = view
        .workspace
        .collections
        .iter()
        .map(|collection| collection.requests.len())
        .sum::<usize>();
    let requests = view
        .workspace
        .collections
        .iter()
        .flat_map(|collection| {
            collection
                .requests
                .iter()
                .map(move |request| (collection.name.as_str(), request))
        })
        .filter(|(collection_name, request)| {
            request_matches_query(collection_name, request, &query)
        })
        .take(50)
        .enumerate()
        .map(|(index, (collection_name, request))| {
            let request = request.clone();
            let selected = current_request_id.as_ref() == Some(&request.id);
            let mut item = ramag_ui::clickable_button(gpui_kit::SharedString::from(format!(
                "api-request-item-{index}"
            )))
            .debug_selector(move || format!("api-request-item-{index}"))
            .w_full()
            .min_w_0()
            .justify_start()
            .truncate()
            .xsmall()
            .label(format!(
                "{} / {} · {}",
                collection_name,
                protocol_label(request.protocol),
                request.name
            ))
            .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                context::apply_request_to_view(view, &request, window, cx);
                cx.notify();
            }));
            item = if selected {
                item.primary()
            } else {
                item.ghost()
            };
            item.into_any_element()
        })
        .collect::<Vec<_>>();
    let visible_request_count = requests.len();
    let list_summary = if query.is_empty() {
        if total_request_count > visible_request_count {
            format!("显示 {visible_request_count} / {total_request_count} 个请求")
        } else {
            format!("共 {total_request_count} 个请求")
        }
    } else {
        format!("搜索“{query}”：显示 {visible_request_count} 个请求")
    };
    let request_list = if visible_request_count == 0 {
        div()
            .id("api-request-list-empty")
            .debug_selector(|| "api-request-list-empty".into())
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(if query.is_empty() {
                "暂无已保存请求"
            } else {
                "没有匹配当前搜索条件的请求"
            })
            .into_any_element()
    } else {
        v_flex().gap(px(4.0)).children(requests).into_any_element()
    };
    v_flex()
        .gap(px(4.0))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("请求列表"),
        )
        .child(
            div()
                .id("api-request-search")
                .debug_selector(|| "api-request-search".into())
                .w_full()
                .min_w_0()
                .child(
                    ramag_ui::cleanable_input(
                        &view.request_search,
                        "api-request-search-clear",
                        false,
                        cx,
                    )
                    .small(),
                ),
        )
        .child(
            div()
                .id("api-request-list-summary")
                .debug_selector(|| "api-request-list-summary".into())
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(list_summary),
        )
        .child(request_list)
        .into_any_element()
}

fn request_matches_query(collection_name: &str, request: &ApiRequestRecord, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let matches = |value: &str| value.to_lowercase().contains(query);
    if matches(collection_name)
        || matches(&request.name)
        || matches(protocol_label(request.protocol))
    {
        return true;
    }
    match &request.request {
        ApiRequestSpec::Http(spec) => {
            matches(&spec.method)
                || matches(&spec.url_template)
                || spec
                    .query
                    .iter()
                    .any(|parameter| matches(&parameter.name) || matches(&parameter.value))
        }
        ApiRequestSpec::Grpc(spec) => {
            matches(&spec.endpoint_template) || matches(&spec.service) || matches(&spec.method)
        }
    }
}
