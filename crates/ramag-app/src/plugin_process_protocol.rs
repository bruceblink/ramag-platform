//! 进程外插件协议的帧和 JSON 信封；本模块不启动进程，也不访问凭据或 Shell。

use std::io;

use base64::Engine as _;
use ramag_domain::traits::{
    CURRENT_PLUGIN_API_VERSION, MAX_PLUGIN_CAPABILITIES, MAX_PLUGIN_ENTRY_ID_BYTES,
    MAX_PLUGIN_ENTRY_PAYLOAD_BYTES, MAX_PLUGIN_ID_BYTES, PluginApiVersion, PluginCapability,
    PluginId,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// 当前插件进程协议主版本。
pub const CURRENT_PLUGIN_PROTOCOL_VERSION: u16 = 1;
/// 单帧 JSON 数据的最大字节数；包含消息信封，不包含 4 字节长度前缀。
pub const MAX_PLUGIN_FRAME_BYTES: usize = 8 * 1024 * 1024;
/// 协议错误摘要的最大字节数。
pub const MAX_PLUGIN_PROTOCOL_ERROR_BYTES: usize = 512;
/// 单个响应输出的最大字节数。
pub const MAX_PLUGIN_PROTOCOL_OUTPUT_BYTES: usize = 1024 * 1024;

/// 插件进程启动后由宿主发送的握手消息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginHello {
    pub protocol_version: u16,
    pub plugin_id: PluginId,
    pub api_version: PluginApiVersion,
    pub granted_capabilities: Vec<PluginCapability>,
    pub max_frame_bytes: usize,
}

/// 插件进程确认握手时返回的消息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginReady {
    pub protocol_version: u16,
    pub plugin_id: PluginId,
    pub api_version: PluginApiVersion,
    pub capabilities: Vec<PluginCapability>,
}

/// 宿主提交的入口调用消息；输入以 base64 编码写入 JSON。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInvoke {
    pub request_id: u64,
    pub entry_id: String,
    #[serde(with = "base64_bytes")]
    pub input: Vec<u8>,
}

/// 插件进程返回的入口结果；输出以 base64 编码写入 JSON。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginResponse {
    pub request_id: u64,
    pub ok: bool,
    #[serde(default, with = "base64_bytes")]
    pub output: Vec<u8>,
    #[serde(default)]
    pub error: Option<String>,
}

/// 宿主请求插件进程结束当前会话。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginShutdown {
    pub request_id: u64,
}

/// 进程外插件协议的 JSON 信封。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginProcessMessage {
    Hello(PluginHello),
    Ready(PluginReady),
    Invoke(PluginInvoke),
    Response(PluginResponse),
    Shutdown(PluginShutdown),
}

impl PluginProcessMessage {
    /// 校验协议版本、身份、能力和载荷上限。
    pub fn validate(&self) -> Result<(), PluginProcessProtocolError> {
        match self {
            Self::Hello(message) => {
                validate_protocol_version(message.protocol_version)?;
                validate_plugin_id(&message.plugin_id)?;
                validate_api_version(message.api_version)?;
                validate_capabilities(&message.granted_capabilities)?;
                if message.max_frame_bytes == 0 || message.max_frame_bytes > MAX_PLUGIN_FRAME_BYTES
                {
                    return Err(PluginProcessProtocolError::InvalidFrameLimit {
                        value: message.max_frame_bytes,
                        max: MAX_PLUGIN_FRAME_BYTES,
                    });
                }
            }
            Self::Ready(message) => {
                validate_protocol_version(message.protocol_version)?;
                validate_plugin_id(&message.plugin_id)?;
                validate_api_version(message.api_version)?;
                validate_capabilities(&message.capabilities)?;
            }
            Self::Invoke(message) => {
                validate_request_id(message.request_id)?;
                validate_entry_id(&message.entry_id)?;
                if message.input.len() > MAX_PLUGIN_ENTRY_PAYLOAD_BYTES {
                    return Err(PluginProcessProtocolError::PayloadTooLarge {
                        bytes: message.input.len(),
                        max: MAX_PLUGIN_ENTRY_PAYLOAD_BYTES,
                    });
                }
            }
            Self::Response(message) => {
                validate_request_id(message.request_id)?;
                if message.output.len() > MAX_PLUGIN_PROTOCOL_OUTPUT_BYTES {
                    return Err(PluginProcessProtocolError::OutputTooLarge {
                        bytes: message.output.len(),
                        max: MAX_PLUGIN_PROTOCOL_OUTPUT_BYTES,
                    });
                }
                if let Some(error) = &message.error
                    && error.len() > MAX_PLUGIN_PROTOCOL_ERROR_BYTES
                {
                    return Err(PluginProcessProtocolError::ErrorTooLong {
                        max: MAX_PLUGIN_PROTOCOL_ERROR_BYTES,
                    });
                }
                if message.ok && message.error.is_some() {
                    return Err(PluginProcessProtocolError::ResponseShape);
                }
                if !message.ok && message.error.is_none() {
                    return Err(PluginProcessProtocolError::ResponseShape);
                }
            }
            Self::Shutdown(message) => validate_request_id(message.request_id)?,
        }
        Ok(())
    }
}

/// 将消息编码为长度前缀帧；帧内容超过上限时不分配完整输出。
pub fn encode_frame(message: &PluginProcessMessage) -> Result<Vec<u8>, PluginProcessProtocolError> {
    message.validate()?;
    let payload = serde_json::to_vec(message).map_err(|_| PluginProcessProtocolError::Encode)?;
    ensure_frame_size(payload.len())?;
    let length =
        u32::try_from(payload.len()).map_err(|_| PluginProcessProtocolError::FrameTooLarge {
            bytes: payload.len(),
        })?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

/// 解码一个完整长度前缀帧，并重新校验消息边界。
pub fn decode_frame(frame: &[u8]) -> Result<PluginProcessMessage, PluginProcessProtocolError> {
    if frame.len() < 4 {
        return Err(PluginProcessProtocolError::Truncated);
    }
    let length = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
    ensure_frame_size(length)?;
    let expected = length
        .checked_add(4)
        .ok_or(PluginProcessProtocolError::FrameTooLarge { bytes: length })?;
    if frame.len() != expected {
        return Err(PluginProcessProtocolError::LengthMismatch {
            declared: length,
            actual: frame.len().saturating_sub(4),
        });
    }
    let message = serde_json::from_slice::<PluginProcessMessage>(&frame[4..])
        .map_err(|_| PluginProcessProtocolError::Decode)?;
    message.validate()?;
    Ok(message)
}

/// 从异步输入流读取一帧；先读取长度并校验，再分配 payload 缓冲区。
pub async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<PluginProcessMessage, PluginProcessProtocolError> {
    let mut prefix = [0_u8; 4];
    reader.read_exact(&mut prefix).await.map_err(map_io_error)?;
    let length = u32::from_le_bytes(prefix) as usize;
    ensure_frame_size(length)?;
    let mut payload = vec![0_u8; length];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(map_io_error)?;
    let mut frame = Vec::with_capacity(length.saturating_add(4));
    frame.extend_from_slice(&prefix);
    frame.extend_from_slice(&payload);
    decode_frame(&frame)
}

/// 将一帧完整写入异步输出流，并刷新管道。
pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    message: &PluginProcessMessage,
) -> Result<(), PluginProcessProtocolError> {
    let frame = encode_frame(message)?;
    writer.write_all(&frame).await.map_err(map_io_error)?;
    writer.flush().await.map_err(map_io_error)
}

fn validate_protocol_version(version: u16) -> Result<(), PluginProcessProtocolError> {
    if version != CURRENT_PLUGIN_PROTOCOL_VERSION {
        return Err(PluginProcessProtocolError::UnsupportedProtocolVersion {
            version,
            current: CURRENT_PLUGIN_PROTOCOL_VERSION,
        });
    }
    Ok(())
}

fn validate_plugin_id(plugin_id: &PluginId) -> Result<(), PluginProcessProtocolError> {
    if plugin_id.as_str().len() > MAX_PLUGIN_ID_BYTES {
        return Err(PluginProcessProtocolError::InvalidPluginId);
    }
    plugin_id
        .validate()
        .map_err(|_| PluginProcessProtocolError::InvalidPluginId)
}

fn validate_api_version(version: PluginApiVersion) -> Result<(), PluginProcessProtocolError> {
    if !version.is_supported_by(CURRENT_PLUGIN_API_VERSION) {
        return Err(PluginProcessProtocolError::UnsupportedApiVersion {
            plugin: version,
            host: CURRENT_PLUGIN_API_VERSION,
        });
    }
    Ok(())
}

fn validate_capabilities(
    capabilities: &[PluginCapability],
) -> Result<(), PluginProcessProtocolError> {
    if capabilities.len() > MAX_PLUGIN_CAPABILITIES {
        return Err(PluginProcessProtocolError::TooManyCapabilities {
            max: MAX_PLUGIN_CAPABILITIES,
        });
    }
    let mut seen = std::collections::HashSet::with_capacity(capabilities.len());
    for capability in capabilities {
        if !capability.is_known() {
            return Err(PluginProcessProtocolError::UnknownCapability {
                capability: capability.as_str().to_owned(),
            });
        }
        if !seen.insert(capability) {
            return Err(PluginProcessProtocolError::DuplicateCapability {
                capability: capability.as_str().to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_entry_id(entry_id: &str) -> Result<(), PluginProcessProtocolError> {
    if entry_id.is_empty()
        || entry_id.len() > MAX_PLUGIN_ENTRY_ID_BYTES
        || entry_id.chars().any(char::is_control)
    {
        return Err(PluginProcessProtocolError::InvalidEntryId);
    }
    Ok(())
}

fn validate_request_id(request_id: u64) -> Result<(), PluginProcessProtocolError> {
    if request_id == 0 {
        return Err(PluginProcessProtocolError::InvalidRequestId);
    }
    Ok(())
}

fn ensure_frame_size(bytes: usize) -> Result<(), PluginProcessProtocolError> {
    if bytes == 0 {
        return Err(PluginProcessProtocolError::ZeroLengthFrame);
    }
    if bytes > MAX_PLUGIN_FRAME_BYTES {
        return Err(PluginProcessProtocolError::FrameTooLarge { bytes });
    }
    Ok(())
}

fn map_io_error(error: io::Error) -> PluginProcessProtocolError {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        PluginProcessProtocolError::Truncated
    } else {
        PluginProcessProtocolError::Io(error)
    }
}

#[derive(Debug, Error)]
pub enum PluginProcessProtocolError {
    #[error("插件协议帧长度为零")]
    ZeroLengthFrame,
    #[error("插件协议帧超过上限（实际 {bytes} 字节）")]
    FrameTooLarge { bytes: usize },
    #[error("插件协议帧被截断")]
    Truncated,
    #[error("插件协议声明 {declared} 字节，实际 {actual} 字节")]
    LengthMismatch { declared: usize, actual: usize },
    #[error("插件协议 JSON 编码失败")]
    Encode,
    #[error("插件协议 JSON 解码失败")]
    Decode,
    #[error("插件协议版本 {version} 不受支持，当前版本为 {current}")]
    UnsupportedProtocolVersion { version: u16, current: u16 },
    #[error("插件 API {plugin} 不受宿主 API {host} 支持")]
    UnsupportedApiVersion {
        plugin: PluginApiVersion,
        host: PluginApiVersion,
    },
    #[error("插件 ID 无效")]
    InvalidPluginId,
    #[error("插件能力超过 {max} 项")]
    TooManyCapabilities { max: usize },
    #[error("未知插件能力：{capability}")]
    UnknownCapability { capability: String },
    #[error("插件能力重复：{capability}")]
    DuplicateCapability { capability: String },
    #[error("插件入口 ID 无效")]
    InvalidEntryId,
    #[error("插件请求 ID 无效")]
    InvalidRequestId,
    #[error("插件入口输入超过 {max} 字节（实际 {bytes} 字节）")]
    PayloadTooLarge { bytes: usize, max: usize },
    #[error("插件响应输出超过 {max} 字节（实际 {bytes} 字节）")]
    OutputTooLarge { bytes: usize, max: usize },
    #[error("插件响应错误摘要超过 {max} 字节")]
    ErrorTooLong { max: usize },
    #[error("插件响应的成功标志和错误字段不一致")]
    ResponseShape,
    #[error("插件握手声明的帧上限无效：{value}（最大 {max}）")]
    InvalidFrameLimit { value: usize, max: usize },
    #[error("插件协议 I/O 失败：{0}")]
    Io(#[source] io::Error),
}

mod base64_bytes {
    use super::*;

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
#[path = "plugin_process_protocol_tests.rs"]
mod tests;
