use std::fmt;

use serde::{Deserialize, Serialize};

use super::{
    ApiParameter, ApiProtocol, MAX_API_ERROR_BYTES, MAX_API_RESPONSE_BODY_BYTES,
    validate_parameters, validate_text,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiResponseStatus {
    Http { code: u16 },
    Grpc { code: String },
    TransportError,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiResponseSnapshot {
    pub protocol: ApiProtocol,
    pub status: ApiResponseStatus,
    #[serde(default)]
    pub headers: Vec<ApiParameter>,
    #[serde(default)]
    pub metadata: Vec<ApiParameter>,
    #[serde(default)]
    pub body: Vec<u8>,
    pub elapsed_millis: u64,
    pub size_bytes: u64,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiResponseSnapshotParts {
    pub protocol: ApiProtocol,
    pub status: ApiResponseStatus,
    #[serde(default)]
    pub headers: Vec<ApiParameter>,
    #[serde(default)]
    pub metadata: Vec<ApiParameter>,
    #[serde(default)]
    pub body: Vec<u8>,
    pub elapsed_millis: u64,
    pub size_bytes: u64,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub error: Option<String>,
}

impl fmt::Debug for ApiResponseSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiResponseSnapshot")
            .field("protocol", &self.protocol)
            .field("status", &self.status)
            .field("headers_count", &self.headers.len())
            .field("metadata_count", &self.metadata.len())
            .field("body_bytes", &self.body.len())
            .field("elapsed_millis", &self.elapsed_millis)
            .field("size_bytes", &self.size_bytes)
            .field("truncated", &self.truncated)
            .field("error", &self.error)
            .finish()
    }
}

impl ApiResponseSnapshot {
    /// 构造有界响应快照；超出正文限制的响应必须在驱动层截断后再创建。
    pub fn new(parts: ApiResponseSnapshotParts) -> Result<Self, String> {
        let snapshot = Self {
            protocol: parts.protocol,
            status: parts.status,
            headers: parts.headers,
            metadata: parts.metadata,
            body: parts.body,
            elapsed_millis: parts.elapsed_millis,
            size_bytes: parts.size_bytes,
            truncated: parts.truncated,
            error: parts.error,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.body.len() > MAX_API_RESPONSE_BODY_BYTES {
            return Err(format!(
                "API 响应正文超过 {MAX_API_RESPONSE_BODY_BYTES} bytes 上限"
            ));
        }
        if self.size_bytes < self.body.len() as u64 {
            return Err("API 响应 size_bytes 不能小于已缓存正文长度".into());
        }
        validate_parameters("API 响应 Headers", &self.headers, true)?;
        validate_parameters("API 响应 Metadata", &self.metadata, true)?;
        if let Some(error) = &self.error {
            validate_text("API 响应错误", error, MAX_API_ERROR_BYTES, false)?;
        }
        Ok(())
    }
}
