use std::fmt;

use serde::{Deserialize, Serialize};

use super::{
    ApiAuth, ApiBody, ApiGrpcDescriptor, ApiParameter, ApiProtocol, ApiTlsConfig,
    MAX_API_ASSERTION_VALUE_BYTES, MAX_API_ASSERTIONS, MAX_API_GRPC_ENDPOINT_BYTES,
    MAX_API_GRPC_METHOD_BYTES, MAX_API_GRPC_SERVICE_BYTES, MAX_API_HTTP_METHOD_BYTES,
    MAX_API_PARAMETER_NAME_BYTES, MAX_API_REQUEST_BODY_BYTES, MAX_API_REQUEST_NAME_BYTES,
    MAX_API_URL_TEMPLATE_BYTES, MAX_API_VARIABLE_NAME_BYTES, default_api_timeout,
    validate_parameters, validate_protocol_name, validate_required_text, validate_text,
    validate_timeout,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestSpec {
    pub method: String,
    pub url_template: String,
    #[serde(default)]
    pub query: Vec<ApiParameter>,
    #[serde(default)]
    pub headers: Vec<ApiParameter>,
    #[serde(default)]
    pub auth: ApiAuth,
    #[serde(default)]
    pub body: Option<ApiBody>,
    #[serde(default)]
    pub tls: ApiTlsConfig,
    #[serde(default = "default_api_timeout")]
    pub timeout_millis: u64,
}

impl HttpRequestSpec {
    /// 创建最小 HTTP 请求，后续可在 UI 中补充参数和正文。
    pub fn new(method: impl Into<String>, url_template: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            url_template: url_template.into(),
            query: Vec::new(),
            headers: Vec::new(),
            auth: ApiAuth::default(),
            body: None,
            tls: ApiTlsConfig::default(),
            timeout_millis: default_api_timeout(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_protocol_name("HTTP 方法", &self.method, MAX_API_HTTP_METHOD_BYTES)?;
        if self.method.bytes().any(|byte| byte.is_ascii_lowercase()) {
            return Err("HTTP 方法必须使用大写 ASCII 字符".into());
        }
        validate_required_text(
            "HTTP URL 模板",
            &self.url_template,
            MAX_API_URL_TEMPLATE_BYTES,
        )?;
        validate_parameters("HTTP 查询参数", &self.query, false)?;
        validate_parameters("HTTP Headers", &self.headers, true)?;
        self.auth.validate()?;
        if let Some(body) = &self.body {
            body.validate()?;
        }
        self.tls.validate()?;
        validate_timeout(self.timeout_millis)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrpcRequestSpec {
    pub endpoint_template: String,
    pub service: String,
    pub method: String,
    #[serde(default)]
    pub descriptor: ApiGrpcDescriptor,
    #[serde(default)]
    pub metadata: Vec<ApiParameter>,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub tls: ApiTlsConfig,
    #[serde(default = "default_api_timeout")]
    pub timeout_millis: u64,
}

impl fmt::Debug for GrpcRequestSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrpcRequestSpec")
            .field("endpoint_template", &self.endpoint_template)
            .field("service", &self.service)
            .field("method", &self.method)
            .field("descriptor", &self.descriptor)
            .field("metadata_count", &self.metadata.len())
            .field("message_bytes", &self.message.len())
            .field("tls", &self.tls)
            .field("timeout_millis", &self.timeout_millis)
            .finish()
    }
}

impl GrpcRequestSpec {
    /// 创建使用 Reflection 的最小 gRPC Unary 请求。
    pub fn new(
        endpoint_template: impl Into<String>,
        service: impl Into<String>,
        method: impl Into<String>,
    ) -> Self {
        Self {
            endpoint_template: endpoint_template.into(),
            service: service.into(),
            method: method.into(),
            descriptor: ApiGrpcDescriptor::default(),
            metadata: Vec::new(),
            message: "{}".into(),
            tls: ApiTlsConfig::default(),
            timeout_millis: default_api_timeout(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_text(
            "gRPC Endpoint 模板",
            &self.endpoint_template,
            MAX_API_GRPC_ENDPOINT_BYTES,
            false,
        )?;
        validate_protocol_name("gRPC Service", &self.service, MAX_API_GRPC_SERVICE_BYTES)?;
        validate_protocol_name("gRPC Method", &self.method, MAX_API_GRPC_METHOD_BYTES)?;
        if self.service.contains('/') || self.method.contains('/') {
            return Err("gRPC Service 和 Method 不能包含 '/'".into());
        }
        validate_parameters("gRPC Metadata", &self.metadata, true)?;
        validate_text(
            "gRPC 请求消息",
            &self.message,
            MAX_API_REQUEST_BODY_BYTES,
            false,
        )?;
        self.descriptor.validate()?;
        self.tls.validate()?;
        validate_timeout(self.timeout_millis)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiRequestSpec {
    Http(HttpRequestSpec),
    Grpc(GrpcRequestSpec),
}

impl fmt::Debug for ApiRequestSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(request) => formatter.debug_tuple("Http").field(request).finish(),
            Self::Grpc(request) => formatter.debug_tuple("Grpc").field(request).finish(),
        }
    }
}

impl ApiRequestSpec {
    pub fn protocol(&self) -> ApiProtocol {
        match self {
            Self::Http(_) => ApiProtocol::Http,
            Self::Grpc(_) => ApiProtocol::Grpc,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Http(request) => request.validate(),
            Self::Grpc(request) => request.validate(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiAssertion {
    HttpStatus { expected: u16 },
    HeaderEquals { name: String, expected: String },
    MetadataEquals { name: String, expected: String },
    BodyContains { expected: String },
    JsonPathEquals { path: String, expected: String },
    LatencyAtMostMillis { expected: u64 },
}

impl ApiAssertion {
    pub fn validate(&self, protocol: ApiProtocol) -> Result<(), String> {
        match self {
            Self::HttpStatus { expected } => {
                if protocol != ApiProtocol::Http {
                    return Err("HTTP 状态断言只能用于 HTTP 请求".into());
                }
                if *expected == 0 {
                    return Err("HTTP 状态码不能为 0".into());
                }
            }
            Self::HeaderEquals { name, expected } => {
                validate_protocol_name("Header 断言名称", name, MAX_API_PARAMETER_NAME_BYTES)?;
                validate_text(
                    "Header 断言值",
                    expected,
                    MAX_API_ASSERTION_VALUE_BYTES,
                    false,
                )?;
            }
            Self::MetadataEquals { name, expected } => {
                if protocol != ApiProtocol::Grpc {
                    return Err("gRPC Metadata 断言只能用于 gRPC 请求".into());
                }
                validate_protocol_name("Metadata 断言名称", name, MAX_API_PARAMETER_NAME_BYTES)?;
                validate_text(
                    "Metadata 断言值",
                    expected,
                    MAX_API_ASSERTION_VALUE_BYTES,
                    false,
                )?;
            }
            Self::BodyContains { expected } => {
                validate_text("正文断言值", expected, MAX_API_ASSERTION_VALUE_BYTES, false)?;
            }
            Self::JsonPathEquals { path, expected } => {
                validate_text("JSON Path", path, MAX_API_PARAMETER_NAME_BYTES, false)?;
                validate_text(
                    "JSON Path 断言值",
                    expected,
                    MAX_API_ASSERTION_VALUE_BYTES,
                    false,
                )?;
            }
            Self::LatencyAtMostMillis { expected } => validate_timeout(*expected)?,
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiVariableSource {
    JsonPath { path: String },
    Header { name: String },
    Metadata { name: String },
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiVariableExtraction {
    pub name: String,
    pub source: ApiVariableSource,
    #[serde(default)]
    pub sensitive: bool,
}

impl ApiVariableExtraction {
    pub fn validate(&self, protocol: ApiProtocol) -> Result<(), String> {
        validate_protocol_name("响应变量名称", &self.name, MAX_API_VARIABLE_NAME_BYTES)?;
        match &self.source {
            ApiVariableSource::JsonPath { path } => validate_text(
                "响应变量 JSON Path",
                path,
                MAX_API_ASSERTION_VALUE_BYTES,
                true,
            ),
            ApiVariableSource::Header { name } => {
                if protocol != ApiProtocol::Http {
                    return Err("HTTP Header 响应变量只能用于 HTTP 请求".into());
                }
                validate_protocol_name("响应 Header 名称", name, MAX_API_PARAMETER_NAME_BYTES)
            }
            ApiVariableSource::Metadata { name } => {
                if protocol != ApiProtocol::Grpc {
                    return Err("gRPC Metadata 响应变量只能用于 gRPC 请求".into());
                }
                validate_protocol_name("响应 Metadata 名称", name, MAX_API_PARAMETER_NAME_BYTES)
            }
            ApiVariableSource::Body => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiRequestRecord {
    pub id: super::ApiRequestId,
    pub name: String,
    #[serde(default)]
    pub protocol: ApiProtocol,
    pub request: ApiRequestSpec,
    #[serde(default)]
    pub assertions: Vec<ApiAssertion>,
    #[serde(default)]
    pub response_variables: Vec<ApiVariableExtraction>,
}

impl ApiRequestRecord {
    /// 用 HTTP 请求创建一条可保存的请求记录。
    pub fn new_http(name: impl Into<String>, request: HttpRequestSpec) -> Self {
        Self {
            id: super::ApiRequestId::new(),
            name: name.into(),
            protocol: ApiProtocol::Http,
            request: ApiRequestSpec::Http(request),
            assertions: Vec::new(),
            response_variables: Vec::new(),
        }
    }

    /// 用 gRPC 请求创建一条可保存的请求记录。
    pub fn new_grpc(name: impl Into<String>, request: GrpcRequestSpec) -> Self {
        Self {
            id: super::ApiRequestId::new(),
            name: name.into(),
            protocol: ApiProtocol::Grpc,
            request: ApiRequestSpec::Grpc(request),
            assertions: Vec::new(),
            response_variables: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_required_text("API 请求名称", &self.name, MAX_API_REQUEST_NAME_BYTES)?;
        if self.protocol != self.request.protocol() {
            return Err("API 请求协议标记与请求内容不一致".into());
        }
        self.request.validate()?;
        if self.assertions.len() > MAX_API_ASSERTIONS {
            return Err(format!("API 断言数量超过 {MAX_API_ASSERTIONS} 条上限"));
        }
        for assertion in &self.assertions {
            assertion.validate(self.protocol)?;
        }
        if self.response_variables.len() > super::MAX_API_RESPONSE_VARIABLES {
            return Err(format!(
                "响应变量数量超过 {} 条上限",
                super::MAX_API_RESPONSE_VARIABLES
            ));
        }
        let mut names = std::collections::HashSet::with_capacity(self.response_variables.len());
        for extraction in &self.response_variables {
            extraction.validate(self.protocol)?;
            if !names.insert(extraction.name.clone()) {
                return Err(format!("响应变量名称重复：{}", extraction.name));
            }
        }
        Ok(())
    }
}
