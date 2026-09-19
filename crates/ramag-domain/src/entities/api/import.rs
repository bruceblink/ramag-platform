use std::collections::HashMap;

use serde_json::{Map, Value};

use super::{
    ApiCollectionId, ApiEnvironment, ApiEnvironmentId, ApiParameter, ApiRequestId, ApiWorkspace,
    MAX_API_ENVIRONMENT_NAME_BYTES, MAX_API_ENVIRONMENTS,
};

pub const MAX_API_IMPORT_DEPTH: usize = 64;
pub const MAX_API_IMPORT_BYTES: usize = 8 * 1024 * 1024;
const MAX_API_IMPORT_WARNINGS: usize = 128;

#[path = "postman.rs"]
mod postman;

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiImportFormat {
    RamagJson,
    PostmanCollectionV21,
}

impl ApiImportFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::RamagJson => "Ramag JSON",
            Self::PostmanCollectionV21 => "Postman Collection v2.1",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiImportSummary {
    pub format: ApiImportFormat,
    pub workspace_name: String,
    pub collection_count: usize,
    pub request_count: usize,
    pub environment_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiImportBundle {
    workspace: ApiWorkspace,
    summary: ApiImportSummary,
}

impl ApiImportBundle {
    pub fn summary(&self) -> &ApiImportSummary {
        &self.summary
    }

    /// 将导入内容复制到目标工作区并重建本地 ID；校验失败时目标不会被修改。
    pub fn merge_into(self, target: &mut ApiWorkspace) -> Result<ApiImportSummary, String> {
        let ApiImportBundle {
            mut workspace,
            summary,
        } = self;
        let mut merged = target.clone();
        let imported_default = workspace.default_environment_id.take();
        let mut environment_ids = HashMap::with_capacity(workspace.environments.len());
        for mut environment in workspace.environments {
            let old_id = environment.id.clone();
            environment.id = ApiEnvironmentId::new();
            environment.name = unique_environment_name(&environment.name, &merged.environments)?;
            environment_ids.insert(old_id, environment.id.clone());
            merged.environments.push(environment);
        }
        for mut collection in workspace.collections {
            collection.id = ApiCollectionId::new();
            for request in &mut collection.requests {
                request.id = ApiRequestId::new();
            }
            merged.collections.push(collection);
        }
        if merged.default_environment_id.is_none()
            && let Some(imported_default) = imported_default
        {
            merged.default_environment_id = environment_ids.get(&imported_default).cloned();
        }
        merged.validate()?;
        *target = merged;
        Ok(summary)
    }
}

/// 识别并解析有界的 Ramag JSON 或 Postman Collection v2.1 JSON。
pub fn import_api_json(raw: &str) -> Result<ApiImportBundle, String> {
    if raw.len() > MAX_API_IMPORT_BYTES {
        return Err(format!(
            "API 导入文件超过 {MAX_API_IMPORT_BYTES} bytes 上限"
        ));
    }
    let value: Value =
        serde_json::from_str(raw).map_err(|error| format!("JSON 根节点解析失败：{error}"))?;
    validate_json_depth(&value, 0, "$")?;
    if is_postman_collection(&value) {
        postman::import_postman_collection(&value)
    } else {
        import_ramag_workspace(&value)
    }
}

fn is_postman_collection(value: &Value) -> bool {
    value.get("info").is_some() && value.get("item").is_some()
}

fn import_ramag_workspace(value: &Value) -> Result<ApiImportBundle, String> {
    let workspace_value = if value.get("workspace").is_some() {
        let object = required_object(value, "$")?;
        let format = required_string(object, "format", "$")?;
        if format != "ramag-api" {
            return Err(format_error("$.format", "只支持 ramag-api"));
        }
        let version = value
            .get("version")
            .and_then(Value::as_u64)
            .ok_or_else(|| format_error("$.version", "必须是数字 1"))?;
        if version != 1 {
            return Err(format_error("$.version", "只支持版本 1"));
        }
        value
            .get("workspace")
            .ok_or_else(|| format_error("$.workspace", "字段缺失"))?
    } else {
        value
    };
    validate_ramag_shape(workspace_value, "$.workspace")?;
    let workspace: ApiWorkspace = serde_json::from_value(workspace_value.clone())
        .map_err(|error| format_error("$.workspace", &format!("字段无法读取：{error}")))?;
    workspace
        .validate()
        .map_err(|error| format_error("$.workspace", &error))?;
    Ok(make_bundle(
        workspace,
        ApiImportFormat::RamagJson,
        Vec::new(),
    ))
}

fn validate_ramag_shape(value: &Value, path: &str) -> Result<(), String> {
    let object = required_object(value, path)?;
    required_string(object, "name", path)?;
    let collections = required_array(object, "collections", path)?;
    for (index, collection) in collections.iter().enumerate() {
        let collection_path = format!("{path}.collections[{index}]");
        let collection_object = required_object(collection, &collection_path)?;
        required_string(collection_object, "name", &collection_path)?;
        let requests = required_array(collection_object, "requests", &collection_path)?;
        for (request_index, request) in requests.iter().enumerate() {
            let request_path = format!("{collection_path}.requests[{request_index}]");
            let request_object = required_object(request, &request_path)?;
            required_string(request_object, "name", &request_path)?;
            required_string(request_object, "protocol", &request_path)?;
            required_object_field(request_object, "request", &request_path)?;
        }
    }
    let environments = optional_array(object, "environments", path)?;
    if let Some(environments) = environments {
        if environments.len() > MAX_API_ENVIRONMENTS {
            return Err(format_error(
                &format!("{path}.environments"),
                "数量超过导入限制",
            ));
        }
        for (index, environment) in environments.iter().enumerate() {
            let environment_path = format!("{path}.environments[{index}]");
            let environment_object = required_object(environment, &environment_path)?;
            required_string(environment_object, "name", &environment_path)?;
        }
    }
    Ok(())
}

fn make_bundle(
    workspace: ApiWorkspace,
    format: ApiImportFormat,
    warnings: Vec<String>,
) -> ApiImportBundle {
    let summary = ApiImportSummary {
        format,
        workspace_name: workspace.name.clone(),
        collection_count: workspace.collections.len(),
        request_count: workspace
            .collections
            .iter()
            .map(|collection| collection.requests.len())
            .sum(),
        environment_count: workspace.environments.len(),
        warnings: warnings.into_iter().take(MAX_API_IMPORT_WARNINGS).collect(),
    };
    ApiImportBundle { workspace, summary }
}

fn unique_environment_name(name: &str, environments: &[ApiEnvironment]) -> Result<String, String> {
    if !environments.iter().any(|item| item.name == name) {
        return Ok(name.to_string());
    }
    let suffix = " (导入)";
    let candidate = format!("{name}{suffix}");
    if candidate.len() <= MAX_API_ENVIRONMENT_NAME_BYTES
        && !environments.iter().any(|item| item.name == candidate)
    {
        return Ok(candidate);
    }
    for index in 2..=1000 {
        let candidate = format!("{name}{suffix} {index}");
        if candidate.len() > MAX_API_ENVIRONMENT_NAME_BYTES {
            break;
        }
        if !environments.iter().any(|item| item.name == candidate) {
            return Ok(candidate);
        }
    }
    Err(format!("导入环境名称冲突且无法生成不重复名称：{name}"))
}

fn validate_json_depth(value: &Value, depth: usize, path: &str) -> Result<(), String> {
    if depth > MAX_API_IMPORT_DEPTH {
        return Err(format_error(path, "JSON 嵌套深度超过限制"));
    }
    match value {
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_json_depth(value, depth + 1, &format!("{path}[{index}]"))?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                validate_json_depth(value, depth + 1, &format!("{path}.{key}"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn required_object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format_error(path, "必须是对象"))
}

fn required_object_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Map<String, Value>, String> {
    let value = object
        .get(key)
        .ok_or_else(|| format_error(&format!("{path}.{key}"), "字段缺失"))?;
    required_object(value, &format!("{path}.{key}"))
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Vec<Value>, String> {
    let value = object
        .get(key)
        .ok_or_else(|| format_error(&format!("{path}.{key}"), "字段缺失"))?;
    value
        .as_array()
        .ok_or_else(|| format_error(&format!("{path}.{key}"), "必须是数组"))
}

fn optional_array<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<&'a Vec<Value>>, String> {
    match object.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_array()
            .map(Some)
            .ok_or_else(|| format_error(&format!("{path}.{key}"), "必须是数组")),
    }
}

fn required_string(object: &Map<String, Value>, key: &str, path: &str) -> Result<String, String> {
    let value = object
        .get(key)
        .ok_or_else(|| format_error(&format!("{path}.{key}"), "字段缺失"))?;
    let value = value
        .as_str()
        .ok_or_else(|| format_error(&format!("{path}.{key}"), "必须是字符串"))?;
    if value.trim().is_empty() {
        return Err(format_error(&format!("{path}.{key}"), "不能为空"));
    }
    Ok(value.to_string())
}

fn optional_string(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<String>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_string()))
            .ok_or_else(|| format_error(&format!("{path}.{key}"), "必须是字符串")),
    }
}

fn optional_value_string(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<String>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(value) => Err(format_error(
            &format!("{path}.{key}"),
            &format!("必须是字符串，实际为 {}", value_type(value)),
        )),
    }
}

fn optional_bool(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<bool>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| format_error(&format!("{path}.{key}"), "必须是布尔值")),
    }
}

fn header_value(headers: &[ApiParameter], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.clone())
}

fn strip_query(raw: &str) -> String {
    let (base, fragment) = raw
        .split_once('#')
        .map_or((raw, ""), |(base, fragment)| (base, fragment));
    let base = base.split_once('?').map_or(base, |(base, _)| base);
    if fragment.is_empty() {
        base.to_string()
    } else {
        format!("{base}#{fragment}")
    }
}

fn form_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(hex_digit(byte >> 4));
            encoded.push(hex_digit(byte & 0x0f));
        }
    }
    encoded
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        _ => char::from(b'A' + value - 10),
    }
}

fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "布尔值",
        Value::Number(_) => "数字",
        Value::String(_) => "字符串",
        Value::Array(_) => "数组",
        Value::Object(_) => "对象",
    }
}

fn format_error(path: &str, message: &str) -> String {
    format!("导入字段 {path}：{message}")
}
