use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::super::super::{ApiParameter, MAX_API_PARAMETER_COUNT, MAX_API_PARAMETER_NAME_BYTES};
use super::super::format_error;
use super::ParameterDefinition;

pub(super) fn merge_parameters(
    path_parameters: &[ParameterDefinition],
    operation_parameters: &[ParameterDefinition],
) -> Result<Vec<ParameterDefinition>, String> {
    let mut merged = Vec::with_capacity(path_parameters.len() + operation_parameters.len());
    for parameter in path_parameters.iter().chain(operation_parameters) {
        let key = (
            parameter.location.as_str(),
            parameter.name.to_ascii_lowercase(),
        );
        if let Some(existing) = merged.iter_mut().find(|item: &&mut ParameterDefinition| {
            (item.location.as_str(), item.name.to_ascii_lowercase()) == key
        }) {
            *existing = parameter.clone();
        } else {
            merged.push(parameter.clone());
        }
    }
    if merged.len() > MAX_API_PARAMETER_COUNT {
        return Err(format_error("$.paths", "合并后的参数数量超过上限"));
    }
    Ok(merged)
}

pub(super) fn build_parameters(
    parameters: &[ParameterDefinition],
    path: &str,
) -> Result<(Vec<ApiParameter>, Vec<ApiParameter>), String> {
    let mut query = Vec::new();
    let mut headers = Vec::new();
    let mut cookies = Vec::new();
    for parameter in parameters {
        let Some(value) = parameter.value.clone() else {
            continue;
        };
        let item = ApiParameter::new(&parameter.name, value, parameter.sensitive);
        match parameter.location.as_str() {
            "query" => query.push(item),
            "header" => push_unique_header(&mut headers, item, path)?,
            "cookie" => cookies.push(item),
            "path" => {}
            _ => return Err(format_error(path, "出现未支持的参数位置")),
        }
    }
    if !cookies.is_empty() {
        let value = cookies
            .into_iter()
            .map(|item| format!("{}={}", item.name, item.value))
            .collect::<Vec<_>>()
            .join("; ");
        push_unique_header(
            &mut headers,
            ApiParameter::new("Cookie", value, false),
            path,
        )?;
    }
    Ok((query, headers))
}

fn push_unique_header(
    headers: &mut Vec<ApiParameter>,
    parameter: ApiParameter,
    path: &str,
) -> Result<(), String> {
    if headers
        .iter()
        .any(|item| item.name.eq_ignore_ascii_case(&parameter.name))
    {
        return Err(format_error(path, "Header 名称重复"));
    }
    headers.push(parameter);
    Ok(())
}

pub(super) fn apply_path_parameters(
    raw_path: &str,
    parameters: &[ParameterDefinition],
    path: &str,
) -> Result<String, String> {
    let mut result = raw_path.to_string();
    let mut seen = HashSet::new();
    for parameter in parameters.iter().filter(|item| item.location == "path") {
        let placeholder = format!("{{{}}}", parameter.name);
        if !raw_path.contains(&placeholder) {
            return Err(format_error(path, "Path 参数未出现在 Path 模板中"));
        }
        let value = parameter.value.as_deref().unwrap_or_default();
        result = result.replace(&placeholder, value);
        seen.insert(parameter.name.as_str());
    }
    if let Some(start) = result.find('{')
        && result[start..].contains('}')
    {
        return Err(format_error(path, "Path 模板包含未声明的参数"));
    }
    if seen.is_empty() && raw_path.contains('{') {
        return Err(format_error(path, "Path 模板包含未声明的参数"));
    }
    Ok(result)
}

pub(super) fn join_server_and_path(server: Option<&str>, path: &str) -> String {
    let Some(server) = server.filter(|value| !value.is_empty()) else {
        return path.to_string();
    };
    if server.ends_with('/') && path.starts_with('/') {
        format!("{}{}", server.trim_end_matches('/'), path)
    } else if !server.ends_with('/') && !path.starts_with('/') {
        format!("{server}/{path}")
    } else {
        format!("{server}{path}")
    }
}

pub(super) fn replace_server_variables(
    url: &str,
    variables: &HashMap<String, String>,
    path: &str,
) -> Result<String, String> {
    let mut result = String::with_capacity(url.len());
    let mut cursor = 0usize;
    while let Some(relative_start) = url[cursor..].find('{') {
        let start = cursor + relative_start;
        result.push_str(&url[cursor..start]);
        let value_start = start + 1;
        let relative_end = url[value_start..]
            .find('}')
            .ok_or_else(|| format_error(path, "服务器 URL 变量未闭合"))?;
        let end = value_start + relative_end;
        let name = &url[value_start..end];
        if name.is_empty() || !variables.contains_key(name) {
            return Err(format_error(path, "服务器 URL 引用了未声明变量"));
        }
        result.push_str("{{");
        result.push_str(name);
        result.push_str("}}");
        cursor = end + 1;
    }
    result.push_str(&url[cursor..]);
    Ok(result)
}

pub(super) fn is_json_media_type(media_type: &str) -> bool {
    let media_type = media_type.split(';').next().unwrap_or(media_type).trim();
    media_type.eq_ignore_ascii_case("application/json")
        || media_type.to_ascii_lowercase().ends_with("+json")
}

pub(super) fn value_to_text(value: &Value, path: &str) -> Result<String, String> {
    match value {
        Value::String(value) => Ok(value.clone()),
        _ => serde_json::to_string(value)
            .map_err(|error| format_error(path, &format!("示例值无法序列化：{error}"))),
    }
}

pub(super) fn validate_openapi_name(name: &str, path: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err(format_error(path, "名称不能为空"));
    }
    if name.len() > MAX_API_PARAMETER_NAME_BYTES {
        return Err(format_error(path, "名称超过长度上限"));
    }
    if name
        .chars()
        .any(|character| character.is_whitespace() || character.is_control() || character == ':')
    {
        return Err(format_error(path, "名称包含空白、控制字符或 ':'"));
    }
    Ok(())
}

pub(super) fn sanitize_name(name: &str) -> String {
    let mut result = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
            result.push(character);
        } else {
            result.push('_');
        }
    }
    if result.is_empty() {
        "scheme".into()
    } else {
        result
    }
}

pub(super) fn is_sensitive_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "authorization",
        "token",
        "password",
        "secret",
        "api-key",
        "apikey",
    ]
    .iter()
    .any(|needle| name.contains(needle))
}

pub(super) fn resolve_ref<'a>(
    root: &'a Value,
    value: &'a Value,
    path: &str,
) -> Result<Value, String> {
    let mut references = Vec::new();
    resolve_ref_inner(root, value, path, &mut references)
}

fn resolve_ref_inner(
    root: &Value,
    value: &Value,
    path: &str,
    references: &mut Vec<String>,
) -> Result<Value, String> {
    let Some(object) = value.as_object() else {
        return Ok(value.clone());
    };
    let Some(reference) = object.get("$ref") else {
        return Ok(value.clone());
    };
    if object.len() != 1 {
        return Err(format_error(path, "$ref 不能与其它字段混用"));
    }
    let reference = reference
        .as_str()
        .ok_or_else(|| format_error(&format!("{path}.$ref"), "必须是字符串"))?;
    if references.iter().any(|item| item == reference) {
        return Err(format_error(path, "$ref 存在循环引用"));
    }
    let target = resolve_pointer(root, reference, &format!("{path}.$ref"))?;
    references.push(reference.to_string());
    let result = resolve_ref_inner(root, target, path, references);
    references.pop();
    result
}

pub(super) fn resolve_pointer<'a>(
    root: &'a Value,
    reference: &str,
    path: &str,
) -> Result<&'a Value, String> {
    let Some(pointer) = reference.strip_prefix('#') else {
        return Err(format_error(path, "只支持本地 $ref"));
    };
    if pointer.is_empty() {
        return Ok(root);
    }
    let Some(pointer) = pointer.strip_prefix('/') else {
        return Err(format_error(path, "本地 $ref 必须使用 JSON Pointer"));
    };
    let mut current = root;
    for segment in pointer.split('/') {
        let segment = segment.replace("~1", "/").replace("~0", "~");
        current = current
            .get(&segment)
            .ok_or_else(|| format_error(path, "$ref 目标不存在"))?;
    }
    Ok(current)
}
