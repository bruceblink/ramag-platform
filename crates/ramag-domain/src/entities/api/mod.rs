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
pub const MAX_API_REQUEST_BODY_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_API_RESPONSE_BODY_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_API_DESCRIPTOR_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_API_ASSERTIONS: usize = 64;
pub const MAX_API_ASSERTION_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_API_ERROR_BYTES: usize = 64 * 1024;
pub const MAX_API_TIMEOUT_MILLIS: u64 = 5 * 60 * 1000;
pub const MAX_API_TLS_PATH_BYTES: usize = 32 * 1024;

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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiBody {
    #[serde(default)]
    pub content_type: Option<String>,
    pub value: String,
}

impl fmt::Debug for ApiBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiBody")
            .field("content_type", &self.content_type)
            .field("value_bytes", &self.value.len())
            .field("value", &"[REDACTED]")
            .finish()
    }
}

impl ApiBody {
    pub fn text(value: impl Into<String>, content_type: Option<String>) -> Self {
        Self {
            content_type,
            value: value.into(),
        }
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        if let Some(content_type) = &self.content_type {
            validate_text(
                "请求 Content-Type",
                content_type,
                MAX_API_PARAMETER_NAME_BYTES,
                true,
            )?;
        }
        validate_text("请求正文", &self.value, MAX_API_REQUEST_BODY_BYTES, false)
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
        Ok(())
    }
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
    pub(super) fn validate(&self) -> Result<(), String> {
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
    if value.chars().any(char::is_control) && (!single_line || value.contains(['\r', '\n'])) {
        return Err(format!("{label}包含不允许的控制字符"));
    }
    if single_line && value.contains(['\r', '\n']) {
        return Err(format!("{label}不能包含换行"));
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

mod requests;
mod response;
mod workspace;

pub use requests::{
    ApiAssertion, ApiRequestRecord, ApiRequestSpec, GrpcRequestSpec, HttpRequestSpec,
};
pub use response::{ApiResponseSnapshot, ApiResponseSnapshotParts, ApiResponseStatus};
pub use workspace::{ApiCollection, ApiEnvironment, ApiWorkspace};

#[cfg(test)]
mod tests;
