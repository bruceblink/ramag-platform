use std::collections::HashSet;

use serde_json::{Map, Value};

use super::super::{
    ApiAuth, ApiBody, ApiCollection, ApiEnvironment, ApiKeyLocation, ApiParameter,
    ApiRequestRecord, ApiWorkspace, HttpRequestSpec, MAX_API_ENVIRONMENT_NAME_BYTES,
    MAX_API_REQUEST_NAME_BYTES, MAX_API_REQUESTS,
};
use super::{
    ApiImportBundle, ApiImportFormat, MAX_API_IMPORT_WARNINGS, form_encode, format_error,
    header_value, make_bundle, optional_array, optional_bool, optional_string,
    optional_value_string, required_array, required_object, required_object_field, required_string,
    strip_query,
};

pub(super) fn import_postman_collection(value: &Value) -> Result<ApiImportBundle, String> {
    let root = required_object(value, "$")?;
    let info = required_object_field(root, "info", "$")?;
    let schema = required_string(info, "schema", "$.info")?;
    if !schema.contains("/collection/v2.1.0/") {
        return Err(format_error(
            "$.info.schema",
            "必须是 Postman Collection v2.1",
        ));
    }
    let collection_name = required_string(info, "name", "$.info")?;
    let items = required_array(root, "item", "$")?;
    let mut parser = PostmanParser {
        warnings: Vec::new(),
        request_count: 0,
    };
    let mut collection = ApiCollection::new(collection_name.clone());
    parser.parse_items(items, "$.item", "", &mut collection)?;
    collection
        .validate()
        .map_err(|error| format_error("$.item", &error))?;

    let environments =
        parse_postman_variables(root.get("variable"), &collection_name, &mut parser.warnings)?;
    let mut workspace = ApiWorkspace::new(collection_name);
    workspace.collections.push(collection);
    workspace.environments = environments;
    workspace.default_environment_id = workspace.environments.first().map(|item| item.id.clone());
    workspace
        .validate()
        .map_err(|error| format_error("$", &error))?;
    Ok(make_bundle(
        workspace,
        ApiImportFormat::PostmanCollectionV21,
        parser.warnings,
    ))
}

struct PostmanParser {
    warnings: Vec<String>,
    request_count: usize,
}

impl PostmanParser {
    fn parse_items(
        &mut self,
        items: &[Value],
        path: &str,
        folder: &str,
        collection: &mut ApiCollection,
    ) -> Result<(), String> {
        for (index, item) in items.iter().enumerate() {
            let item_path = format!("{path}[{index}]");
            let object = required_object(item, &item_path)?;
            let name = required_string(object, "name", &item_path)?;
            if let Some(children) = object.get("item") {
                let children = children
                    .as_array()
                    .ok_or_else(|| format_error(&format!("{item_path}.item"), "必须是数组"))?;
                let nested_folder = if folder.is_empty() {
                    name.clone()
                } else {
                    format!("{folder} / {name}")
                };
                self.parse_items(
                    children,
                    &format!("{item_path}.item"),
                    &nested_folder,
                    collection,
                )?;
                continue;
            }
            if object.get("request").is_none() {
                return Err(format_error(&item_path, "必须包含 request 或 item 字段"));
            }
            self.request_count = self.request_count.saturating_add(1);
            if self.request_count > MAX_API_REQUESTS {
                return Err(format_error(&item_path, "请求数量超过上限"));
            }
            let request_name = if folder.is_empty() {
                name
            } else {
                format!("{folder} / {name}")
            };
            let record = parse_postman_request(object, &item_path, request_name)?;
            collection.requests.push(record);
            if object.get("event").is_some() {
                self.warn(&format!("{item_path}.event"), "Postman 脚本未执行，已忽略");
            }
        }
        Ok(())
    }

    fn warn(&mut self, path: &str, message: &str) {
        if self.warnings.len() < MAX_API_IMPORT_WARNINGS {
            self.warnings.push(format!("{path}: {message}"));
        }
    }
}

fn parse_postman_request(
    item: &Map<String, Value>,
    item_path: &str,
    name: String,
) -> Result<ApiRequestRecord, String> {
    if name.len() > MAX_API_REQUEST_NAME_BYTES {
        return Err(format_error(
            &format!("{item_path}.name"),
            "请求名称超过长度上限",
        ));
    }
    let request_path = format!("{item_path}.request");
    let request = required_object_field(item, "request", item_path)?;
    let method = optional_string(request, "method", &request_path)?.unwrap_or_else(|| "GET".into());
    let url_value = request
        .get("url")
        .ok_or_else(|| format_error(&format!("{request_path}.url"), "字段缺失"))?;
    let (url, query) = parse_postman_url(url_value, &format!("{request_path}.url"))?;
    let headers = parse_postman_headers(request.get("header"), &request_path)?;
    let auth = parse_postman_auth(request.get("auth"), &request_path)?;
    let body = parse_postman_body(request.get("body"), &request_path, &headers)?;
    let mut spec = HttpRequestSpec::new(method, url);
    spec.query = query;
    spec.headers = headers;
    spec.auth = auth;
    spec.body = body;
    let record = ApiRequestRecord::new_http(name, spec);
    record
        .validate()
        .map_err(|error| format_error(&request_path, &error))?;
    Ok(record)
}

fn parse_postman_url(value: &Value, path: &str) -> Result<(String, Vec<ApiParameter>), String> {
    let Some(object) = value.as_object() else {
        let raw = value
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format_error(path, "必须是 URL 字符串或对象"))?;
        return Ok((raw.to_string(), Vec::new()));
    };
    let mut query = Vec::new();
    if let Some(values) = optional_array(object, "query", path)? {
        for (index, value) in values.iter().enumerate() {
            let query_path = format!("{path}.query[{index}]");
            let query_object = required_object(value, &query_path)?;
            if optional_bool(query_object, "disabled", &query_path)?.unwrap_or(false) {
                continue;
            }
            let name = required_string(query_object, "key", &query_path)?;
            let value =
                optional_value_string(query_object, "value", &query_path)?.unwrap_or_default();
            query.push(ApiParameter::new(
                name,
                value,
                optional_string(query_object, "type", &query_path)?.as_deref() == Some("secret"),
            ));
        }
    }
    let raw = optional_string(object, "raw", path)?;
    let url = match raw {
        Some(raw) if !query.is_empty() => strip_query(&raw),
        Some(raw) => raw,
        None => build_postman_url(object, path)?,
    };
    if url.trim().is_empty() {
        return Err(format_error(path, "URL 不能为空"));
    }
    Ok((url, query))
}

fn build_postman_url(object: &Map<String, Value>, path: &str) -> Result<String, String> {
    let protocol = optional_string(object, "protocol", path)?.unwrap_or_else(|| "http".into());
    let host = match object.get("host") {
        Some(Value::Array(parts)) => parts
            .iter()
            .enumerate()
            .map(|(index, value)| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format_error(&format!("{path}.host[{index}]"), "必须是字符串"))
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("."),
        Some(Value::String(host)) => host.clone(),
        Some(_) => return Err(format_error(&format!("{path}.host"), "必须是字符串或数组")),
        None => return Err(format_error(&format!("{path}.host"), "字段缺失")),
    };
    if host.trim().is_empty() {
        return Err(format_error(&format!("{path}.host"), "不能为空"));
    }
    let port = optional_string(object, "port", path)?;
    let path_part = match object.get("path") {
        Some(Value::Array(parts)) => parts
            .iter()
            .enumerate()
            .map(|(index, value)| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format_error(&format!("{path}.path[{index}]"), "必须是字符串"))
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("/"),
        Some(Value::String(path_part)) => path_part.trim_start_matches('/').to_string(),
        Some(_) => return Err(format_error(&format!("{path}.path"), "必须是字符串或数组")),
        None => String::new(),
    };
    let mut url = format!("{protocol}://{host}");
    if let Some(port) = port {
        url.push(':');
        url.push_str(&port);
    }
    if !path_part.is_empty() {
        url.push('/');
        url.push_str(&path_part);
    }
    Ok(url)
}

fn parse_postman_headers(value: Option<&Value>, path: &str) -> Result<Vec<ApiParameter>, String> {
    let Some(values) = value else {
        return Ok(Vec::new());
    };
    let values = values
        .as_array()
        .ok_or_else(|| format_error(&format!("{path}.header"), "必须是数组"))?;
    let mut headers = Vec::new();
    let mut names = HashSet::new();
    for (index, value) in values.iter().enumerate() {
        let header_path = format!("{path}.header[{index}]");
        let object = required_object(value, &header_path)?;
        if optional_bool(object, "disabled", &header_path)?.unwrap_or(false) {
            continue;
        }
        let name = required_string(object, "key", &header_path)?;
        if !names.insert(name.to_ascii_lowercase()) {
            return Err(format_error(
                &format!("{header_path}.key"),
                "Header 名称重复",
            ));
        }
        let value = optional_value_string(object, "value", &header_path)?.unwrap_or_default();
        let sensitive = optional_string(object, "type", &header_path)?.as_deref() == Some("secret");
        headers.push(ApiParameter::new(name, value, sensitive));
    }
    Ok(headers)
}

fn parse_postman_body(
    value: Option<&Value>,
    path: &str,
    headers: &[ApiParameter],
) -> Result<Option<ApiBody>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let object = required_object(value, &format!("{path}.body"))?;
    let body_path = format!("{path}.body");
    let mode = optional_string(object, "mode", &body_path)?.unwrap_or_default();
    match mode.as_str() {
        "" | "none" => Ok(None),
        "raw" => {
            let raw = optional_value_string(object, "raw", &body_path)?.unwrap_or_default();
            let content_type = header_value(headers, "content-type").or_else(|| {
                object
                    .get("options")
                    .and_then(Value::as_object)
                    .and_then(|options| options.get("raw"))
                    .and_then(Value::as_object)
                    .and_then(|raw| raw.get("language"))
                    .and_then(Value::as_str)
                    .filter(|language| language.eq_ignore_ascii_case("json"))
                    .map(|_| "application/json".to_string())
            });
            Ok(Some(ApiBody::text(raw, content_type)))
        }
        "urlencoded" => {
            let values = required_array(object, "urlencoded", &body_path)?;
            let mut pairs = Vec::new();
            for (index, value) in values.iter().enumerate() {
                let pair_path = format!("{body_path}.urlencoded[{index}]");
                let pair = required_object(value, &pair_path)?;
                if optional_bool(pair, "disabled", &pair_path)?.unwrap_or(false) {
                    continue;
                }
                let key = required_string(pair, "key", &pair_path)?;
                let value = optional_value_string(pair, "value", &pair_path)?.unwrap_or_default();
                pairs.push(format!("{}={}", form_encode(&key), form_encode(&value)));
            }
            Ok(Some(ApiBody::text(
                pairs.join("&"),
                Some("application/x-www-form-urlencoded".into()),
            )))
        }
        "formdata" | "file" | "binary" | "graphql" => Err(format_error(
            &format!("{body_path}.mode"),
            "该 Body 类型无法安全导入",
        )),
        _ => Err(format_error(
            &format!("{body_path}.mode"),
            "不支持的 Body 类型",
        )),
    }
}

fn parse_postman_auth(value: Option<&Value>, path: &str) -> Result<ApiAuth, String> {
    let Some(value) = value else {
        return Ok(ApiAuth::None);
    };
    let object = required_object(value, &format!("{path}.auth"))?;
    let auth_path = format!("{path}.auth");
    let kind = required_string(object, "type", &auth_path)?;
    match kind.as_str() {
        "noauth" => Ok(ApiAuth::None),
        "basic" => Ok(ApiAuth::Basic {
            username: auth_entry(object, "basic", "username", &auth_path)?,
            password: auth_entry(object, "basic", "password", &auth_path)?,
        }),
        "bearer" => Ok(ApiAuth::Bearer {
            token: auth_entry(object, "bearer", "token", &auth_path)?,
        }),
        "apikey" => {
            let location = match auth_entry(object, "apikey", "in", &auth_path)?.as_str() {
                "header" => ApiKeyLocation::Header,
                "query" => ApiKeyLocation::Query,
                _ => {
                    return Err(format_error(
                        &format!("{auth_path}.apikey.in"),
                        "只支持 header 或 query",
                    ));
                }
            };
            Ok(ApiAuth::ApiKey {
                name: auth_entry(object, "apikey", "key", &auth_path)?,
                value: auth_entry(object, "apikey", "value", &auth_path)?,
                location,
            })
        }
        _ => Err(format_error(
            &format!("{auth_path}.type"),
            "不支持的认证类型，未执行或猜测认证语义",
        )),
    }
}

fn auth_entry(
    auth: &Map<String, Value>,
    kind: &str,
    key: &str,
    path: &str,
) -> Result<String, String> {
    let entries = required_array(auth, kind, path)?;
    for entry in entries {
        let object = required_object(entry, &format!("{path}.{kind}"))?;
        if optional_bool(object, "disabled", path)?.unwrap_or(false) {
            continue;
        }
        if optional_string(object, "key", path)?.as_deref() == Some(key) {
            return optional_value_string(object, "value", path)?
                .ok_or_else(|| format_error(&format!("{path}.{kind}.{key}"), "value 字段缺失"));
        }
    }
    Err(format_error(
        &format!("{path}.{kind}.{key}"),
        "认证字段缺失",
    ))
}

fn parse_postman_variables(
    value: Option<&Value>,
    collection_name: &str,
    warnings: &mut Vec<String>,
) -> Result<Vec<ApiEnvironment>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| format_error("$.variable", "必须是数组"))?;
    let environment_name = format!("{collection_name} Environment");
    if environment_name.len() > MAX_API_ENVIRONMENT_NAME_BYTES {
        return Err(format_error("$.info.name", "导入环境名称超过长度上限"));
    }
    let mut environment = ApiEnvironment::new(environment_name);
    for (index, value) in values.iter().enumerate() {
        let path = format!("$.variable[{index}]");
        let object = required_object(value, &path)?;
        if optional_bool(object, "disabled", &path)?.unwrap_or(false) {
            continue;
        }
        let name = required_string(object, "key", &path)?;
        if environment.variables.contains_key(&name) {
            return Err(format_error(&format!("{path}.key"), "变量名称重复"));
        }
        let variable = match object.get("value") {
            None | Some(Value::Null) => {
                optional_value_string(object, "initialValue", &path)?.unwrap_or_default()
            }
            Some(_) => optional_value_string(object, "value", &path)?.unwrap_or_default(),
        };
        environment.variables.insert(name.clone(), variable);
        if optional_string(object, "type", &path)?.as_deref() == Some("secret") {
            environment.sensitive_variable_refs.push(name);
        }
    }
    environment
        .validate()
        .map_err(|error| format_error("$.variable", &error))?;
    if environment.variables.is_empty() {
        warnings.push("$.variable: 未发现启用的 Collection variables".into());
        return Ok(Vec::new());
    }
    Ok(vec![environment])
}
