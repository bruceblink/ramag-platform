//! 从编辑器生成保存和发送共用的请求，保留没有对应控件的已存配置。

use super::context::format_helpers::{format_parameters, format_query_parameters};
use super::*;

pub(crate) fn request_from_view(view: &ApiView, cx: &App) -> Result<ApiRequestSpec> {
    match view.protocol {
        ApiProtocol::Http => http_from_view(view, cx).map(ApiRequestSpec::Http),
        ApiProtocol::Grpc => grpc_from_view(view, cx).map(ApiRequestSpec::Grpc),
    }
}

fn http_from_view(view: &ApiView, cx: &App) -> Result<HttpRequestSpec> {
    let method = input_value(&view.http_method, cx).to_ascii_uppercase();
    let url = input_value(&view.http_url, cx);
    let mut spec = match &view.request_baseline {
        Some(ApiRequestSpec::Http(original)) => original.clone(),
        _ => HttpRequestSpec::new(&method, &url),
    };
    spec.method = method;
    spec.url_template = url;
    spec.tls = context::tls_from_view(view, cx)?;
    spec.proxy = context::proxy_from_view(view, cx)?;
    spec.auth = view.auth_editor.to_auth(cx)?;
    let query = textarea_value(&view.http_query, cx);
    if query != format_query_parameters(&spec.query).trim() {
        spec.query = edited_parameters(parse_http_query(&query)?, &spec.query, false);
    }
    let headers = textarea_value(&view.http_headers, cx);
    let original_content_type = spec
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("content-type"))
        .map(|header| header.value.clone());
    if headers != format_parameters(&spec.headers).trim() {
        spec.headers = edited_parameters(parse_http_headers(&headers)?, &spec.headers, true);
    }
    let content_type = spec
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("content-type"))
        .map(|header| header.value.clone());
    let body = editor_value(&view.http_body, cx);
    // 逐行编辑器会裁剪空白；未触碰正文时保留原字节、空正文及 Multipart 附加字段。
    let body_unchanged = spec.body.as_ref().is_some_and(|original| {
        let original_text = match original.mode {
            ApiBodyMode::Text => original.value.clone(),
            ApiBodyMode::Multipart => format_multipart_body(&original.multipart),
        };
        let content_type = match original.mode {
            ApiBodyMode::Text => original
                .content_type
                .as_deref()
                .unwrap_or("application/json"),
            ApiBodyMode::Multipart => "",
        };
        original.mode == view.http_body_mode
            && original_text.trim() == body
            && content_type == view.http_body_content_type
    });
    if body_unchanged {
        if original_content_type != content_type
            && let Some(original) = spec.body.as_mut()
            && original.mode == ApiBodyMode::Text
        {
            original.content_type = content_type.or_else(|| {
                let value = view.http_body_content_type.trim();
                (!value.is_empty()).then(|| value.to_string())
            });
        }
    } else {
        spec.body = if body.is_empty() {
            None
        } else {
            Some(match view.http_body_mode {
                ApiBodyMode::Text => {
                    let content_type = content_type.or_else(|| {
                        let value = view.http_body_content_type.trim();
                        (!value.is_empty()).then(|| value.to_string())
                    });
                    ApiBody::text(body, content_type)
                }
                ApiBodyMode::Multipart => ApiBody::multipart(parse_multipart_body(&body)?),
            })
        };
    }
    Ok(spec)
}

/// 先匹配未改值的同名参数，再按同名出现顺序匹配已改值，避免重排或删除时串用敏感标记。
/// 新名称使用默认值，清空文本删除全部参数。
/// HTTP 请求头名称不区分大小写，查询参数名称区分大小写。
fn edited_parameters(
    mut edited: Vec<ApiParameter>,
    original: &[ApiParameter],
    case_insensitive: bool,
) -> Vec<ApiParameter> {
    let mut remaining = original.iter().collect::<Vec<_>>();
    let mut unmatched = Vec::new();
    let same_name = |left: &str, right: &str| {
        if case_insensitive {
            left.eq_ignore_ascii_case(right)
        } else {
            left == right
        }
    };
    for (position, parameter) in edited.iter_mut().enumerate() {
        if let Some(index) = remaining.iter().position(|previous| {
            same_name(&previous.name, &parameter.name) && previous.value == parameter.value
        }) {
            parameter.sensitive = remaining.remove(index).sensitive;
        } else {
            unmatched.push(position);
        }
    }
    for position in unmatched {
        let parameter = &mut edited[position];
        if let Some(index) = remaining
            .iter()
            .position(|previous| same_name(&previous.name, &parameter.name))
        {
            parameter.sensitive = remaining.remove(index).sensitive;
        }
    }
    edited
}

fn grpc_from_view(view: &ApiView, cx: &App) -> Result<GrpcRequestSpec> {
    let endpoint = input_value(&view.grpc_endpoint, cx);
    let service = input_value(&view.grpc_service, cx);
    let method = input_value(&view.grpc_method, cx);
    let mut spec = match &view.request_baseline {
        Some(ApiRequestSpec::Grpc(original)) => original.clone(),
        _ => GrpcRequestSpec::new(&endpoint, &service, &method),
    };
    spec.endpoint_template = endpoint;
    spec.service = service;
    spec.method = method;
    let message = editor_value(&view.grpc_message, cx);
    if message != spec.message.trim() {
        spec.message = message;
    }
    spec.tls = context::tls_from_view(view, cx)?;
    spec.proxy = context::proxy_from_view(view, cx)?;
    spec.auth = view.auth_editor.to_auth(cx)?;
    spec.descriptor = view.grpc_descriptor.clone();
    let name = input_value(&view.grpc_metadata_name, cx);
    let value = input_value(&view.grpc_metadata_value, cx);
    // 这两个输入框只编辑首项；隐藏的尾项必须按原顺序保留，不能因清空首项而全删。
    if let Some(first) = spec.metadata.first_mut() {
        if name.is_empty() && value.is_empty() {
            spec.metadata.remove(0);
        } else {
            if name != first.name.trim() {
                first.name = name;
            }
            if value != first.value.trim() {
                first.value = value;
            }
        }
    } else if !name.is_empty() || !value.is_empty() {
        spec.metadata.push(ApiParameter::new(name, value, false));
    }
    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_duplicate_values_keep_flags_when_reordered_or_deleted() {
        let original = vec![
            ApiParameter::new("item", "public", false),
            ApiParameter::new("item", "private", true),
        ];
        for edited in [
            vec![
                ApiParameter::new("item", "private", false),
                ApiParameter::new("item", "public", false),
            ],
            vec![ApiParameter::new("item", "private", false)],
        ] {
            let result = edited_parameters(edited, &original, false);
            assert!(result[0].sensitive);
            if let Some(second) = result.get(1) {
                assert!(!second.sensitive);
            }
        }
    }
}
