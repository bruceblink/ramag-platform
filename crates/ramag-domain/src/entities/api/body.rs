use std::fmt;

use serde::{Deserialize, Serialize};

use super::{
    MAX_API_MULTIPART_FILE_NAME_BYTES, MAX_API_MULTIPART_PARTS, MAX_API_MULTIPART_PATH_BYTES,
    MAX_API_PARAMETER_NAME_BYTES, MAX_API_PARAMETER_VALUE_BYTES, MAX_API_REQUEST_BODY_BYTES,
    validate_required_text, validate_text,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiBodyMode {
    #[default]
    Text,
    Multipart,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApiMultipartValue {
    Text {
        value: String,
    },
    File {
        path: String,
        #[serde(default)]
        file_name: Option<String>,
    },
}

impl fmt::Debug for ApiMultipartValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text { value } => formatter
                .debug_struct("Text")
                .field("value_bytes", &value.len())
                .field("value", &"[REDACTED]")
                .finish(),
            Self::File { path, file_name } => formatter
                .debug_struct("File")
                .field("path_bytes", &path.len())
                .field("path", &"[REDACTED]")
                .field("file_name", file_name)
                .finish(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiMultipartPart {
    pub name: String,
    pub value: ApiMultipartValue,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub sensitive: bool,
}

impl fmt::Debug for ApiMultipartPart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiMultipartPart")
            .field("name", &self.name)
            .field("value", &self.value)
            .field("content_type", &self.content_type)
            .field("sensitive", &self.sensitive)
            .finish()
    }
}

impl ApiMultipartPart {
    pub fn text(name: impl Into<String>, value: impl Into<String>, sensitive: bool) -> Self {
        Self {
            name: name.into(),
            value: ApiMultipartValue::Text {
                value: value.into(),
            },
            content_type: None,
            sensitive,
        }
    }

    pub fn file(
        name: impl Into<String>,
        path: impl Into<String>,
        file_name: Option<String>,
        content_type: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            value: ApiMultipartValue::File {
                path: path.into(),
                file_name,
            },
            content_type,
            sensitive: false,
        }
    }

    fn validate(&self) -> Result<(), String> {
        validate_required_text(
            "Multipart 字段名称",
            &self.name,
            MAX_API_PARAMETER_NAME_BYTES,
        )?;
        match &self.value {
            ApiMultipartValue::Text { value } => validate_text(
                "Multipart 文本字段值",
                value,
                MAX_API_PARAMETER_VALUE_BYTES,
                false,
            )?,
            ApiMultipartValue::File { path, file_name } => {
                validate_required_text("Multipart 文件路径", path, MAX_API_MULTIPART_PATH_BYTES)?;
                if let Some(file_name) = file_name {
                    validate_required_text(
                        "Multipart 文件名",
                        file_name,
                        MAX_API_MULTIPART_FILE_NAME_BYTES,
                    )?;
                }
            }
        }
        if let Some(content_type) = &self.content_type {
            validate_required_text(
                "Multipart Content-Type",
                content_type,
                MAX_API_PARAMETER_VALUE_BYTES,
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiBody {
    #[serde(default)]
    pub mode: ApiBodyMode,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub multipart: Vec<ApiMultipartPart>,
}

impl fmt::Debug for ApiBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiBody")
            .field("mode", &self.mode)
            .field("content_type", &self.content_type)
            .field("value_bytes", &self.value.len())
            .field("multipart_parts", &self.multipart.len())
            .field("value", &"[REDACTED]")
            .finish()
    }
}

impl ApiBody {
    pub fn text(value: impl Into<String>, content_type: Option<String>) -> Self {
        Self {
            mode: ApiBodyMode::Text,
            content_type,
            value: value.into(),
            multipart: Vec::new(),
        }
    }

    pub fn multipart(parts: Vec<ApiMultipartPart>) -> Self {
        Self {
            mode: ApiBodyMode::Multipart,
            content_type: None,
            value: String::new(),
            multipart: parts,
        }
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        match self.mode {
            ApiBodyMode::Text => {
                if !self.multipart.is_empty() {
                    return Err("普通请求正文不能包含 Multipart 字段".into());
                }
                if let Some(content_type) = &self.content_type {
                    validate_required_text(
                        "请求 Content-Type",
                        content_type,
                        MAX_API_PARAMETER_NAME_BYTES,
                    )?;
                }
                validate_text("请求正文", &self.value, MAX_API_REQUEST_BODY_BYTES, false)
            }
            ApiBodyMode::Multipart => {
                if !self.value.is_empty() || self.content_type.is_some() {
                    return Err("Multipart 请求正文不能同时设置普通正文或固定 Content-Type".into());
                }
                if self.multipart.is_empty() {
                    return Err("Multipart 请求至少需要一个字段".into());
                }
                if self.multipart.len() > MAX_API_MULTIPART_PARTS {
                    return Err(format!(
                        "Multipart 字段数量超过 {MAX_API_MULTIPART_PARTS} 个上限"
                    ));
                }
                let mut text_bytes = 0usize;
                for part in &self.multipart {
                    part.validate()?;
                    if let ApiMultipartValue::Text { value } = &part.value {
                        text_bytes = text_bytes
                            .checked_add(value.len())
                            .ok_or_else(|| "Multipart 文本字段总大小计算溢出".to_string())?;
                    }
                }
                if text_bytes > MAX_API_REQUEST_BODY_BYTES {
                    return Err(format!(
                        "Multipart 文本字段总大小超过 {MAX_API_REQUEST_BODY_BYTES} bytes 上限"
                    ));
                }
                Ok(())
            }
        }
    }
}
