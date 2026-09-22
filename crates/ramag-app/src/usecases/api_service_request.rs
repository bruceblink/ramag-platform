//! API 请求在应用边界的变量展开。
//!
//! 这里复制请求配置并展开模板，不改变请求协议或正文模式；文件路径只作为受限配置传给
//! HTTP 驱动，驱动仍会重新校验文件状态和大小。

use std::collections::BTreeMap;

use ramag_domain::entities::{
    ApiAuth, ApiBody, ApiBodyMode, ApiMultipartPart, ApiMultipartValue, ApiOAuth2Config,
    ApiParameter, ApiProxyConfig, ApiRequestSpec, GrpcRequestSpec, HttpRequestSpec,
    MAX_API_GRPC_ENDPOINT_BYTES, MAX_API_GRPC_METHOD_BYTES, MAX_API_GRPC_SERVICE_BYTES,
    MAX_API_MULTIPART_FILE_NAME_BYTES, MAX_API_MULTIPART_PATH_BYTES, MAX_API_OAUTH2_SCOPE_BYTES,
    MAX_API_OAUTH2_URL_BYTES, MAX_API_PARAMETER_NAME_BYTES, MAX_API_PARAMETER_VALUE_BYTES,
    MAX_API_PROXY_CREDENTIAL_BYTES, MAX_API_PROXY_URL_BYTES, MAX_API_REQUEST_BODY_BYTES,
    MAX_API_URL_TEMPLATE_BYTES, resolve_template,
};
use ramag_domain::error::{DomainError, Result};

pub(super) fn resolve_request(
    request: &ApiRequestSpec,
    variables: &BTreeMap<String, String>,
) -> Result<ApiRequestSpec> {
    match request {
        ApiRequestSpec::Http(spec) => {
            Ok(ApiRequestSpec::Http(resolve_http_request(spec, variables)?))
        }
        ApiRequestSpec::Grpc(spec) => {
            Ok(ApiRequestSpec::Grpc(resolve_grpc_request(spec, variables)?))
        }
    }
}

fn expand(
    template: &str,
    variables: &BTreeMap<String, String>,
    label: &str,
    max_bytes: usize,
) -> Result<String> {
    resolve_template(template, variables, label, max_bytes).map_err(DomainError::InvalidConfig)
}

fn expand_parameter(
    parameter: &ApiParameter,
    variables: &BTreeMap<String, String>,
    label: &str,
) -> Result<ApiParameter> {
    Ok(ApiParameter::new(
        expand(
            &parameter.name,
            variables,
            label,
            MAX_API_PARAMETER_NAME_BYTES,
        )?,
        expand(
            &parameter.value,
            variables,
            label,
            MAX_API_PARAMETER_VALUE_BYTES,
        )?,
        parameter.sensitive,
    ))
}

/// 展开代理地址和认证字段；密码只在执行副本中保留，不进入响应历史或错误文本。
fn resolve_proxy_config(
    proxy: &ApiProxyConfig,
    variables: &BTreeMap<String, String>,
) -> Result<ApiProxyConfig> {
    let resolved = ApiProxyConfig {
        url: proxy
            .url
            .as_ref()
            .map(|url| expand(url, variables, "代理 URL", MAX_API_PROXY_URL_BYTES))
            .transpose()?,
        username: proxy
            .username
            .as_ref()
            .map(|username| {
                expand(
                    username,
                    variables,
                    "代理用户名",
                    MAX_API_PROXY_CREDENTIAL_BYTES,
                )
            })
            .transpose()?,
        password: proxy
            .password
            .as_ref()
            .map(|password| {
                expand(
                    password,
                    variables,
                    "代理密码",
                    MAX_API_PROXY_CREDENTIAL_BYTES,
                )
            })
            .transpose()?,
    };
    resolved
        .validate_resolved()
        .map_err(DomainError::InvalidConfig)?;
    Ok(resolved)
}

pub(super) fn resolve_http_request(
    spec: &HttpRequestSpec,
    variables: &BTreeMap<String, String>,
) -> Result<HttpRequestSpec> {
    let mut resolved = spec.clone();
    resolved.proxy = resolve_proxy_config(&spec.proxy, variables)?;
    resolved.url_template = expand(
        &spec.url_template,
        variables,
        "HTTP URL 模板",
        MAX_API_URL_TEMPLATE_BYTES,
    )?;
    resolved.query = spec
        .query
        .iter()
        .map(|parameter| expand_parameter(parameter, variables, "HTTP 查询参数"))
        .collect::<Result<Vec<_>>>()?;
    resolved.headers = spec
        .headers
        .iter()
        .map(|parameter| expand_parameter(parameter, variables, "HTTP Headers"))
        .collect::<Result<Vec<_>>>()?;
    resolved.auth = resolve_auth(&spec.auth, variables)?;
    resolved.body = match &spec.body {
        Some(body) => Some(match body.mode {
            ApiBodyMode::Text => ApiBody::text(
                expand(
                    &body.value,
                    variables,
                    "HTTP 请求正文",
                    MAX_API_REQUEST_BODY_BYTES,
                )?,
                body.content_type
                    .as_ref()
                    .map(|content_type| {
                        expand(
                            content_type,
                            variables,
                            "HTTP Content-Type",
                            MAX_API_PARAMETER_VALUE_BYTES,
                        )
                    })
                    .transpose()?,
            ),
            ApiBodyMode::Multipart => {
                let parts = body
                    .multipart
                    .iter()
                    .map(|part| {
                        let name = expand(
                            &part.name,
                            variables,
                            "Multipart 字段名称",
                            MAX_API_PARAMETER_NAME_BYTES,
                        )?;
                        let content_type = part
                            .content_type
                            .as_ref()
                            .map(|content_type| {
                                expand(
                                    content_type,
                                    variables,
                                    "Multipart Content-Type",
                                    MAX_API_PARAMETER_VALUE_BYTES,
                                )
                            })
                            .transpose()?;
                        let value = match &part.value {
                            ApiMultipartValue::Text { value } => ApiMultipartValue::Text {
                                value: expand(
                                    value,
                                    variables,
                                    "Multipart 文本字段值",
                                    MAX_API_PARAMETER_VALUE_BYTES,
                                )?,
                            },
                            ApiMultipartValue::File { path, file_name } => {
                                ApiMultipartValue::File {
                                    path: expand(
                                        path,
                                        variables,
                                        "Multipart 文件路径",
                                        MAX_API_MULTIPART_PATH_BYTES,
                                    )?,
                                    file_name: file_name
                                        .as_ref()
                                        .map(|file_name| {
                                            expand(
                                                file_name,
                                                variables,
                                                "Multipart 文件名",
                                                MAX_API_MULTIPART_FILE_NAME_BYTES,
                                            )
                                        })
                                        .transpose()?,
                                }
                            }
                        };
                        Ok(ApiMultipartPart {
                            name,
                            value,
                            content_type,
                            sensitive: part.sensitive,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                ApiBody::multipart(parts)
            }
        }),
        None => None,
    };
    resolved.validate().map_err(DomainError::InvalidConfig)?;
    Ok(resolved)
}

fn resolve_grpc_request(
    spec: &GrpcRequestSpec,
    variables: &BTreeMap<String, String>,
) -> Result<GrpcRequestSpec> {
    let mut resolved = spec.clone();
    resolved.proxy = resolve_proxy_config(&spec.proxy, variables)?;
    resolved.auth = resolve_auth(&spec.auth, variables)?;
    resolved.endpoint_template = expand(
        &spec.endpoint_template,
        variables,
        "gRPC Endpoint 模板",
        MAX_API_GRPC_ENDPOINT_BYTES,
    )?;
    resolved.service = expand(
        &spec.service,
        variables,
        "gRPC Service",
        MAX_API_GRPC_SERVICE_BYTES,
    )?;
    resolved.method = expand(
        &spec.method,
        variables,
        "gRPC Method",
        MAX_API_GRPC_METHOD_BYTES,
    )?;
    resolved.metadata = spec
        .metadata
        .iter()
        .map(|parameter| expand_parameter(parameter, variables, "gRPC Metadata"))
        .collect::<Result<Vec<_>>>()?;
    resolved.message = expand(
        &spec.message,
        variables,
        "gRPC 请求消息",
        MAX_API_REQUEST_BODY_BYTES,
    )?;
    resolved.validate().map_err(DomainError::InvalidConfig)?;
    Ok(resolved)
}

fn resolve_auth(auth: &ApiAuth, variables: &BTreeMap<String, String>) -> Result<ApiAuth> {
    match auth {
        ApiAuth::None => Ok(ApiAuth::None),
        ApiAuth::Basic { username, password } => Ok(ApiAuth::Basic {
            username: expand(
                username,
                variables,
                "Basic 用户名",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?,
            password: expand(
                password,
                variables,
                "Basic 密码",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?,
        }),
        ApiAuth::Bearer { token } => Ok(ApiAuth::Bearer {
            token: expand(
                token,
                variables,
                "Bearer Token",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?,
        }),
        ApiAuth::OAuth2 { config } => Ok(ApiAuth::OAuth2 {
            config: ApiOAuth2Config {
                token_url: expand(
                    &config.token_url,
                    variables,
                    "OAuth2 Token URL",
                    MAX_API_OAUTH2_URL_BYTES,
                )?,
                client_id: expand(
                    &config.client_id,
                    variables,
                    "OAuth2 Client ID",
                    MAX_API_PARAMETER_VALUE_BYTES,
                )?,
                client_secret: expand(
                    &config.client_secret,
                    variables,
                    "OAuth2 Client Secret",
                    MAX_API_PARAMETER_VALUE_BYTES,
                )?,
                scope: config
                    .scope
                    .as_ref()
                    .map(|scope| {
                        expand(scope, variables, "OAuth2 Scope", MAX_API_OAUTH2_SCOPE_BYTES)
                    })
                    .transpose()?,
            },
        }),
        ApiAuth::ApiKey {
            name,
            value,
            location,
        } => Ok(ApiAuth::ApiKey {
            name: expand(
                name,
                variables,
                "API Key 名称",
                MAX_API_PARAMETER_NAME_BYTES,
            )?,
            value: expand(
                value,
                variables,
                "API Key 值",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?,
            location: *location,
        }),
    }
}
