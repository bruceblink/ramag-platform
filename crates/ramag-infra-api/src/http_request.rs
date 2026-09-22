//! HTTP 请求构造；与响应读取和 Multipart 文件读取分开，保持传输边界有界。

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use http::Method;
use http::header::{CONTENT_TYPE, HeaderName, HeaderValue};
use ramag_domain::entities::{
    ApiAuth, ApiBodyMode, ApiCancellation, ApiProxyConfig, ApiTlsConfig, HttpRequestSpec,
    MAX_API_REQUEST_BODY_BYTES, MAX_API_URL_TEMPLATE_BYTES,
};
use ramag_domain::error::{DomainError, Result};
use reqwest::{Response, Url};

use super::{
    HttpApiDriver, apply_auth, build_client, ensure_not_cancelled, expand_parameter,
    expand_template, multipart_form, oauth2_bearer_auth, resolve_proxy_config, send_request,
};
use crate::oauth2::resolve_oauth2_config;

impl HttpApiDriver {
    pub(super) async fn execute_once(
        &self,
        spec: &HttpRequestSpec,
        variables: &BTreeMap<String, String>,
        cancelled: ApiCancellation,
        force_refresh: bool,
    ) -> Result<(Response, Instant)> {
        ensure_not_cancelled(&cancelled)?;
        let proxy = resolve_proxy_config(&spec.proxy, variables)?;
        let client = if spec.tls == ApiTlsConfig::default() && proxy == ApiProxyConfig::default() {
            self.client.clone()
        } else {
            build_client(&spec.tls, &proxy)?
        };
        let method = Method::from_bytes(spec.method.as_bytes())
            .map_err(|_| DomainError::InvalidConfig("HTTP 方法无效".into()))?;
        let url_text = expand_template(
            &spec.url_template,
            variables,
            "HTTP URL 模板",
            MAX_API_URL_TEMPLATE_BYTES,
        )?;
        let mut url = Url::parse(&url_text)
            .map_err(|_| DomainError::InvalidConfig("HTTP URL 模板无效".into()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(DomainError::InvalidConfig(
                "HTTP URL 只支持 http 或 https scheme".into(),
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(DomainError::InvalidConfig(
                "HTTP URL 不能包含用户名或密码".into(),
            ));
        }

        let mut query = spec
            .query
            .iter()
            .map(|parameter| expand_parameter(parameter, variables, "HTTP 查询参数"))
            .collect::<Result<Vec<_>>>()?;
        let mut header_parameters = spec
            .headers
            .iter()
            .map(|parameter| expand_parameter(parameter, variables, "HTTP Headers"))
            .collect::<Result<Vec<_>>>()?;
        if !matches!(&spec.auth, ApiAuth::None)
            && header_parameters
                .iter()
                .any(|(name, _, _)| name.eq_ignore_ascii_case("authorization"))
        {
            return Err(DomainError::InvalidConfig(
                "HTTP Header 不能与认证配置同时设置 authorization".into(),
            ));
        }
        let auth = match &spec.auth {
            ApiAuth::OAuth2 { config } => {
                let config = resolve_oauth2_config(config, variables)?;
                let token = self
                    .oauth2
                    .access_token(
                        &config,
                        &spec.tls,
                        &proxy,
                        Duration::from_millis(spec.timeout_millis),
                        cancelled.clone(),
                        force_refresh,
                    )
                    .await?;
                oauth2_bearer_auth(token)
            }
            auth => auth.clone(),
        };
        apply_auth(&auth, variables, &mut query, &mut header_parameters)?;
        {
            let mut query_pairs = url.query_pairs_mut();
            for (name, value, _) in &query {
                query_pairs.append_pair(name, value);
            }
        }

        let mut builder = client
            .request(method, url)
            .timeout(Duration::from_millis(spec.timeout_millis));
        for (name, value, _) in &header_parameters {
            let name = HeaderName::try_from(name.as_str())
                .map_err(|_| DomainError::InvalidConfig("HTTP Header 名称无效".into()))?;
            let value = HeaderValue::try_from(value.as_str())
                .map_err(|_| DomainError::InvalidConfig("HTTP Header 值无效".into()))?;
            builder = builder.header(name, value);
        }

        if let Some(body) = &spec.body {
            match body.mode {
                ApiBodyMode::Text => {
                    let body_value = expand_template(
                        &body.value,
                        variables,
                        "HTTP 请求正文",
                        MAX_API_REQUEST_BODY_BYTES,
                    )?;
                    if let Some(content_type) = &body.content_type
                        && !header_parameters
                            .iter()
                            .any(|(name, _, _)| name.eq_ignore_ascii_case("content-type"))
                    {
                        let value = HeaderValue::try_from(content_type.as_str()).map_err(|_| {
                            DomainError::InvalidConfig("HTTP Content-Type 无效".into())
                        })?;
                        builder = builder.header(CONTENT_TYPE, value);
                    }
                    builder = builder.body(body_value);
                }
                ApiBodyMode::Multipart => {
                    let form = multipart_form(body, variables, cancelled.clone()).await?;
                    builder = builder.multipart(form);
                }
            }
        }

        let started = Instant::now();
        let response = send_request(builder, cancelled).await?;
        Ok((response, started))
    }
}
