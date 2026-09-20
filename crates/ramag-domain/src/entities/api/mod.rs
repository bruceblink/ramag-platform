//! API 测试工具共享领域模型。
//!
//! 该模块只描述请求、环境和响应的数据约定，不创建 HTTP/gRPC 客户端。
//! 敏感字段在模型中仍以明文供执行驱动使用，但持久化由 Storage 统一加密，
//! Debug 输出只显示脱敏摘要。

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

pub const MAX_API_WORKSPACES: usize = 32;
pub const MAX_API_WORKSPACE_NAME_BYTES: usize = 256;
pub const MAX_API_COLLECTIONS: usize = 128;
pub const MAX_API_COLLECTION_NAME_BYTES: usize = 256;
pub const MAX_API_REQUESTS: usize = 2048;
pub const MAX_API_REQUEST_NAME_BYTES: usize = 256;
pub const MAX_API_ENVIRONMENTS: usize = 64;
pub const MAX_API_ENVIRONMENT_NAME_BYTES: usize = 256;
pub const MAX_API_ENVIRONMENT_VARIABLES: usize = 256;
pub const MAX_API_VARIABLE_NAME_BYTES: usize = 256;
pub const MAX_API_PARAMETER_COUNT: usize = 128;
pub const MAX_API_PARAMETER_NAME_BYTES: usize = 1024;
pub const MAX_API_PARAMETER_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_API_HTTP_METHOD_BYTES: usize = 32;
pub const MAX_API_URL_TEMPLATE_BYTES: usize = 8 * 1024;
pub const MAX_API_GRPC_ENDPOINT_BYTES: usize = 8 * 1024;
pub const MAX_API_GRPC_SERVICE_BYTES: usize = 512;
pub const MAX_API_GRPC_METHOD_BYTES: usize = 512;
pub const MAX_API_GRPC_STREAM_MESSAGES: usize = 1024;
pub const MAX_API_REQUEST_BODY_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_API_RESPONSE_BODY_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_API_DESCRIPTOR_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_API_PROTO_SOURCE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_API_PROTO_SOURCE_TOTAL_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_API_ASSERTIONS: usize = 64;
pub const MAX_API_RESPONSE_VARIABLES: usize = 32;
pub const MAX_API_ASSERTION_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_API_ERROR_BYTES: usize = 64 * 1024;
pub const MAX_API_HISTORY_BODY_BYTES: usize = 16 * 1024;
pub const MAX_API_TIMEOUT_MILLIS: u64 = 5 * 60 * 1000;
pub const MAX_API_TLS_PATH_BYTES: usize = 32 * 1024;
pub const MAX_API_PROXY_URL_BYTES: usize = 8 * 1024;
pub const MAX_API_PROXY_CREDENTIAL_BYTES: usize = 8 * 1024;
pub const MAX_API_MULTIPART_PARTS: usize = 64;
pub const MAX_API_MULTIPART_PATH_BYTES: usize = 32 * 1024;
pub const MAX_API_MULTIPART_FILE_NAME_BYTES: usize = 1024;
pub const MAX_API_MULTIPART_FILE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_API_MULTIPART_TOTAL_BYTES: usize = 32 * 1024 * 1024;

macro_rules! api_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}", self.0)
            }
        }
    };
}

api_id!(ApiWorkspaceId);
api_id!(ApiCollectionId);
api_id!(ApiRequestId);
api_id!(ApiEnvironmentId);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiProtocol {
    #[default]
    Http,
    Grpc,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiParameter {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub sensitive: bool,
}

impl fmt::Debug for ApiParameter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiParameter")
            .field("name", &self.name)
            .field(
                "value",
                if self.sensitive {
                    &"[REDACTED]"
                } else {
                    &self.value
                },
            )
            .field("sensitive", &self.sensitive)
            .finish()
    }
}

impl ApiParameter {
    /// 创建一个请求参数；`sensitive` 决定 Debug 输出是否隐藏值。
    pub fn new(name: impl Into<String>, value: impl Into<String>, sensitive: bool) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            sensitive,
        }
    }

    pub(super) fn validate(&self, label: &str) -> Result<(), String> {
        validate_protocol_name(label, &self.name, MAX_API_PARAMETER_NAME_BYTES)?;
        validate_text(
            &format!("{label}值"),
            &self.value,
            MAX_API_PARAMETER_VALUE_BYTES,
            false,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ApiKeyLocation {
    #[default]
    Header,
    Query,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ApiAuth {
    #[default]
    None,
    Basic {
        username: String,
        password: String,
    },
    Bearer {
        token: String,
    },
    ApiKey {
        name: String,
        value: String,
        #[serde(default)]
        location: ApiKeyLocation,
    },
}

impl fmt::Debug for ApiAuth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => formatter.write_str("None"),
            Self::Basic { username, .. } => formatter
                .debug_struct("Basic")
                .field("username", username)
                .field("password", &"[REDACTED]")
                .finish(),
            Self::Bearer { .. } => formatter
                .debug_struct("Bearer")
                .field("token", &"[REDACTED]")
                .finish(),
            Self::ApiKey { name, location, .. } => formatter
                .debug_struct("ApiKey")
                .field("name", name)
                .field("value", &"[REDACTED]")
                .field("location", location)
                .finish(),
        }
    }
}

impl ApiAuth {
    pub(super) fn validate(&self) -> Result<(), String> {
        match self {
            Self::None => Ok(()),
            Self::Basic { username, password } => {
                validate_text(
                    "Basic 用户名",
                    username,
                    MAX_API_PARAMETER_VALUE_BYTES,
                    false,
                )?;
                validate_text("Basic 密码", password, MAX_API_PARAMETER_VALUE_BYTES, false)
            }
            Self::Bearer { token } => {
                validate_text("Bearer Token", token, MAX_API_PARAMETER_VALUE_BYTES, false)
            }
            Self::ApiKey { name, value, .. } => {
                validate_protocol_name("API Key 名称", name, MAX_API_PARAMETER_NAME_BYTES)?;
                validate_text("API Key 值", value, MAX_API_PARAMETER_VALUE_BYTES, false)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ApiTlsVerify {
    None,
    Ca,
    #[default]
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ApiTlsConfig {
    #[serde(default)]
    pub verify: ApiTlsVerify,
    #[serde(default)]
    pub ca_cert_path: Option<String>,
    #[serde(default)]
    pub client_cert_path: Option<String>,
    #[serde(default)]
    pub client_key_path: Option<String>,
}

impl ApiTlsConfig {
    pub(super) fn validate(&self) -> Result<(), String> {
        for (label, path) in [
            ("CA 证书路径", self.ca_cert_path.as_deref()),
            ("客户端证书路径", self.client_cert_path.as_deref()),
            ("客户端密钥路径", self.client_key_path.as_deref()),
        ] {
            if let Some(path) = path {
                validate_text(label, path, MAX_API_TLS_PATH_BYTES, true)?;
            }
        }
        if self.client_cert_path.is_some() != self.client_key_path.is_some() {
            return Err("客户端证书和客户端密钥必须同时配置".into());
        }
        if matches!(self.verify, ApiTlsVerify::Ca) && self.ca_cert_path.is_none() {
            return Err("TLS verify=ca 必须配置 CA 证书路径".into());
        }
        if (self.client_cert_path.is_some() || self.client_key_path.is_some())
            && matches!(self.verify, ApiTlsVerify::None)
        {
            return Err("客户端证书认证不能关闭服务端证书校验".into());
        }
        Ok(())
    }
}

/// API 工作台持久化的显式 HTTP 代理配置。
///
/// `url` 只描述代理节点，不允许在其中嵌入认证信息；认证字段独立保存，避免 URL、日志和
/// 导入导出文本意外泄露密码。HTTPS 和 gRPC 隧道使用 HTTP CONNECT，明文 HTTP 由客户端
/// 使用标准代理请求格式转发。字段允许使用 `{{environment_variable}}` 模板，保存时执行
/// 结构与长度检查，发送前由应用层或传输层展开并调用 [`Self::validate_resolved`] 进行完整
/// 的协议、主机和路径校验。未配置 `url` 时请求保持直连，不读取系统代理或 PAC 配置。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ApiProxyConfig {
    /// 代理节点 URL；展开后只接受 `http://host[:port]`，不包含路径、查询或片段。
    #[serde(default)]
    pub url: Option<String>,
    /// 可选 Basic 认证用户名；与密码必须同时出现，允许通过环境变量模板提供。
    #[serde(default)]
    pub username: Option<String>,
    /// 可选 Basic 认证密码；Debug 输出只保留是否配置的状态，绝不显示实际值。
    #[serde(default)]
    pub password: Option<String>,
}

impl fmt::Debug for ApiProxyConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiProxyConfig")
            .field("url", &self.url)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl ApiProxyConfig {
    pub(super) fn validate(&self) -> Result<(), String> {
        let credentials_present = self.username.is_some() || self.password.is_some();
        let Some(url) = self.url.as_deref() else {
            if credentials_present {
                return Err("代理用户名和密码必须在配置代理地址后使用".into());
            }
            return Ok(());
        };
        validate_required_text("代理 URL", url, MAX_API_PROXY_URL_BYTES)?;
        if self.username.is_some() != self.password.is_some() {
            return Err("代理用户名和密码必须同时配置".into());
        }
        if let Some(username) = &self.username {
            validate_text("代理用户名", username, MAX_API_PROXY_CREDENTIAL_BYTES, true)?;
        }
        if let Some(password) = &self.password {
            validate_text("代理密码", password, MAX_API_PROXY_CREDENTIAL_BYTES, true)?;
        }
        if !url.contains("{{") {
            validate_proxy_url(url)?;
        }
        Ok(())
    }

    /// 校验变量展开后的代理配置，拒绝被环境变量替换出的不安全地址或内嵌凭据。
    ///
    /// 调用方必须在建立 TCP 连接或构造 HTTP 客户端前调用它；这样即使模板变量来自用户
    /// 编辑的 Environment，也不会绕过 `http` 协议、无路径 URL 和分离认证信息的限制。
    pub fn validate_resolved(&self) -> Result<(), String> {
        self.validate()?;
        if let Some(url) = self.url.as_deref() {
            validate_proxy_url(url)?;
        }
        Ok(())
    }

    pub fn is_enabled(&self) -> bool {
        self.url.is_some()
    }
}

/// 解析已经展开的代理地址；模板文本由上层先替换，再在此处执行完整 URL 限制。
///
/// 该函数不返回解析后的 `Url`，以免持久化层意外保留带认证信息的派生对象；它只负责
/// 验证，实际传输层在构造连接时重新解析同一受限字符串。
fn validate_proxy_url(url: &str) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|_| "代理 URL 无效".to_string())?;
    if parsed.scheme() != "http" {
        return Err("代理 URL 只支持 http scheme".into());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("代理认证信息必须单独配置，不能写入代理 URL".into());
    }
    if parsed.host_str().is_none() || parsed.port_or_known_default().is_none() {
        return Err("代理 URL 必须包含有效主机和端口".into());
    }
    if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("代理 URL 不能包含路径、查询参数或片段".into());
    }
    Ok(())
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiGrpcDescriptor {
    #[default]
    Reflection,
    FileDescriptorSet {
        bytes: Vec<u8>,
    },
}

impl fmt::Debug for ApiGrpcDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reflection => formatter.write_str("Reflection"),
            Self::FileDescriptorSet { bytes } => formatter
                .debug_struct("FileDescriptorSet")
                .field("bytes", &bytes.len())
                .finish(),
        }
    }
}

impl ApiGrpcDescriptor {
    pub fn validate(&self) -> Result<(), String> {
        if let Self::FileDescriptorSet { bytes } = self {
            if bytes.is_empty() {
                return Err("gRPC FileDescriptorSet 不能为空".into());
            }
            if bytes.len() > MAX_API_DESCRIPTOR_BYTES {
                return Err(format!(
                    "gRPC FileDescriptorSet 超过 {MAX_API_DESCRIPTOR_BYTES} bytes 上限"
                ));
            }
        }
        Ok(())
    }
}

fn default_api_timeout() -> u64 {
    30_000
}

fn validate_timeout(timeout_millis: u64) -> Result<(), String> {
    if timeout_millis == 0 || timeout_millis > MAX_API_TIMEOUT_MILLIS {
        return Err(format!("API 超时必须为 1 - {MAX_API_TIMEOUT_MILLIS} 毫秒"));
    }
    Ok(())
}

fn validate_required_text(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label}不能为空"));
    }
    validate_text(label, value, max_bytes, true)
}

fn validate_protocol_name(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    validate_required_text(label, value, max_bytes)?;
    if value
        .chars()
        .any(|character| character.is_whitespace() || character.is_control() || character == ':')
    {
        return Err(format!("{label}包含空白、控制字符或 ':'"));
    }
    Ok(())
}

fn validate_text(
    label: &str,
    value: &str,
    max_bytes: usize,
    single_line: bool,
) -> Result<(), String> {
    if value.len() > max_bytes {
        return Err(format!("{label}超过 {max_bytes} bytes 上限"));
    }
    if single_line && value.contains(['\r', '\n']) {
        return Err(format!("{label}不能包含换行"));
    }
    if value.chars().any(|character| {
        character.is_control() && (single_line || !matches!(character, '\r' | '\n'))
    }) {
        return Err(format!("{label}包含不允许的控制字符"));
    }
    Ok(())
}

fn validate_parameters(
    label: &str,
    parameters: &[ApiParameter],
    reject_case_insensitive_duplicates: bool,
) -> Result<(), String> {
    if parameters.len() > MAX_API_PARAMETER_COUNT {
        return Err(format!("{label}数量超过 {MAX_API_PARAMETER_COUNT} 个上限"));
    }
    let mut names = HashSet::with_capacity(parameters.len());
    for parameter in parameters {
        parameter.validate(label)?;
        if reject_case_insensitive_duplicates && !names.insert(parameter.name.to_ascii_lowercase())
        {
            return Err(format!("{label}名称重复：{}", parameter.name));
        }
    }
    Ok(())
}

/// 把驱动收到的响应正文裁剪到领域层上限，并返回原始大小和是否发生裁剪。
pub fn bound_response_body(body: Vec<u8>) -> (Vec<u8>, u64, bool) {
    let original_size = body.len() as u64;
    if body.len() <= MAX_API_RESPONSE_BODY_BYTES {
        return (body, original_size, false);
    }
    (
        body[..MAX_API_RESPONSE_BODY_BYTES].to_vec(),
        original_size,
        true,
    )
}

/// API 驱动收到的取消标记；驱动应在网络等待前后检查该标记并尽快结束。
pub type ApiCancellation = Arc<AtomicBool>;

mod body;
mod execution;
mod grpc;
mod import;
mod requests;
mod response;
mod workspace;

pub use body::{ApiBody, ApiBodyMode, ApiMultipartPart, ApiMultipartValue};
pub use execution::{
    ApiAssertionResult, ApiCollectionRunResult, ApiExecutionOutcome, ApiExecutionResult,
    ApiExtractedVariable, ApiHistoryRecord, MAX_API_HISTORY, MAX_API_HISTORY_LIST_BYTES,
    evaluate_assertions, extract_response_variables, resolve_template,
};
pub use grpc::{ApiGrpcMethodSummary, ApiGrpcServiceSummary};
pub use import::{
    ApiImportBundle, ApiImportFormat, ApiImportSummary, MAX_API_IMPORT_BYTES, MAX_API_IMPORT_DEPTH,
    import_api_json,
};
pub use requests::{
    ApiAssertion, ApiGrpcDiscoverySpec, ApiRequestRecord, ApiRequestSpec, ApiVariableExtraction,
    ApiVariableSource, GrpcRequestSpec, HttpRequestSpec,
};
pub use response::{ApiResponseSnapshot, ApiResponseSnapshotParts, ApiResponseStatus};
pub use workspace::{ApiCollection, ApiEnvironment, ApiWorkspace};

#[cfg(test)]
mod tests;
