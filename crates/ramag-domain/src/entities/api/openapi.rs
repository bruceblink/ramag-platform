use std::collections::HashMap;

use serde_json::{Map, Value};

use super::super::{
    ApiBody, ApiCollection, ApiEnvironment, ApiRequestRecord, ApiWorkspace, HttpRequestSpec,
    MAX_API_COLLECTION_NAME_BYTES, MAX_API_ENVIRONMENT_NAME_BYTES, MAX_API_PARAMETER_COUNT,
    MAX_API_REQUEST_NAME_BYTES, MAX_API_REQUESTS,
};
use super::{
    ApiImportBundle, ApiImportFormat, MAX_API_IMPORT_DEPTH, MAX_API_IMPORT_WARNINGS, format_error,
    make_bundle, optional_bool, optional_string, required_array, required_object,
    required_object_field, required_string,
};

#[path = "openapi_support.rs"]
mod support;

#[path = "openapi_security.rs"]
mod security;

use support::{
    apply_path_parameters, build_parameters, is_json_media_type, is_sensitive_name,
    join_server_and_path, merge_parameters, replace_server_variables, resolve_pointer, resolve_ref,
    validate_openapi_name, value_to_text,
};

const HTTP_METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

#[derive(Debug, Clone)]
struct ParameterDefinition {
    name: String,
    location: String,
    value: Option<String>,
    sensitive: bool,
}

struct OperationInput<'a> {
    method: &'a str,
    raw_path: &'a str,
    operation_path: &'a str,
    operation: &'a Map<String, Value>,
    parameters: Vec<ParameterDefinition>,
    server: Option<String>,
}

struct OpenApiParser<'a> {
    root: &'a Value,
    default_server: Option<String>,
    environment: ApiEnvironment,
    warnings: Vec<String>,
    request_count: usize,
}

pub(super) fn import_openapi_document(value: &Value) -> Result<ApiImportBundle, String> {
    let root = required_object(value, "$")?;
    let version = required_string(root, "openapi", "$")?;
    let version_prefix = version.split('.').take(2).collect::<Vec<_>>().join(".");
    if !matches!(version_prefix.as_str(), "3.0" | "3.1") {
        return Err(format_error("$.openapi", "只支持 OpenAPI 3.0 或 3.1"));
    }
    let info = required_object_field(root, "info", "$")?;
    let title = required_string(info, "title", "$.info")?;
    required_string(info, "version", "$.info")?;
    if title.len() > MAX_API_COLLECTION_NAME_BYTES {
        return Err(format_error("$.info.title", "标题超过长度上限"));
    }
    required_object_field(root, "paths", "$")?;

    let mut parser = OpenApiParser::new(value, &title)?;
    let mut collection = ApiCollection::new(title.clone());
    parser.parse_paths(&mut collection)?;
    if collection.requests.is_empty() {
        return Err(format_error("$.paths", "没有可导入的 HTTP Operation"));
    }
    collection
        .validate()
        .map_err(|error| format_error("$.paths", &error))?;

    let mut workspace = ApiWorkspace::new(title.clone());
    workspace.collections.push(collection);
    if !parser.environment.variables.is_empty() {
        parser
            .environment
            .validate()
            .map_err(|error| format_error("$.servers", &error))?;
        workspace.default_environment_id = Some(parser.environment.id.clone());
        workspace.environments.push(parser.environment);
    }
    workspace
        .validate()
        .map_err(|error| format_error("$", &error))?;
    Ok(make_bundle(
        workspace,
        ApiImportFormat::OpenApi3Json,
        parser.warnings,
    ))
}

impl<'a> OpenApiParser<'a> {
    /// 初始化文档级服务器和环境；服务器变量会成为请求模板可展开的环境变量。
    fn new(root: &'a Value, title: &str) -> Result<Self, String> {
        let environment_name = format!("{title} Environment");
        if environment_name.len() > MAX_API_ENVIRONMENT_NAME_BYTES {
            return Err(format_error("$.info.title", "导入环境名称超过长度上限"));
        }
        let mut parser = Self {
            root,
            default_server: None,
            environment: ApiEnvironment::new(environment_name),
            warnings: Vec::new(),
            request_count: 0,
        };
        parser.default_server =
            parser.parse_server_list(root.get("servers").cloned(), "$.servers")?;
        if parser.default_server.is_none() {
            parser.warn("$.servers", "未声明服务器，导入请求将使用相对 URL");
        }
        Ok(parser)
    }

    /// 遍历 Path Item，只把标准 HTTP Operation 转换为可执行请求记录。
    fn parse_paths(&mut self, collection: &mut ApiCollection) -> Result<(), String> {
        let paths = required_object_field(self.root_object()?, "paths", "$")?.clone();
        for (raw_path, raw_item) in paths {
            let item_path = format!("$.paths.{raw_path}");
            if raw_path.starts_with("x-") {
                continue;
            }
            if !raw_path.starts_with('/') {
                return Err(format_error(&item_path, "Path 必须以 '/' 开头"));
            }
            let item = resolve_ref(self.root, &raw_item, &item_path)?;
            let item = required_object(&item, &item_path)?.clone();
            let path_server = self.parse_server_list(
                item.get("servers").cloned(),
                &format!("{item_path}.servers"),
            )?;
            let path_parameters = self.parse_parameters(
                item.get("parameters").cloned(),
                &format!("{item_path}.parameters"),
            )?;
            for method in HTTP_METHODS {
                let Some(raw_operation) = item.get(method) else {
                    continue;
                };
                let operation_path = format!("{item_path}.{method}");
                let operation = resolve_ref(self.root, raw_operation, &operation_path)?;
                let operation = required_object(&operation, &operation_path)?.clone();
                let operation_server = self.parse_server_list(
                    operation.get("servers").cloned(),
                    &format!("{operation_path}.servers"),
                )?;
                let parameters = merge_parameters(
                    &path_parameters,
                    &self.parse_parameters(
                        operation.get("parameters").cloned(),
                        &format!("{operation_path}.parameters"),
                    )?,
                )?;
                self.parse_operation(
                    OperationInput {
                        method,
                        raw_path: &raw_path,
                        operation_path: &operation_path,
                        operation: &operation,
                        parameters,
                        server: operation_server
                            .or(path_server.clone())
                            .or(self.default_server.clone()),
                    },
                    collection,
                )?;
            }
        }
        Ok(())
    }

    /// 把一个 Operation 映射为 HTTP 请求、参数、认证和可选 JSON 正文。
    fn parse_operation(
        &mut self,
        input: OperationInput<'_>,
        collection: &mut ApiCollection,
    ) -> Result<(), String> {
        let OperationInput {
            method,
            raw_path,
            operation_path,
            operation,
            parameters,
            server,
        } = input;
        self.request_count = self.request_count.saturating_add(1);
        if self.request_count > MAX_API_REQUESTS {
            return Err(format_error(operation_path, "请求数量超过上限"));
        }
        let path = apply_path_parameters(raw_path, &parameters, operation_path)?;
        let url = join_server_and_path(server.as_deref(), &path);
        let (query, headers) = build_parameters(&parameters, operation_path)?;
        let body = self.parse_request_body(
            operation.get("requestBody").cloned(),
            &format!("{operation_path}.requestBody"),
        )?;
        let mut spec = HttpRequestSpec::new(method.to_ascii_uppercase(), url);
        spec.query = query;
        spec.headers = headers;
        spec.body = body;
        for field in ["callbacks", "links", "externalDocs"] {
            if operation.contains_key(field) {
                self.warn(
                    &format!("{operation_path}.{field}"),
                    "字段未映射，导入请求不会执行该扩展行为",
                );
            }
        }
        self.apply_security(operation, operation_path, &mut spec)?;
        let name = optional_string(operation, "operationId", operation_path)?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("{} {raw_path}", method.to_ascii_uppercase()));
        if name.len() > MAX_API_REQUEST_NAME_BYTES {
            return Err(format_error(
                &format!("{operation_path}.operationId"),
                "请求名称超过长度上限",
            ));
        }
        let record = ApiRequestRecord::new_http(name, spec);
        record
            .validate()
            .map_err(|error| format_error(operation_path, &error))?;
        collection.requests.push(record);
        Ok(())
    }

    /// 选择第一个服务器，并将 OpenAPI 的单括号变量改成工作台模板。
    fn parse_server_list(
        &mut self,
        value: Option<Value>,
        path: &str,
    ) -> Result<Option<String>, String> {
        let Some(value) = value else {
            return Ok(None);
        };
        let servers = value
            .as_array()
            .ok_or_else(|| format_error(path, "必须是数组"))?;
        let Some(server) = servers.first() else {
            return Err(format_error(path, "至少需要一个服务器"));
        };
        if servers.len() > 1 {
            self.warn(path, "仅导入第一个服务器，其余服务器未导入");
        }
        let server_path = format!("{path}[0]");
        let server = resolve_ref(self.root, server, &server_path)?;
        let object = required_object(&server, &server_path)?;
        let url = required_string(object, "url", &server_path)?;
        let variables = match object.get("variables") {
            None => HashMap::new(),
            Some(value) => {
                let variables = required_object(value, &format!("{server_path}.variables"))?;
                let mut defaults = HashMap::with_capacity(variables.len());
                for (name, value) in variables {
                    let variable_path = format!("{server_path}.variables.{name}");
                    let variable = resolve_ref(self.root, value, &variable_path)?;
                    let variable = required_object(&variable, &variable_path)?;
                    let default = match variable.get("default") {
                        Some(_) => required_string(variable, "default", &variable_path)?,
                        None => {
                            self.warn(
                                &format!("{variable_path}.default"),
                                "缺少默认值，使用空字符串并等待编辑",
                            );
                            String::new()
                        }
                    };
                    self.ensure_variable(name, &default, &variable_path, false)?;
                    defaults.insert(name.clone(), default);
                }
                defaults
            }
        };
        Ok(Some(replace_server_variables(&url, &variables, path)?))
    }

    /// 解析 Path/Operation 参数，保留示例优先级并为缺失必填值创建空环境变量。
    fn parse_parameters(
        &mut self,
        value: Option<Value>,
        path: &str,
    ) -> Result<Vec<ParameterDefinition>, String> {
        let Some(value) = value else {
            return Ok(Vec::new());
        };
        let values = value
            .as_array()
            .ok_or_else(|| format_error(path, "必须是数组"))?;
        if values.len() > MAX_API_PARAMETER_COUNT {
            return Err(format_error(path, "参数数量超过上限"));
        }
        let mut definitions = Vec::with_capacity(values.len());
        for (index, raw_parameter) in values.iter().enumerate() {
            let parameter_path = format!("{path}[{index}]");
            let parameter = resolve_ref(self.root, raw_parameter, &parameter_path)?;
            let object = required_object(&parameter, &parameter_path)?;
            let name = required_string(object, "name", &parameter_path)?;
            validate_openapi_name(&name, &format!("{parameter_path}.name"))?;
            let location = required_string(object, "in", &parameter_path)?;
            if !matches!(location.as_str(), "path" | "query" | "header" | "cookie") {
                return Err(format_error(
                    &format!("{parameter_path}.in"),
                    "只支持 path、query、header 和 cookie",
                ));
            }
            let required = optional_bool(object, "required", &parameter_path)?.unwrap_or(false);
            if location == "path" && !required {
                return Err(format_error(
                    &format!("{parameter_path}.required"),
                    "Path 参数必须是 true",
                ));
            }
            if object.contains_key("content") {
                self.warn(
                    &format!("{parameter_path}.content"),
                    "参数 content 未映射，已跳过该参数",
                );
                continue;
            }
            for field in ["style", "explode", "allowReserved", "allowEmptyValue"] {
                if object.contains_key(field) {
                    self.warn(
                        &format!("{parameter_path}.{field}"),
                        "参数序列化选项未映射，使用工作台默认序列化",
                    );
                }
            }
            let schema = match object.get("schema") {
                None => None,
                Some(value) => Some(resolve_ref(
                    self.root,
                    value,
                    &format!("{parameter_path}.schema"),
                )?),
            };
            let sample = self.example_value(object, schema.as_ref(), &parameter_path)?;
            let value = match sample {
                Some(value) => Some(value_to_text(&value, &parameter_path)?),
                None if location == "path" || required => {
                    self.ensure_variable(&name, "", &parameter_path, is_sensitive_name(&name))?;
                    Some(format!("{{{{{name}}}}}"))
                }
                None => None,
            };
            definitions.push(ParameterDefinition {
                name: name.clone(),
                location,
                value,
                sensitive: is_sensitive_name(&name),
            });
        }
        Ok(definitions)
    }

    /// 按 OpenAPI 的 example/examples/schema 顺序选一个值，并生成有界 Schema 样例。
    fn example_value(
        &mut self,
        object: &Map<String, Value>,
        schema: Option<&Value>,
        path: &str,
    ) -> Result<Option<Value>, String> {
        if let Some(value) = object.get("example") {
            return Ok(Some(value.clone()));
        }
        if let Some(examples) = object.get("examples")
            && let Some(value) = self.first_named_example(examples, &format!("{path}.examples"))?
        {
            return Ok(Some(value));
        }
        schema.map_or(Ok(None), |schema| {
            let mut references = Vec::new();
            self.schema_example(schema, &format!("{path}.schema"), 0, &mut references)
        })
    }

    /// 读取一个命名 examples 对象；externalValue 无法安全读取时只留下具体提示。
    fn first_named_example(&mut self, value: &Value, path: &str) -> Result<Option<Value>, String> {
        let examples = required_object(value, path)?;
        for (name, raw_example) in examples {
            let example_path = format!("{path}.{name}");
            let example = resolve_ref(self.root, raw_example, &example_path)?;
            let object = required_object(&example, &example_path)?;
            if let Some(value) = object.get("value") {
                return Ok(Some(value.clone()));
            }
            if object.get("externalValue").is_some() {
                self.warn(&example_path, "externalValue 未读取");
            }
        }
        Ok(None)
    }

    /// 解析 requestBody，只选择 JSON 媒体类型并按示例优先生成正文。
    fn parse_request_body(
        &mut self,
        value: Option<Value>,
        path: &str,
    ) -> Result<Option<ApiBody>, String> {
        let Some(value) = value else {
            return Ok(None);
        };
        let value = resolve_ref(self.root, &value, path)?;
        let object = required_object(&value, path)?;
        let content = required_object_field(object, "content", path)?;
        let mut selected: Option<(String, Value, String)> = None;
        for (media_type, raw_media) in content {
            if !is_json_media_type(media_type) {
                self.warn(
                    &format!("{path}.content.{media_type}"),
                    "非 JSON 媒体类型未映射",
                );
                continue;
            }
            if selected.is_none() {
                let media_path = format!("{path}.content.{media_type}");
                selected = Some((
                    media_type.clone(),
                    resolve_ref(self.root, raw_media, &media_path)?,
                    media_path,
                ));
            }
        }
        let Some((media_type, media, media_path)) = selected else {
            self.warn(path, "没有可安全映射的 JSON requestBody");
            return Ok(None);
        };
        let media = required_object(&media, &media_path)?;
        if media.contains_key("encoding") {
            self.warn(
                &format!("{media_path}.encoding"),
                "JSON Body 的 encoding 未映射",
            );
        }
        let schema = match media.get("schema") {
            None => None,
            Some(value) => Some(resolve_ref(
                self.root,
                value,
                &format!("{media_path}.schema"),
            )?),
        };
        let example = self.example_value(media, schema.as_ref(), &media_path)?;
        let value = example.unwrap_or_else(|| Value::Object(Map::new()));
        let body = serde_json::to_string(&value)
            .map_err(|error| format_error(&media_path, &format!("JSON 示例无法序列化：{error}")))?;
        Ok(Some(ApiBody::text(body, Some(media_type))))
    }

    /// 根据有限 Schema 生成最小 JSON 样例，避免导入后正文为空而无法直接编辑。
    fn schema_example(
        &mut self,
        value: &Value,
        path: &str,
        depth: usize,
        references: &mut Vec<String>,
    ) -> Result<Option<Value>, String> {
        if depth > MAX_API_IMPORT_DEPTH {
            return Err(format_error(path, "Schema 嵌套深度超过限制"));
        }
        if let Some(object) = value.as_object()
            && let Some(reference) = object.get("$ref")
        {
            let reference = reference
                .as_str()
                .ok_or_else(|| format_error(&format!("{path}.$ref"), "必须是字符串"))?;
            if references.iter().any(|item| item == reference) {
                return Err(format_error(path, "Schema $ref 存在循环引用"));
            }
            let target = resolve_pointer(self.root, reference, &format!("{path}.$ref"))?;
            references.push(reference.to_string());
            let result = self.schema_example(target, path, depth + 1, references);
            references.pop();
            return result;
        }
        let object = required_object(value, path)?;
        if let Some(example) = object.get("example") {
            return Ok(Some(example.clone()));
        }
        if let Some(default) = object.get("default") {
            return Ok(Some(default.clone()));
        }
        for field in ["allOf", "not", "discriminator"] {
            if object.contains_key(field) {
                self.warn(
                    &format!("{path}.{field}"),
                    "Schema 字段未完整映射，示例生成将忽略该字段",
                );
            }
        }
        if object.get("enum").is_some() {
            let values = required_array(object, "enum", path)?;
            if let Some(value) = values.first() {
                return Ok(Some(value.clone()));
            }
        }
        for key in ["oneOf", "anyOf"] {
            if object.get(key).is_some() {
                let values = required_array(object, key, path)?;
                if let Some(first) = values.first() {
                    self.warn(&format!("{path}.{key}"), "仅使用第一个 Schema 分支生成示例");
                    return self.schema_example(
                        first,
                        &format!("{path}.{key}[0]"),
                        depth + 1,
                        references,
                    );
                }
            }
        }
        let schema_type = object.get("type").and_then(Value::as_str);
        match schema_type {
            Some("object") | None if object.contains_key("properties") => {
                let properties = object
                    .get("properties")
                    .ok_or_else(|| format_error(&format!("{path}.properties"), "字段缺失"))?;
                let properties = required_object(properties, &format!("{path}.properties"))?;
                let mut sample = Map::new();
                for (name, property) in properties {
                    let property_path = format!("{path}.properties.{name}");
                    if let Some(value) =
                        self.schema_example(property, &property_path, depth + 1, references)?
                    {
                        sample.insert(name.clone(), value);
                    }
                }
                Ok(Some(Value::Object(sample)))
            }
            Some("array") => {
                let Some(items) = object.get("items") else {
                    return Ok(Some(Value::Array(Vec::new())));
                };
                let item =
                    self.schema_example(items, &format!("{path}.items"), depth + 1, references)?;
                Ok(Some(Value::Array(
                    item.map_or_else(Vec::new, |value| vec![value]),
                )))
            }
            Some("string") => Ok(Some(Value::String(String::new()))),
            Some("integer") | Some("number") => Ok(Some(Value::from(0))),
            Some("boolean") => Ok(Some(Value::Bool(false))),
            Some("null") => Ok(Some(Value::Null)),
            _ => Ok(None),
        }
    }

    fn root_object(&self) -> Result<&Map<String, Value>, String> {
        required_object(self.root, "$")
    }

    fn warn(&mut self, path: &str, message: &str) {
        if self.warnings.len() < MAX_API_IMPORT_WARNINGS {
            self.warnings.push(format!("{path}: {message}"));
        }
    }
}
