use super::*;
use ramag_domain::entities::{
    ApiAssertion, ApiBodyMode, ApiEnvironment, ApiExtractedVariable, ApiProxyConfig,
    ApiRequestRecord, ApiRequestSpec, ApiTlsConfig, ApiTlsVerify, ApiVariableExtraction,
    ApiVariableSource, ApiWorkspace,
};

#[path = "context_format.rs"]
mod format_helpers;

use format_helpers::{
    format_assertions, format_parameters, format_query_parameters, format_response_variables,
};

/// 从可见环境输入和运行时敏感值构造执行环境；敏感值不会回填到编辑器。
pub(crate) fn environment_from_view(view: &ApiView, cx: &App) -> Result<ApiEnvironment> {
    let mut environment = parse_environment_values(&input_value(&view.environment_variables, cx))?;
    apply_sensitive_references(
        &mut environment,
        &input_value(&view.environment_sensitive, cx),
        Some(&view.runtime_environment),
    )?;
    Ok(environment)
}

#[cfg(test)]
pub(crate) fn parse_environment(variables: &str, sensitive: &str) -> Result<ApiEnvironment> {
    let mut environment = parse_environment_values(variables)?;
    apply_sensitive_references(&mut environment, sensitive, None)?;
    Ok(environment)
}

/// 解析非敏感环境变量；调用方负责根据敏感变量引用补齐隐藏值。
fn parse_environment_values(variables: &str) -> Result<ApiEnvironment> {
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
    environment.validate().map_err(DomainError::InvalidConfig)?;
    Ok(environment)
}

/// 将敏感变量引用加入执行环境，必要时从运行时副本恢复其值。
fn apply_sensitive_references(
    environment: &mut ApiEnvironment,
    sensitive: &str,
    runtime_environment: Option<&ApiEnvironment>,
) -> Result<()> {
    for (index, line) in sensitive.lines().enumerate() {
        let name = line.trim();
        if name.is_empty() {
            continue;
        }
        if !environment.variables.contains_key(name) {
            let Some(runtime_environment) = runtime_environment else {
                return Err(DomainError::InvalidConfig(format!(
                    "敏感变量第 {} 行未找到变量：{name}",
                    index + 1
                )));
            };
            let Some(value) = runtime_environment.variables.get(name) else {
                return Err(DomainError::InvalidConfig(format!(
                    "敏感变量第 {} 行未找到变量：{name}",
                    index + 1
                )));
            };
            environment
                .variables
                .insert(name.to_string(), value.clone());
        }
        environment.sensitive_variable_refs.push(name.to_string());
    }
    environment.validate().map_err(DomainError::InvalidConfig)?;
    Ok(())
}

pub(crate) fn assertions_from_view(view: &ApiView, cx: &App) -> Result<Vec<ApiAssertion>> {
    parse_assertions(&input_value(&view.assertions, cx), view.protocol)
}

/// 解析共享 TLS 编辑器；mTLS 必须同时保留服务端证书校验和客户端身份材料。
pub(crate) fn tls_from_view(view: &ApiView, cx: &App) -> Result<ApiTlsConfig> {
    let verify = match input_value(&view.tls_verify, cx)
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "full" => ApiTlsVerify::Full,
        "ca" => ApiTlsVerify::Ca,
        "none" => ApiTlsVerify::None,
        value => {
            return Err(DomainError::InvalidConfig(format!(
                "TLS 校验模式不支持：{value}，可选 full、ca 或 none"
            )));
        }
    };
    Ok(ApiTlsConfig {
        verify,
        ca_cert_path: optional_input_value(&view.tls_ca_cert_path, cx),
        client_cert_path: optional_input_value(&view.tls_client_cert_path, cx),
        client_key_path: optional_input_value(&view.tls_client_key_path, cx),
    })
}

/// 解析共享显式代理编辑器；密码只随加密工作区或执行副本流转。
pub(crate) fn proxy_from_view(view: &ApiView, cx: &App) -> Result<ApiProxyConfig> {
    Ok(ApiProxyConfig {
        url: optional_input_value(&view.proxy_url, cx),
        username: optional_input_value(&view.proxy_username, cx),
        password: optional_input_value(&view.proxy_password, cx),
    })
}

pub(crate) fn response_variables_from_view(
    view: &ApiView,
    cx: &App,
) -> Result<Vec<ApiVariableExtraction>> {
    parse_response_variables(&input_value(&view.response_variables, cx), view.protocol)
}

pub(crate) fn apply_extracted_variables_to_view(
    view: &mut ApiView,
    extracted: &[ApiExtractedVariable],
    window: &mut Window,
    cx: &mut Context<ApiView>,
) -> Result<()> {
    if extracted.is_empty() {
        return Ok(());
    }
    let mut environment = environment_from_view(view, cx)?;
    environment
        .apply_extracted_variables(extracted)
        .map_err(DomainError::InvalidConfig)?;
    view.runtime_environment = environment.clone();
    set_input(
        &view.environment_variables,
        environment
            .variables
            .iter()
            .filter(|(name, _)| {
                !environment
                    .sensitive_variable_refs
                    .iter()
                    .any(|sensitive_name| sensitive_name == *name)
            })
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
    Ok(())
}

pub(crate) fn parse_response_variables(
    value: &str,
    protocol: ApiProtocol,
) -> Result<Vec<ApiVariableExtraction>> {
    let mut extractions = Vec::new();
    for (index, line) in value.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((target, expression)) = line.split_once('=') else {
            return Err(DomainError::InvalidConfig(format!(
                "响应变量第 {} 行必须使用 name=source 格式",
                index + 1
            )));
        };
        let mut target = target.trim();
        let sensitive = if let Some(rest) = target.strip_prefix("secret ") {
            target = rest.trim();
            true
        } else {
            false
        };
        let expression = expression.trim();
        let source = match expression.split_once(':') {
            Some((kind, value)) => match kind.trim().to_ascii_lowercase().as_str() {
                "json" => ApiVariableSource::JsonPath {
                    path: value.trim().to_string(),
                },
                "header" => ApiVariableSource::Header {
                    name: value.trim().to_string(),
                },
                "metadata" => ApiVariableSource::Metadata {
                    name: value.trim().to_string(),
                },
                other => {
                    return Err(DomainError::InvalidConfig(format!(
                        "响应变量第 {} 行来源类型不支持：{other}",
                        index + 1
                    )));
                }
            },
            None if expression.eq_ignore_ascii_case("body") => ApiVariableSource::Body,
            None => {
                return Err(DomainError::InvalidConfig(format!(
                    "响应变量第 {} 行必须使用 json:path、header:name、metadata:name 或 body",
                    index + 1
                )));
            }
        };
        let extraction = ApiVariableExtraction {
            name: target.to_string(),
            source,
            sensitive,
        };
        extraction
            .validate(protocol)
            .map_err(DomainError::InvalidConfig)?;
        extractions.push(extraction);
    }
    if extractions.len() > ramag_domain::entities::MAX_API_RESPONSE_VARIABLES {
        return Err(DomainError::InvalidConfig(format!(
            "响应变量数量超过 {} 条上限",
            ramag_domain::entities::MAX_API_RESPONSE_VARIABLES
        )));
    }
    Ok(extractions)
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
    let response_variables = response_variables_from_view(view, cx)?;
    let mut record = match request {
        ApiRequestSpec::Http(spec) => ApiRequestRecord::new_http(name, spec),
        ApiRequestSpec::Grpc(spec) => ApiRequestRecord::new_grpc(name, spec),
    };
    record.assertions = assertions;
    record.response_variables = response_variables;
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
    view.runtime_environment = ApiEnvironment::new("local");
    if let Some(environment) = workspace.default_environment_id.as_ref().and_then(|id| {
        workspace
            .environments
            .iter()
            .find(|environment| &environment.id == id)
    }) {
        view.runtime_environment = environment.clone();
        set_input(
            &view.environment_variables,
            environment
                .variables
                .iter()
                .filter(|(name, _)| {
                    !environment
                        .sensitive_variable_refs
                        .iter()
                        .any(|sensitive_name| sensitive_name == *name)
                })
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
    set_input(
        &view.response_variables,
        format_response_variables(&request.response_variables),
        window,
        cx,
    );
    match &request.request {
        ApiRequestSpec::Http(spec) => {
            view.protocol = ApiProtocol::Http;
            view.auth_editor.set_auth(&spec.auth, window, cx);
            set_tls_inputs(view, &spec.tls, window, cx);
            set_proxy_inputs(view, &spec.proxy, window, cx);
            view.http_body_mode = spec
                .body
                .as_ref()
                .map(|body| body.mode)
                .unwrap_or(ApiBodyMode::Text);
            view.http_body_content_type = match spec.body.as_ref().map(|body| body.mode) {
                Some(ApiBodyMode::Multipart) => String::new(),
                _ => spec
                    .body
                    .as_ref()
                    .and_then(|body| body.content_type.clone())
                    .unwrap_or_else(|| "application/json".into()),
            };
            set_input(&view.http_method, spec.method.clone(), window, cx);
            set_input(&view.http_url, spec.url_template.clone(), window, cx);
            set_input(
                &view.http_query,
                format_query_parameters(&spec.query),
                window,
                cx,
            );
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
                    .map(|body| match body.mode {
                        ApiBodyMode::Text => body.value.clone(),
                        ApiBodyMode::Multipart => super::format_multipart_body(&body.multipart),
                    })
                    .unwrap_or_default(),
                window,
                cx,
            );
        }
        ApiRequestSpec::Grpc(spec) => {
            view.protocol = ApiProtocol::Grpc;
            view.grpc_descriptor = spec.descriptor.clone();
            view.auth_editor.set_auth(&spec.auth, window, cx);
            set_tls_inputs(view, &spec.tls, window, cx);
            set_proxy_inputs(view, &spec.proxy, window, cx);
            view.http_body_mode = ApiBodyMode::Text;
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

fn optional_input_value(field: &Entity<InputState>, cx: &App) -> Option<String> {
    let value = input_value(field, cx);
    (!value.trim().is_empty()).then(|| value.trim().to_string())
}

fn set_tls_inputs(
    view: &ApiView,
    tls: &ApiTlsConfig,
    window: &mut Window,
    cx: &mut Context<ApiView>,
) {
    set_input(
        &view.tls_verify,
        match tls.verify {
            ApiTlsVerify::Full => "full",
            ApiTlsVerify::Ca => "ca",
            ApiTlsVerify::None => "none",
        }
        .into(),
        window,
        cx,
    );
    set_input(
        &view.tls_ca_cert_path,
        tls.ca_cert_path.clone().unwrap_or_default(),
        window,
        cx,
    );
    set_input(
        &view.tls_client_cert_path,
        tls.client_cert_path.clone().unwrap_or_default(),
        window,
        cx,
    );
    set_input(
        &view.tls_client_key_path,
        tls.client_key_path.clone().unwrap_or_default(),
        window,
        cx,
    );
}

fn set_proxy_inputs(
    view: &ApiView,
    proxy: &ApiProxyConfig,
    window: &mut Window,
    cx: &mut Context<ApiView>,
) {
    set_input(
        &view.proxy_url,
        proxy.url.clone().unwrap_or_default(),
        window,
        cx,
    );
    set_input(
        &view.proxy_username,
        proxy.username.clone().unwrap_or_default(),
        window,
        cx,
    );
    set_input(
        &view.proxy_password,
        proxy.password.clone().unwrap_or_default(),
        window,
        cx,
    );
}

fn set_input(
    field: &Entity<InputState>,
    value: String,
    window: &mut Window,
    cx: &mut Context<ApiView>,
) {
    field.update(cx, |input, cx| input.set_value(value, window, cx));
}
