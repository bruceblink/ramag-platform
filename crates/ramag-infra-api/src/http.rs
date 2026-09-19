//! HTTP API 请求驱动。
//!
//! 该驱动只负责把领域请求转换为一次 HTTP 调用，并把响应转换成有界快照。
//! 它不记录请求历史，也不把凭据写入错误文本；请求变量由应用层传入并在发送前展开。

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytes::Bytes;
use http::header::{CONTENT_TYPE, HeaderName, HeaderValue};
use http::{HeaderMap, Method};
use ramag_domain::entities::{
    ApiAuth, ApiCancellation, ApiKeyLocation, ApiParameter, ApiProtocol, ApiRequestSpec,
    ApiResponseSnapshot, ApiResponseSnapshotParts, ApiResponseStatus, ApiTlsConfig, ApiTlsVerify,
    MAX_API_PARAMETER_COUNT, MAX_API_PARAMETER_VALUE_BYTES, MAX_API_REQUEST_BODY_BYTES,
    MAX_API_RESPONSE_BODY_BYTES, MAX_API_URL_TEMPLATE_BYTES,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::ApiDriver;
use reqwest::{Client, RequestBuilder, Response, Url};
use rustls::crypto::CryptoProvider;

const MAX_TLS_FILE_BYTES: usize = 4 * 1024 * 1024;
const CANCELLATION_POLL: Duration = Duration::from_millis(10);

#[derive(Clone)]
pub struct HttpApiDriver {
    client: Client,
}

impl HttpApiDriver {
    /// 创建不带工作区状态的 HTTP 客户端；TLS Provider 在基础设施边界确保可用。
    pub fn new() -> Result<Self> {
        ensure_tls_provider()?;
        Ok(Self {
            client: build_client(&ApiTlsConfig::default())?,
        })
    }
}

#[async_trait]
impl ApiDriver for HttpApiDriver {
    fn protocol(&self) -> ApiProtocol {
        ApiProtocol::Http
    }

    /// 校验并发送 HTTP 请求，随后以固定大小读取响应；取消标记会中断发送和响应流读取。
    async fn execute(
        &self,
        request: &ApiRequestSpec,
        variables: &BTreeMap<String, String>,
        cancelled: ApiCancellation,
    ) -> Result<ApiResponseSnapshot> {
        let spec = match request {
            ApiRequestSpec::Http(spec) => spec,
            ApiRequestSpec::Grpc(_) => {
                return Err(DomainError::InvalidConfig(
                    "HTTP 驱动不能执行 gRPC 请求".into(),
                ));
            }
        };
        spec.validate().map_err(DomainError::InvalidConfig)?;
        ensure_not_cancelled(&cancelled)?;

        let client = if spec.tls == ApiTlsConfig::default() {
            self.client.clone()
        } else {
            build_client(&spec.tls)?
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

        let mut query = Vec::with_capacity(spec.query.len() + 1);
        for parameter in &spec.query {
            query.push(expand_parameter(parameter, variables, "HTTP 查询参数")?);
        }
        let mut header_parameters = Vec::with_capacity(spec.headers.len() + 1);
        for parameter in &spec.headers {
            header_parameters.push(expand_parameter(parameter, variables, "HTTP Headers")?);
        }
        apply_auth(&spec.auth, variables, &mut query, &mut header_parameters)?;
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
                let value = HeaderValue::try_from(content_type.as_str())
                    .map_err(|_| DomainError::InvalidConfig("HTTP Content-Type 无效".into()))?;
                builder = builder.header(CONTENT_TYPE, value);
            }
            builder = builder.body(body_value);
        }

        let started = Instant::now();
        let response = send_request(builder, cancelled.clone()).await?;
        read_response(response, started, cancelled).await
    }
}

type ExpandedParameter = (String, String, bool);

// 把认证配置展开成请求参数；敏感标记只影响响应/日志摘要，不改变实际发送值。
fn apply_auth(
    auth: &ApiAuth,
    variables: &BTreeMap<String, String>,
    query: &mut Vec<ExpandedParameter>,
    headers: &mut Vec<ExpandedParameter>,
) -> Result<()> {
    match auth {
        ApiAuth::None => {}
        ApiAuth::Basic { username, password } => {
            let username = expand_template(
                username,
                variables,
                "Basic 用户名",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?;
            let password = expand_template(
                password,
                variables,
                "Basic 密码",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?;
            let encoded = base64_basic(&username, &password);
            headers.push(("authorization".into(), format!("Basic {encoded}"), true));
        }
        ApiAuth::Bearer { token } => {
            let token = expand_template(
                token,
                variables,
                "Bearer Token",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?;
            headers.push(("authorization".into(), format!("Bearer {token}"), true));
        }
        ApiAuth::ApiKey {
            name,
            value,
            location,
        } => {
            let name = expand_template(
                name,
                variables,
                "API Key 名称",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?;
            let value = expand_template(
                value,
                variables,
                "API Key 值",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?;
            match location {
                ApiKeyLocation::Header => headers.push((name, value, true)),
                ApiKeyLocation::Query => query.push((name, value, true)),
            }
        }
    }
    Ok(())
}

fn expand_parameter(
    parameter: &ApiParameter,
    variables: &BTreeMap<String, String>,
    label: &str,
) -> Result<ExpandedParameter> {
    Ok((
        expand_template(
            &parameter.name,
            variables,
            label,
            MAX_API_PARAMETER_VALUE_BYTES,
        )?,
        expand_template(
            &parameter.value,
            variables,
            label,
            MAX_API_PARAMETER_VALUE_BYTES,
        )?,
        parameter.sensitive,
    ))
}

fn expand_template(
    template: &str,
    variables: &BTreeMap<String, String>,
    label: &str,
    max_bytes: usize,
) -> Result<String> {
    // 逐个替换 {{name}}，并在生成结果上限处停止，避免变量把请求体或 URL 放大到无界。
    let mut output = String::with_capacity(template.len());
    let mut cursor = 0usize;
    while let Some(relative_start) = template[cursor..].find("{{") {
        let start = cursor + relative_start;
        output.push_str(&template[cursor..start]);
        let value_start = start + 2;
        let relative_end = template[value_start..]
            .find("}}")
            .ok_or_else(|| DomainError::InvalidConfig(format!("{label}变量占位符未闭合")))?;
        let end = value_start + relative_end;
        let name = template[value_start..end].trim();
        if name.is_empty() {
            return Err(DomainError::InvalidConfig(format!("{label}变量名不能为空")));
        }
        let value = variables
            .get(name)
            .ok_or_else(|| DomainError::InvalidConfig(format!("缺少 API 环境变量：{name}")))?;
        output.push_str(value);
        cursor = end + 2;
    }
    output.push_str(&template[cursor..]);
    if output.len() > max_bytes {
        return Err(DomainError::InvalidConfig(format!(
            "{label}展开后超过 {max_bytes} bytes 上限"
        )));
    }
    Ok(output)
}

async fn send_request(builder: RequestBuilder, cancelled: ApiCancellation) -> Result<Response> {
    // reqwest 的发送 Future 被 pin 后与取消轮询并行；取消时丢弃 Future 会关闭底层请求。
    let mut send = Box::pin(builder.send());
    tokio::select! {
        result = &mut send => result.map_err(map_request_error),
        _ = wait_until_cancelled(cancelled) => Err(DomainError::Cancelled("HTTP 请求已取消".into())),
    }
}

async fn read_response(
    mut response: Response,
    started: Instant,
    cancelled: ApiCancellation,
) -> Result<ApiResponseSnapshot> {
    // 只保留领域层允许的正文前缀，同时用 Content-Length 或已读取字节数记录原始大小。
    let headers = response_headers(response.headers())?;
    let status = ApiResponseStatus::Http {
        code: response.status().as_u16(),
    };
    let declared_size = response.content_length();
    let mut body = Vec::new();
    let mut observed_size = 0u64;
    let mut truncated = declared_size.is_some_and(|size| size > MAX_API_RESPONSE_BODY_BYTES as u64);

    while let Some(chunk) = next_chunk(&mut response, cancelled.clone()).await? {
        observed_size = observed_size.saturating_add(chunk.len() as u64);
        if body.len() < MAX_API_RESPONSE_BODY_BYTES {
            let remaining = MAX_API_RESPONSE_BODY_BYTES - body.len();
            body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        }
        if observed_size > MAX_API_RESPONSE_BODY_BYTES as u64 {
            truncated = true;
            break;
        }
    }

    let size_bytes = declared_size.unwrap_or(observed_size);
    ApiResponseSnapshot::new(ApiResponseSnapshotParts {
        protocol: ApiProtocol::Http,
        status,
        headers,
        metadata: Vec::new(),
        body,
        elapsed_millis: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        size_bytes,
        truncated,
        error: None,
    })
    .map_err(DomainError::InvalidConfig)
}

async fn next_chunk(response: &mut Response, cancelled: ApiCancellation) -> Result<Option<Bytes>> {
    // 每个响应块都检查取消标记，避免慢响应占用请求任务直到网络自然结束。
    tokio::select! {
        result = response.chunk() => result.map_err(map_request_error),
        _ = wait_until_cancelled(cancelled) => Err(DomainError::Cancelled("HTTP 请求已取消".into())),
    }
}

async fn wait_until_cancelled(cancelled: ApiCancellation) {
    while !cancelled.load(Ordering::Relaxed) {
        tokio::time::sleep(CANCELLATION_POLL).await;
    }
}

fn response_headers(headers: &HeaderMap) -> Result<Vec<ApiParameter>> {
    if headers.len() > MAX_API_PARAMETER_COUNT {
        return Err(DomainError::InvalidConfig(format!(
            "HTTP 响应 Headers 数量超过 {MAX_API_PARAMETER_COUNT} 个上限"
        )));
    }
    headers
        .iter()
        .map(|(name, value)| {
            let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
            if value.len() > MAX_API_PARAMETER_VALUE_BYTES {
                return Err(DomainError::InvalidConfig(
                    "HTTP 响应 Header 值超过长度上限".into(),
                ));
            }
            Ok(ApiParameter::new(
                name.as_str(),
                value,
                is_sensitive_header(name.as_str()),
            ))
        })
        .collect()
}

fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "proxy-authorization" | "cookie" | "set-cookie" | "www-authenticate"
    )
}

fn build_client(tls: &ApiTlsConfig) -> Result<Client> {
    // 为每种 TLS 配置创建独立客户端，加载受限大小的证书材料并关闭自动重定向。
    let mut builder = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .user_agent(concat!("Ramag/", env!("CARGO_PKG_VERSION")));
    if matches!(tls.verify, ApiTlsVerify::None) {
        builder = builder.danger_accept_invalid_certs(true);
    }
    if let Some(path) = &tls.ca_cert_path {
        let bytes = read_tls_file(path, "CA 证书")?;
        let certificate = reqwest::Certificate::from_pem(&bytes)
            .map_err(|_| DomainError::InvalidConfig("CA 证书格式无效".into()))?;
        builder = builder.add_root_certificate(certificate);
    }
    if let (Some(cert_path), Some(key_path)) = (&tls.client_cert_path, &tls.client_key_path) {
        let mut identity_bytes = read_tls_file(cert_path, "客户端证书")?;
        identity_bytes.extend_from_slice(&read_tls_file(key_path, "客户端密钥")?);
        let identity = reqwest::Identity::from_pem(&identity_bytes)
            .map_err(|_| DomainError::InvalidConfig("客户端证书或密钥格式无效".into()))?;
        builder = builder.identity(identity);
    }
    builder
        .build()
        .map_err(|_| DomainError::InvalidConfig("创建 HTTP 客户端失败".into()))
}

fn read_tls_file(path: &str, label: &str) -> Result<Vec<u8>> {
    let mut file =
        File::open(path).map_err(|_| DomainError::InvalidConfig(format!("{label}文件无法读取")))?;
    let length = file
        .seek(SeekFrom::End(0))
        .map_err(|_| DomainError::InvalidConfig(format!("{label}文件大小无法读取")))?;
    if length > MAX_TLS_FILE_BYTES as u64 {
        return Err(DomainError::InvalidConfig(format!(
            "{label}文件超过 {MAX_TLS_FILE_BYTES} bytes 上限"
        )));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| DomainError::InvalidConfig(format!("{label}文件无法读取")))?;
    let mut bytes = Vec::with_capacity(length as usize);
    file.read_to_end(&mut bytes)
        .map_err(|_| DomainError::InvalidConfig(format!("{label}文件无法读取")))?;
    Ok(bytes)
}

fn ensure_tls_provider() -> Result<()> {
    if CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    if CryptoProvider::get_default().is_none() {
        return Err(DomainError::InvalidConfig(
            "无法安装 TLS 加密 Provider".into(),
        ));
    }
    Ok(())
}

fn ensure_not_cancelled(cancelled: &ApiCancellation) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        Err(DomainError::Cancelled("HTTP 请求已取消".into()))
    } else {
        Ok(())
    }
}

fn map_request_error(error: reqwest::Error) -> DomainError {
    if error.is_timeout() {
        DomainError::ConnectionFailed("HTTP 请求超时".into())
    } else if error.is_builder() {
        DomainError::InvalidConfig("HTTP 请求构造失败".into())
    } else {
        DomainError::ConnectionFailed("HTTP 请求失败".into())
    }
}

fn base64_basic(username: &str, password: &str) -> String {
    use base64::Engine as _;

    base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
