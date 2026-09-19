use super::*;
use ramag_domain::entities::{
    ApiAssertion, ApiEnvironment, ApiRequestRecord, ApiRequestSpec, ApiWorkspace,
};

pub(crate) fn environment_from_view(view: &ApiView, cx: &App) -> Result<ApiEnvironment> {
    parse_environment(
        &input_value(&view.environment_variables, cx),
        &input_value(&view.environment_sensitive, cx),
    )
}

pub(crate) fn parse_environment(variables: &str, sensitive: &str) -> Result<ApiEnvironment> {
    let mut environment = ApiEnvironment::new("local");
    for (index, line) in variables.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            return Err(DomainError::InvalidConfig(format!(
                "环境变量第 {} 行必须使用 name=value 格式",
                index + 1
            )));
        };
        let name = name.trim();
        if environment
            .variables
            .insert(name.to_string(), value.trim().to_string())
            .is_some()
        {
            return Err(DomainError::InvalidConfig(format!(
                "环境变量名称重复：{name}"
            )));
        }
    }
    for (index, line) in sensitive.lines().enumerate() {
        let name = line.trim();
        if name.is_empty() {
            continue;
        }
        if !environment.variables.contains_key(name) {
            return Err(DomainError::InvalidConfig(format!(
                "敏感变量第 {} 行未找到变量：{name}",
                index + 1
            )));
        }
        environment.sensitive_variable_refs.push(name.to_string());
    }
    environment.validate().map_err(DomainError::InvalidConfig)?;
    Ok(environment)
}

pub(crate) fn assertions_from_view(view: &ApiView, cx: &App) -> Result<Vec<ApiAssertion>> {
    parse_assertions(&input_value(&view.assertions, cx), view.protocol)
}

pub(crate) fn parse_assertions(value: &str, protocol: ApiProtocol) -> Result<Vec<ApiAssertion>> {
    let mut assertions = Vec::new();
    for (index, line) in value.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((kind, expected)) = line.split_once('=') else {
            return Err(DomainError::InvalidConfig(format!(
                "断言第 {} 行必须使用 type=value 格式",
                index + 1
            )));
        };
        let assertion = match kind.trim().to_ascii_lowercase().as_str() {
            "status" => ApiAssertion::HttpStatus {
                expected: parse_status(expected, index)?,
            },
            "header" => {
                let (name, value) = split_pair(expected, "Header", index)?;
                ApiAssertion::HeaderEquals {
                    name: name.to_string(),
                    expected: value.to_string(),
                }
            }
            "metadata" => {
                let (name, value) = split_pair(expected, "Metadata", index)?;
                ApiAssertion::MetadataEquals {
                    name: name.to_string(),
                    expected: value.to_string(),
                }
            }
            "body" => ApiAssertion::BodyContains {
                expected: expected.trim().to_string(),
            },
            "json" => {
                let (path, value) = split_pair(expected, "JSON Path", index)?;
                ApiAssertion::JsonPathEquals {
                    path: path.to_string(),
                    expected: value.to_string(),
                }
            }
            "latency" => ApiAssertion::LatencyAtMostMillis {
                expected: parse_number(expected, "耗时", index)?,
            },
            other => {
                return Err(DomainError::InvalidConfig(format!(
                    "断言第 {} 行类型不支持：{other}",
                    index + 1
                )));
            }
        };
        assertion
            .validate(protocol)
            .map_err(DomainError::InvalidConfig)?;
        assertions.push(assertion);
    }
    Ok(assertions)
}

fn parse_number(value: &str, label: &str, index: usize) -> Result<u64> {
    value.trim().parse::<u64>().map_err(|_| {
        DomainError::InvalidConfig(format!("断言第 {} 行的 {label} 必须是数字", index + 1))
    })
}

fn parse_status(value: &str, index: usize) -> Result<u16> {
    value.trim().parse::<u16>().map_err(|_| {
        DomainError::InvalidConfig(format!("断言第 {} 行的 HTTP 状态必须是数字", index + 1))
    })
}

fn split_pair<'a>(value: &'a str, label: &str, index: usize) -> Result<(&'a str, &'a str)> {
    let Some((name, expected)) = value.split_once(':') else {
        return Err(DomainError::InvalidConfig(format!(
            "断言第 {} 行的 {label} 必须使用 name:value 格式",
            index + 1
        )));
    };
    let name = name.trim();
    let expected = expected.trim();
    if name.is_empty() || expected.is_empty() {
        return Err(DomainError::InvalidConfig(format!(
            "断言第 {} 行的 {label} 名称和值不能为空",
            index + 1
        )));
    }
    Ok((name, expected))
}

pub(crate) fn request_record(view: &ApiView, cx: &App) -> Result<ApiRequestRecord> {
    let name = input_value(&view.request_name, cx);
    let request = request_from_view(view, cx)?;
    let assertions = assertions_from_view(view, cx)?;
    let mut record = match request {
        ApiRequestSpec::Http(spec) => ApiRequestRecord::new_http(name, spec),
        ApiRequestSpec::Grpc(spec) => ApiRequestRecord::new_grpc(name, spec),
    };
    record.assertions = assertions;
    record.validate().map_err(DomainError::InvalidConfig)?;
    Ok(record)
}

pub(crate) fn upsert_environment(workspace: &mut ApiWorkspace, environment: ApiEnvironment) {
    if let Some(existing) = workspace
        .environments
        .iter_mut()
        .find(|existing| existing.name == environment.name)
    {
        let id = existing.id.clone();
        *existing = environment;
        existing.id = id.clone();
        workspace.default_environment_id = Some(id);
    } else {
        workspace.default_environment_id = Some(environment.id.clone());
        workspace.environments.push(environment);
    }
}

/// 将导入后的首个请求和默认环境同步到编辑器，确保导入结果立即可见且再次保存不会丢失认证信息。
pub(crate) fn apply_imported_workspace(
    view: &mut ApiView,
    workspace: &ApiWorkspace,
    window: &mut Window,
    cx: &mut Context<ApiView>,
) {
    if let Some(environment) = workspace.default_environment_id.as_ref().and_then(|id| {
        workspace
            .environments
            .iter()
            .find(|environment| &environment.id == id)
    }) {
        set_input(
            &view.environment_variables,
            environment
                .variables
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join("\n"),
            window,
            cx,
        );
        set_input(
            &view.environment_sensitive,
            environment.sensitive_variable_refs.join("\n"),
            window,
            cx,
        );
    }

    let Some(request) = workspace
        .collections
        .iter()
        .flat_map(|collection| collection.requests.iter())
        .next()
    else {
        return;
    };
    set_input(&view.request_name, request.name.clone(), window, cx);
    set_input(
        &view.assertions,
        format_assertions(&request.assertions),
        window,
        cx,
    );
    match &request.request {
        ApiRequestSpec::Http(spec) => {
            view.protocol = ApiProtocol::Http;
            view.http_auth = spec.auth.clone();
            view.http_body_content_type = spec
                .body
                .as_ref()
                .and_then(|body| body.content_type.clone())
                .unwrap_or_else(|| "application/json".into());
            set_input(&view.http_method, spec.method.clone(), window, cx);
            set_input(&view.http_url, spec.url_template.clone(), window, cx);
            set_input(
                &view.http_headers,
                format_parameters(&spec.headers),
                window,
                cx,
            );
            set_input(
                &view.http_body,
                spec.body
                    .as_ref()
                    .map(|body| body.value.clone())
                    .unwrap_or_default(),
                window,
                cx,
            );
        }
        ApiRequestSpec::Grpc(spec) => {
            view.protocol = ApiProtocol::Grpc;
            view.http_auth = ApiAuth::None;
            view.http_body_content_type = "application/json".into();
            set_input(
                &view.grpc_endpoint,
                spec.endpoint_template.clone(),
                window,
                cx,
            );
            set_input(&view.grpc_service, spec.service.clone(), window, cx);
            set_input(&view.grpc_method, spec.method.clone(), window, cx);
            set_input(&view.grpc_message, spec.message.clone(), window, cx);
            if let Some(metadata) = spec.metadata.first() {
                set_input(&view.grpc_metadata_name, metadata.name.clone(), window, cx);
                set_input(
                    &view.grpc_metadata_value,
                    metadata.value.clone(),
                    window,
                    cx,
                );
            }
        }
    }
}

fn set_input(
    field: &Entity<InputState>,
    value: String,
    window: &mut Window,
    cx: &mut Context<ApiView>,
) {
    field.update(cx, |input, cx| input.set_value(value, window, cx));
}

fn format_parameters(parameters: &[ApiParameter]) -> String {
    parameters
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name, parameter.value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_assertions(assertions: &[ApiAssertion]) -> String {
    assertions
        .iter()
        .map(|assertion| match assertion {
            ApiAssertion::HttpStatus { expected } => format!("status={expected}"),
            ApiAssertion::HeaderEquals { name, expected } => {
                format!("header={name}:{expected}")
            }
            ApiAssertion::MetadataEquals { name, expected } => {
                format!("metadata={name}:{expected}")
            }
            ApiAssertion::BodyContains { expected } => format!("body={expected}"),
            ApiAssertion::JsonPathEquals { path, expected } => format!("json={path}:{expected}"),
            ApiAssertion::LatencyAtMostMillis { expected } => format!("latency={expected}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
