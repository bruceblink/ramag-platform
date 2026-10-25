use super::*;

use gpui_kit::ClickEvent;
use gpui_kit::component::button::ButtonVariants as _;
use ramag_domain::entities::{ApiRequestRecord, ApiRequestSpec, ApiWorkspace};

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
    let matching_request_total = if query.is_empty() {
        total_request_count
    } else {
        count_matching_requests(&view.workspace, &query)
    };
    let mut groups = Vec::new();
    let mut visible_request_count = 0;
    for collection in &view.workspace.collections {
        let mut items = Vec::new();
        for request in &collection.requests {
            if !request_matches_query(&collection.name, request, &query) {
                continue;
            }
            if visible_request_count == 50 {
                break;
            }
            let request = request.clone();
            let selected = current_request_id.as_ref() == Some(&request.id);
            let selector = format!("api-request-item-{visible_request_count}");
            let label = match &request.request {
                ApiRequestSpec::Http(spec) => format!("{} · {}", spec.method, request.name),
                ApiRequestSpec::Grpc(_) => format!("gRPC · {}", request.name),
            };
            let mut item =
                ramag_ui::clickable_button(gpui_kit::SharedString::from(selector.clone()))
                    .debug_selector(move || selector.clone())
                    .w_full()
                    .min_w_0()
                    .justify_start()
                    .truncate()
                    .xsmall()
                    .label(label)
                    .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                        context::apply_request_to_view(view, &request, window, cx);
                        cx.notify();
                    }));
            item = if selected {
                item.primary()
            } else {
                item.ghost()
            };
            items.push(item.into_any_element());
            visible_request_count += 1;
        }
        if !items.is_empty() {
            let collection_count = items.len();
            groups.push(
                v_flex()
                    .gap(px(2.0))
                    .child(
                        h_flex()
                            .min_w_0()
                            .items_center()
                            .justify_between()
                            .px(px(4.0))
                            .py(px(3.0))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .child(collection.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(collection_count.to_string()),
                            ),
                    )
                    .child(v_flex().gap(px(2.0)).pl(px(6.0)).children(items))
                    .into_any_element(),
            );
        }
        if visible_request_count == 50 {
            break;
        }
    }
    let list_summary = if query.is_empty() {
        if total_request_count > visible_request_count {
            format!("显示 {visible_request_count} / {total_request_count} 个请求")
        } else {
            format!("共 {total_request_count} 个请求")
        }
    } else {
        if matching_request_total > visible_request_count {
            format!(
                "搜索“{query}”：匹配 {matching_request_total} 个请求，显示前 {visible_request_count} 个"
            )
        } else {
            format!("搜索“{query}”：匹配 {matching_request_total} 个请求")
        }
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
        v_flex().gap(px(6.0)).children(groups).into_any_element()
    };
    let mut sidebar = v_flex().gap(px(4.0)).child(
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
    );
    if !query.is_empty() || total_request_count > visible_request_count {
        sidebar = sidebar.child(
            div()
                .id("api-request-list-summary")
                .debug_selector(|| "api-request-list-summary".into())
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(list_summary),
        );
    }
    sidebar.child(request_list).into_any_element()
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

fn count_matching_requests(workspace: &ApiWorkspace, query: &str) -> usize {
    workspace
        .collections
        .iter()
        .flat_map(|collection| {
            collection
                .requests
                .iter()
                .map(move |request| (collection.name.as_str(), request))
        })
        .filter(|(collection_name, request)| request_matches_query(collection_name, request, query))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::{ApiCollection, HttpRequestSpec};

    #[test]
    fn matching_request_count_includes_requests_beyond_render_limit() {
        let mut workspace = ApiWorkspace::new("Saved requests");
        let mut collection = ApiCollection::new("Orders");
        for index in 0..55 {
            collection.requests.push(ApiRequestRecord::new_http(
                format!("Order request {index}"),
                HttpRequestSpec::new("GET", format!("http://127.0.0.1/orders/{index}")),
            ));
        }
        workspace.collections.push(collection);

        assert_eq!(count_matching_requests(&workspace, "orders"), 55);
    }
}
