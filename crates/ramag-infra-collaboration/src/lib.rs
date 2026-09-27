//! 协作 Relay 的受限 HTTPS 客户端。
//!
//! 客户端只发送用户已经确认并通过领域校验的导出包；它不保存凭据、不跟随重定向，
//! 也不把响应正文或 URL 写入错误文本。服务端实现不属于本 crate。

#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ramag_domain::entities::{
    CollaborationRemotePackage, CollaborationRemoteReceipt, MAX_COLLABORATION_EXPORT_BYTES,
    MAX_COLLABORATION_REMOTE_ID_BYTES,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::CollaborationRelay;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};

const FORMAT: &str = "ramag.collaboration.v1";
const MAX_RESPONSE_BYTES: usize = MAX_COLLABORATION_EXPORT_BYTES + 512 * 1024;
const MAX_ENDPOINT_BYTES: usize = 4 * 1024;
const MAX_REMOTE_ID_BYTES: usize = MAX_COLLABORATION_REMOTE_ID_BYTES;

/// Relay 客户端默认拒绝环境代理和自动重定向，避免用户确认的目标被替换。
pub struct HttpCollaborationRelay {
    client: Client,
}

impl HttpCollaborationRelay {
    pub fn new() -> Result<Self> {
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            return Err(DomainError::InvalidConfig(
                "无法安装协作 Relay TLS 加密 Provider".into(),
            ));
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .user_agent(concat!("Ramag-Collaboration/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| DomainError::InvalidConfig("创建协作 Relay HTTP 客户端失败".into()))?;
        Ok(Self { client })
    }

    fn endpoint(&self, base: &str, suffix: &str) -> Result<Url> {
        validate_base_url(base)?;
        let base = Url::parse(base)
            .map_err(|_| DomainError::InvalidConfig("协作 Relay 地址无效".into()))?;
        base.join(suffix)
            .map_err(|_| DomainError::InvalidConfig("协作 Relay 路径无效".into()))
    }

    async fn read_json<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        if !response.status().is_success() {
            return Err(status_error(response.status()));
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
        {
            return Err(DomainError::InvalidConfig(
                "协作 Relay 响应超过大小上限".into(),
            ));
        }
        let mut body = Vec::new();
        let mut response = response;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| DomainError::ConnectionFailed("读取协作 Relay 响应失败".into()))?
        {
            if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(DomainError::InvalidConfig(
                    "协作 Relay 响应超过大小上限".into(),
                ));
            }
            body.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&body)
            .map_err(|_| DomainError::InvalidConfig("协作 Relay 响应格式无效".into()))
    }
}

#[derive(Debug, Serialize)]
struct PublishRequest<'a> {
    format: &'static str,
    payload: &'a str,
}

#[derive(Debug, Deserialize)]
struct RelayResponse {
    format: String,
    remote_id: String,
    revision: u64,
    #[serde(default)]
    expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    payload: Option<String>,
}

impl RelayResponse {
    fn receipt(&self) -> Result<CollaborationRemoteReceipt> {
        if self.format != FORMAT {
            return Err(DomainError::InvalidConfig(
                "协作 Relay 响应格式不支持".into(),
            ));
        }
        let receipt = CollaborationRemoteReceipt {
            remote_id: self.remote_id.clone(),
            revision: self.revision,
            expires_at: self.expires_at,
        };
        receipt
            .validate()
            .map_err(|error| DomainError::InvalidConfig(error.to_string()))?;
        Ok(receipt)
    }
}

#[async_trait]
impl CollaborationRelay for HttpCollaborationRelay {
    async fn publish(&self, endpoint: &str, payload: &str) -> Result<CollaborationRemoteReceipt> {
        if payload.len() > MAX_COLLABORATION_EXPORT_BYTES {
            return Err(DomainError::InvalidConfig(
                "协作导出包超过 Relay 请求大小上限".into(),
            ));
        }
        let url = self.endpoint(endpoint, "v1/collaboration/shares")?;
        let response = self
            .client
            .post(url)
            .json(&PublishRequest {
                format: FORMAT,
                payload,
            })
            .send()
            .await
            .map_err(|_| DomainError::ConnectionFailed("协作 Relay 请求失败".into()))?;
        let body: RelayResponse = self.read_json(response).await?;
        body.receipt()
    }

    async fn fetch(&self, endpoint: &str, remote_id: &str) -> Result<CollaborationRemotePackage> {
        validate_remote_id(remote_id)?;
        let escaped =
            url::form_urlencoded::byte_serialize(remote_id.as_bytes()).collect::<String>();
        let url = self.endpoint(endpoint, &format!("v1/collaboration/shares/{escaped}"))?;
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| DomainError::ConnectionFailed("协作 Relay 请求失败".into()))?;
        let body: RelayResponse = self.read_json(response).await?;
        let receipt = body.receipt()?;
        if receipt.remote_id != remote_id {
            return Err(DomainError::InvalidConfig(
                "协作 Relay 返回了不匹配的远端 ID".into(),
            ));
        }
        let payload = body
            .payload
            .ok_or_else(|| DomainError::InvalidConfig("协作 Relay 响应缺少 payload".into()))?;
        if payload.len() > MAX_COLLABORATION_EXPORT_BYTES {
            return Err(DomainError::InvalidConfig(
                "协作 Relay payload 超过大小上限".into(),
            ));
        }
        Ok(CollaborationRemotePackage { receipt, payload })
    }
}

fn validate_base_url(raw: &str) -> Result<()> {
    if raw.len() > MAX_ENDPOINT_BYTES || raw.trim() != raw || raw.chars().any(char::is_control) {
        return Err(DomainError::InvalidConfig("协作 Relay 地址无效".into()));
    }
    let url =
        Url::parse(raw).map_err(|_| DomainError::InvalidConfig("协作 Relay 地址无效".into()))?;
    if url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(DomainError::InvalidConfig(
            "协作 Relay 地址不能包含凭据、查询参数或片段".into(),
        ));
    }
    let Some(host) = url.host_str() else {
        return Err(DomainError::InvalidConfig("协作 Relay 地址缺少主机".into()));
    };
    let loopback = matches!(host, "localhost" | "127.0.0.1" | "::1");
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        return Err(DomainError::InvalidConfig(
            "协作 Relay 生产地址必须使用 HTTPS".into(),
        ));
    }
    Ok(())
}

fn validate_remote_id(remote_id: &str) -> Result<()> {
    if remote_id.is_empty()
        || remote_id.len() > MAX_REMOTE_ID_BYTES
        || remote_id
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | '?' | '#'))
    {
        return Err(DomainError::InvalidConfig("远端共享包 ID 无效".into()));
    }
    Ok(())
}

fn status_error(status: StatusCode) -> DomainError {
    if status == StatusCode::NOT_FOUND {
        DomainError::NotFound("远端共享包不存在".into())
    } else {
        DomainError::ConnectionFailed(format!("协作 Relay 返回 HTTP {}", status.as_u16()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_relay_requires_https_and_rejects_embedded_credentials() {
        assert!(validate_base_url("http://example.com").is_err());
        assert!(validate_base_url("https://user:pass@example.com").is_err());
        assert!(validate_base_url("https://example.com?token=secret").is_err());
        assert!(validate_base_url("http://127.0.0.1:18080").is_ok());
    }

    #[test]
    fn remote_ids_cannot_escape_the_share_path() {
        assert!(validate_remote_id("abc-123").is_ok());
        assert!(validate_remote_id("../secret").is_err());
        assert!(validate_remote_id("abc?x=1").is_err());
    }

    #[test]
    fn client_can_be_constructed_without_network_access() {
        assert!(HttpCollaborationRelay::new().is_ok());
    }
}
