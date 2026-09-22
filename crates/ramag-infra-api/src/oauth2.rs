//! OAuth2 Client Credentials 令牌获取与内存缓存。
//!
//! 客户端密钥只用于向 Token Endpoint 发起请求；访问令牌只保存在驱动进程内存中，并在
//! 到期前重新获取。这里不把令牌放进领域请求、响应历史、日志或错误文本。

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ramag_domain::entities::{
    ApiCancellation, ApiOAuth2Config, ApiProxyConfig, ApiTlsConfig, MAX_API_OAUTH2_SCOPE_BYTES,
    MAX_API_OAUTH2_URL_BYTES, MAX_API_PARAMETER_VALUE_BYTES,
};
use ramag_domain::error::{DomainError, Result};
use reqwest::{Client, Response};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::http::{build_client, send_request, wait_until_cancelled};

const MAX_TOKEN_RESPONSE_BYTES: usize = 64 * 1024;
const DEFAULT_TOKEN_LIFETIME: Duration = Duration::from_secs(300);
const TOKEN_REFRESH_MARGIN: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub(crate) struct OAuth2AccessToken {
    pub(crate) scheme: String,
    pub(crate) value: String,
}

impl fmt::Debug for OAuth2AccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuth2AccessToken")
            .field("scheme", &self.scheme)
            .field("value", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TokenCacheKey {
    config: ApiOAuth2Config,
    tls: ApiTlsConfig,
    proxy: ApiProxyConfig,
}

#[derive(Clone, Debug)]
struct CachedToken {
    key: TokenCacheKey,
    token: OAuth2AccessToken,
    expires_at: Instant,
}

#[derive(Clone, Default)]
pub(crate) struct OAuth2TokenProvider {
    cache: Arc<Mutex<Vec<CachedToken>>>,
}

impl fmt::Debug for OAuth2TokenProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuth2TokenProvider")
            .field("cache", &"[REDACTED]")
            .finish()
    }
}

impl OAuth2TokenProvider {
    pub(crate) async fn access_token(
        &self,
        config: &ApiOAuth2Config,
        tls: &ApiTlsConfig,
        proxy: &ApiProxyConfig,
        timeout: Duration,
        cancelled: ApiCancellation,
        force_refresh: bool,
    ) -> Result<OAuth2AccessToken> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(DomainError::Cancelled("OAuth2 令牌请求已取消".into()));
        }
        let key = TokenCacheKey {
            config: config.clone(),
            tls: tls.clone(),
            proxy: proxy.clone(),
        };
        let now = Instant::now();
        let refresh_deadline = now + TOKEN_REFRESH_MARGIN;
        {
            let cache = self.cache.lock().await;
            if !force_refresh
                && let Some(cached) = cache
                    .iter()
                    .find(|cached| cached.key == key && cached.expires_at > refresh_deadline)
            {
                return Ok(cached.token.clone());
            }
        }

        let client = build_client(tls, proxy)?;
        let token = request_token(&client, config, timeout, cancelled).await?;
        let expires_at = Instant::now() + token.1;
        let access_token = token.0;
        let mut cache = self.cache.lock().await;
        if !force_refresh
            && let Some(cached) = cache
                .iter()
                .find(|cached| cached.key == key && cached.expires_at > Instant::now())
        {
            return Ok(cached.token.clone());
        }
        cache.retain(|cached| cached.key != key);
        cache.push(CachedToken {
            key,
            token: access_token.clone(),
            expires_at,
        });
        if cache.len() > 32 {
            cache.remove(0);
        }
        Ok(access_token)
    }

    pub(crate) async fn invalidate(
        &self,
        config: &ApiOAuth2Config,
        tls: &ApiTlsConfig,
        proxy: &ApiProxyConfig,
    ) {
        let key = TokenCacheKey {
            config: config.clone(),
            tls: tls.clone(),
            proxy: proxy.clone(),
        };
        self.cache.lock().await.retain(|cached| cached.key != key);
    }
}

/// 展开一次请求使用的 OAuth2 配置，并在展开后再次执行 URL 和字段限制。
pub(crate) fn resolve_oauth2_config(
    config: &ApiOAuth2Config,
    variables: &BTreeMap<String, String>,
) -> Result<ApiOAuth2Config> {
    let resolved = ApiOAuth2Config {
        token_url: crate::http::expand_template(
            &config.token_url,
            variables,
            "OAuth2 Token URL",
            MAX_API_OAUTH2_URL_BYTES,
        )?,
        client_id: crate::http::expand_template(
            &config.client_id,
            variables,
            "OAuth2 Client ID",
            MAX_API_PARAMETER_VALUE_BYTES,
        )?,
        client_secret: crate::http::expand_template(
            &config.client_secret,
            variables,
            "OAuth2 Client Secret",
            MAX_API_PARAMETER_VALUE_BYTES,
        )?,
        scope: config
            .scope
            .as_ref()
            .map(|scope| {
                crate::http::expand_template(
                    scope,
                    variables,
                    "OAuth2 Scope",
                    MAX_API_OAUTH2_SCOPE_BYTES,
                )
            })
            .transpose()?,
    };
    resolved.validate().map_err(DomainError::InvalidConfig)?;
    Ok(resolved)
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

async fn request_token(
    client: &Client,
    config: &ApiOAuth2Config,
    timeout: Duration,
    cancelled: ApiCancellation,
) -> Result<(OAuth2AccessToken, Duration)> {
    let mut form = vec![("grant_type", "client_credentials")];
    if let Some(scope) = config
        .scope
        .as_deref()
        .filter(|scope| !scope.trim().is_empty())
    {
        form.push(("scope", scope));
    }
    let request = client
        .post(&config.token_url)
        .timeout(timeout)
        .basic_auth(&config.client_id, Some(&config.client_secret))
        .form(&form);
    let response = send_request(request, cancelled.clone()).await?;
    if !response.status().is_success() {
        return Err(
            if response.status().as_u16() == 401 || response.status().as_u16() == 400 {
                DomainError::ConnectionFailed("OAuth2 Token Endpoint 拒绝客户端认证".into())
            } else {
                DomainError::ConnectionFailed("OAuth2 Token Endpoint 返回错误".into())
            },
        );
    }
    let body = read_token_response(response, cancelled).await?;
    let parsed: TokenResponse = serde_json::from_slice(&body)
        .map_err(|_| DomainError::ConnectionFailed("OAuth2 Token Endpoint 响应无效".into()))?;
    if parsed.access_token.is_empty()
        || parsed.access_token.len() > MAX_API_PARAMETER_VALUE_BYTES
        || parsed.access_token.chars().any(char::is_control)
    {
        return Err(DomainError::ConnectionFailed(
            "OAuth2 Token Endpoint 未返回有效访问令牌".into(),
        ));
    }
    let scheme = parsed.token_type.unwrap_or_else(|| "Bearer".into());
    if !scheme.eq_ignore_ascii_case("Bearer")
        || scheme.is_empty()
        || scheme
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(DomainError::ConnectionFailed(
            "OAuth2 Token Endpoint 返回了不支持的令牌类型".into(),
        ));
    }
    let lifetime = match parsed.expires_in {
        Some(0) => {
            return Err(DomainError::ConnectionFailed(
                "OAuth2 Token Endpoint 返回了无效的有效期".into(),
            ));
        }
        Some(seconds) => Duration::from_secs(seconds.min(86_400)),
        None => DEFAULT_TOKEN_LIFETIME,
    };
    Ok((
        OAuth2AccessToken {
            scheme: "Bearer".into(),
            value: parsed.access_token,
        },
        lifetime,
    ))
}

async fn read_token_response(
    mut response: Response,
    cancelled: ApiCancellation,
) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_TOKEN_RESPONSE_BYTES as u64)
    {
        return Err(DomainError::ConnectionFailed(
            "OAuth2 Token Endpoint 响应超过大小限制".into(),
        ));
    }
    let mut body = Vec::new();
    while let Some(chunk) = tokio::select! {
        result = response.chunk() => result.map_err(|_| DomainError::ConnectionFailed("OAuth2 Token Endpoint 响应读取失败".into()))?,
        _ = wait_until_cancelled(cancelled.clone()) => return Err(DomainError::Cancelled("OAuth2 令牌响应读取已取消".into())),
    } {
        if body.len().saturating_add(chunk.len()) > MAX_TOKEN_RESPONSE_BYTES {
            return Err(DomainError::ConnectionFailed(
                "OAuth2 Token Endpoint 响应超过大小限制".into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
