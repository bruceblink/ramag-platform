use std::fmt;

use serde::{Deserialize, Serialize};

use super::{
    ApiAuth, ApiBody, ApiBodyMode, ApiGrpcDescriptor, ApiParameter, ApiProtocol, ApiProxyConfig,
    ApiTlsConfig, MAX_API_ASSERTION_VALUE_BYTES, MAX_API_ASSERTIONS, MAX_API_GRPC_ENDPOINT_BYTES,
    MAX_API_GRPC_METHOD_BYTES, MAX_API_GRPC_SERVICE_BYTES, MAX_API_HTTP_METHOD_BYTES,
    MAX_API_PARAMETER_NAME_BYTES, MAX_API_REQUEST_BODY_BYTES, MAX_API_REQUEST_NAME_BYTES,
    MAX_API_URL_TEMPLATE_BYTES, MAX_API_VARIABLE_NAME_BYTES, default_api_timeout,
    validate_parameters, validate_protocol_name, validate_required_text, validate_text,
    validate_timeout,
};

/// 一次 HTTP 请求的完整领域描述。
///
/// 该结构只保存可持久化配置，不持有网络客户端或运行时连接。`url_template`、参数、认证、
/// TLS 和代理会在应用层按 Environment 展开为一次性执行副本；`validate` 在保存和发送前
/// 共同约束协议文本、正文大小、证书配置、代理边界与超时，保证基础设施驱动不需要猜测
/// 字段之间的依赖关系。响应历史不会从此结构反向生成，因此敏感请求配置不会自动进入响应正文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestSpec {
    /// 大写 ASCII HTTP 方法，例如 `GET` 或 `POST`。
    pub method: String,
    /// 可包含 `{{name}}` Environment 占位符的目标 URL；展开后只允许 HTTP/HTTPS。
    pub url_template: String,
    /// 追加到 URL 的查询参数；参数值在执行副本中展开并按 URL 规则编码。
    #[serde(default)]
    pub query: Vec<ApiParameter>,
    /// 请求头参数；敏感标记控制历史和调试展示，不改变实际发送值。
    #[serde(default)]
    pub headers: Vec<ApiParameter>,
    /// HTTP 认证配置；驱动只在内存请求中生成 Authorization 或查询参数。
    #[serde(default)]
    pub auth: ApiAuth,
    /// 可选文本或 Multipart 正文；Multipart 的 boundary 由驱动生成而非由调用者拼接。
    #[serde(default)]
    pub body: Option<ApiBody>,
    /// 服务端证书验证和客户端证书身份配置。
    #[serde(default)]
    pub tls: ApiTlsConfig,
    /// 显式 HTTP 代理；HTTPS/gRPC 使用 CONNECT，明文 HTTP 使用标准代理转发。
    #[serde(default)]
    pub proxy: ApiProxyConfig,
    /// 单次请求的连接、响应和流读取共享超时上限，单位为毫秒。
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
            proxy: ApiProxyConfig::default(),
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
            if body.mode == ApiBodyMode::Multipart
                && self
                    .headers
                    .iter()
                    .any(|header| header.name.eq_ignore_ascii_case("content-type"))
            {
                return Err(
                    "Multipart 请求不能手动设置 Content-Type；驱动需要生成 boundary".into(),
                );
            }
        }
        self.tls.validate()?;
        self.proxy.validate()?;
        validate_timeout(self.timeout_millis)
    }
}

/// 使用 Server Reflection 或本地 Descriptor 读取 gRPC Service/Method 目录的配置。
///
/// 发现操作不写入请求历史，但复用与 gRPC 调用相同的 TLS、代理、变量展开和超时规则；
/// 这样 UI 中“发现服务”和“发送请求”不会因为传输路径不同而产生安全配置漂移。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiGrpcDiscoverySpec {
    /// gRPC Endpoint 模板，展开后必须是带 `http` 或 `https` scheme 的 authority URL。
    pub endpoint_template: String,
    /// Descriptor 来源；Reflection 需要建立网络连接，FileDescriptorSet 可离线解析。
    #[serde(default)]
    pub descriptor: ApiGrpcDescriptor,
    /// 服务端证书验证和客户端证书身份配置。
    #[serde(default)]
    pub tls: ApiTlsConfig,
    /// Reflection 连接使用的显式 HTTP 代理，并通过 CONNECT 建立 gRPC 隧道。
    #[serde(default)]
    pub proxy: ApiProxyConfig,
    /// Reflection 建立连接和读取 Descriptor 的总超时，单位为毫秒。
    #[serde(default = "default_api_timeout")]
    pub timeout_millis: u64,
}

impl ApiGrpcDiscoverySpec {
    /// 创建使用 Server Reflection 的 gRPC Service 发现请求。
    pub fn new(endpoint_template: impl Into<String>) -> Self {
        Self {
            endpoint_template: endpoint_template.into(),
            descriptor: ApiGrpcDescriptor::default(),
            tls: ApiTlsConfig::default(),
            proxy: ApiProxyConfig::default(),
            timeout_millis: default_api_timeout(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_required_text(
            "gRPC Endpoint 模板",
            &self.endpoint_template,
            MAX_API_GRPC_ENDPOINT_BYTES,
        )?;
        self.descriptor.validate()?;
        self.tls.validate()?;
        self.proxy.validate()?;
        validate_timeout(self.timeout_millis)
    }
}

/// 一次动态 gRPC 调用的可持久化描述。
///
/// `descriptor` 决定消息类型的来源，`metadata` 和 `message` 经过同一套 Environment 展开；
/// 执行时驱动依据 Method Descriptor 选择 Unary、Server Streaming、Client Streaming 或
/// 双向 Streaming，并把所有响应限制在领域层规定的大小内。该结构不保存 Channel、任务句柄
/// 或证书内容，只保存路径和模板，便于取消、重试和跨进程持久化。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrpcRequestSpec {
    /// gRPC Endpoint 模板，展开后用于 HTTP/2 建连和 TLS Server Name。
    pub endpoint_template: String,
    /// 完整 Protobuf Service 名称，不包含前导或中间斜杠。
    pub service: String,
    /// Service 内的方法名，不包含斜杠。
    pub method: String,
    /// 消息 Descriptor 来源；Reflection 会先查询目标服务，文件集合则完全离线。
    #[serde(default)]
    pub descriptor: ApiGrpcDescriptor,
    /// gRPC Metadata 参数；敏感项只影响展示和历史记录，发送时仍按原值编码。
    #[serde(default)]
    pub metadata: Vec<ApiParameter>,
    /// 一个或多行 JSON 消息；流式方法按行解析为有界消息序列。
    #[serde(default)]
    pub message: String,
    /// 服务端证书验证和客户端证书身份配置。
    #[serde(default)]
    pub tls: ApiTlsConfig,
    /// 动态调用和 Reflection 共用的显式 HTTP 代理，并通过 CONNECT 建立 gRPC 隧道。
    #[serde(default)]
    pub proxy: ApiProxyConfig,
    /// 单次调用的连接、RPC deadline 和流读取共享超时，单位为毫秒。
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
            .field("proxy", &self.proxy)
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
            proxy: ApiProxyConfig::default(),
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
        self.proxy.validate()?;
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
