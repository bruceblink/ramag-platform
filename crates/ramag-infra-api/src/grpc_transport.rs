//! gRPC Endpoint、TLS、Metadata 和响应快照适配。

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use base64::Engine as _;
use http::Uri;
use hyper_util::rt::TokioIo;
use prost_reflect::DynamicMessage;
use ramag_domain::entities::{
    ApiAuth, ApiCancellation, ApiKeyLocation, ApiParameter, ApiProtocol, ApiProxyConfig,
    ApiResponseSnapshot, ApiResponseSnapshotParts, ApiResponseStatus, ApiTlsConfig, ApiTlsVerify,
    MAX_API_PARAMETER_COUNT, MAX_API_PARAMETER_VALUE_BYTES, MAX_API_RESPONSE_BODY_BYTES,
};
use ramag_domain::error::{DomainError, Result as DomainResult};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tonic::codegen::Service;
use tonic::metadata::{
    AsciiMetadataKey, BinaryMetadataKey, BinaryMetadataValue, KeyAndValueRef, MetadataMap,
    MetadataValue,
};
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};
use tonic::{Response, Status};

use crate::http::{
    ExpandedParameter, ensure_not_cancelled as check_not_cancelled, expand_parameter,
    expand_template, read_tls_file,
};
use crate::oauth2::{OAuth2TokenProvider, resolve_oauth2_config};

pub(super) fn build_endpoint(
    endpoint_text: String,
    tls: &ApiTlsConfig,
    timeout: Duration,
) -> DomainResult<Endpoint> {
    let uri: Uri = endpoint_text
        .parse()
        .map_err(|_| DomainError::InvalidConfig("gRPC Endpoint 无效".into()))?;
    let scheme = uri
        .scheme_str()
        .ok_or_else(|| DomainError::InvalidConfig("gRPC Endpoint 缺少 scheme".into()))?;
    if !matches!(scheme, "http" | "https") {
        return Err(DomainError::InvalidConfig(
            "gRPC Endpoint 只支持 http 或 https scheme".into(),
        ));
    }
    let endpoint = Endpoint::from_shared(endpoint_text)
        .map_err(|_| DomainError::InvalidConfig("gRPC Endpoint 无效".into()))?
        .connect_timeout(timeout)
        .timeout(timeout);
    if scheme == "http" {
        if *tls != ApiTlsConfig::default() {
            return Err(DomainError::InvalidConfig(
                "明文 gRPC Endpoint 不能配置 TLS".into(),
            ));
        }
        return Ok(endpoint);
    }

    let host = uri
        .authority()
        .map(|authority| authority.host().trim_matches(['[', ']']))
        .filter(|host| !host.is_empty())
        .ok_or_else(|| DomainError::InvalidConfig("gRPC TLS Endpoint 缺少主机名".into()))?;
    let mut tls_config = ClientTlsConfig::new().domain_name(host);
    match tls.verify {
        ApiTlsVerify::None => {}
        ApiTlsVerify::Ca => {
            tls_config = tls_config.ca_certificate(load_ca_certificate(tls)?);
        }
        ApiTlsVerify::Full => {
            tls_config = tls_config.with_enabled_roots();
            if tls.ca_cert_path.is_some() {
                tls_config = tls_config.ca_certificate(load_ca_certificate(tls)?);
            }
        }
    }
    if let (Some(cert_path), Some(key_path)) = (&tls.client_cert_path, &tls.client_key_path) {
        let certificate = read_tls_file(cert_path, "客户端证书")?;
        let key = read_tls_file(key_path, "客户端密钥")?;
        tls_config = tls_config.identity(Identity::from_pem(certificate, key));
    }
    if matches!(tls.verify, ApiTlsVerify::None) {
        endpoint
            .tls_config_with_verifier(tls_config, Arc::new(NoCertificateVerification))
            .map_err(|_| DomainError::InvalidConfig("gRPC TLS 配置无效".into()))
    } else {
        endpoint
            .tls_config(tls_config)
            .map_err(|_| DomainError::InvalidConfig("gRPC TLS 配置无效".into()))
    }
}

fn load_ca_certificate(tls: &ApiTlsConfig) -> DomainResult<Certificate> {
    let path = tls
        .ca_cert_path
        .as_deref()
        .ok_or_else(|| DomainError::InvalidConfig("gRPC verify=Ca 必须配置 CA 证书路径".into()))?;
    let bytes = read_tls_file(path, "CA 证书")?;
    Ok(Certificate::from_pem(bytes))
}

pub(super) async fn connect_endpoint(
    endpoint: Endpoint,
    proxy: &ApiProxyConfig,
    cancelled: ApiCancellation,
) -> DomainResult<Channel> {
    tokio::select! {
        result = connect_endpoint_inner(endpoint, proxy) => result.map_err(|_| DomainError::ConnectionFailed("gRPC 连接失败".into())),
        _ = crate::http::wait_until_cancelled(cancelled) => Err(DomainError::Cancelled("gRPC 连接已取消".into())),
    }
}

async fn connect_endpoint_inner(
    endpoint: Endpoint,
    proxy: &ApiProxyConfig,
) -> Result<Channel, tonic::transport::Error> {
    if proxy.is_enabled() {
        endpoint
            .connect_with_connector(ProxyConnector::new(proxy.clone()))
            .await
    } else {
        endpoint.connect().await
    }
}

/// 代理 CONNECT 响应头的最大字节数，防止未受信任代理持续发送头部而占用连接任务内存。
const MAX_PROXY_RESPONSE_BYTES: usize = 64 * 1024;

/// 为 tonic 提供显式代理隧道的连接器。
///
/// tonic 把目标 `Uri` 交给该结构后，它只建立一条到用户指定 HTTP 代理的 TCP 连接，并
/// 完成一次有界的 CONNECT 握手。成功后的原始字节流会交回 `Endpoint`：HTTP/2 协商和
/// HTTPS/mTLS 握手仍由 tonic/rustls 对原始目标主机执行，因此代理无法替代服务端身份校验。
/// 代理配置按请求克隆，避免并发请求共享可变认证状态。
#[derive(Clone)]
struct ProxyConnector {
    proxy: ApiProxyConfig,
}

impl ProxyConnector {
    fn new(proxy: ApiProxyConfig) -> Self {
        Self { proxy }
    }
}

impl Service<Uri> for ProxyConnector {
    type Response = TokioIo<TcpStream>;
    type Error = io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, target: Uri) -> Self::Future {
        let proxy = self.proxy.clone();
        Box::pin(async move { connect_through_proxy(&proxy, &target).await })
    }
}

async fn connect_through_proxy(
    proxy: &ApiProxyConfig,
    target: &Uri,
) -> Result<TokioIo<TcpStream>, io::Error> {
    // 先验证并连接代理，再把目标 authority 写入 CONNECT；响应只读取到空行，超过上限、
    // 非 2xx 状态和无效 authority 都立即关闭 Future。外层 select 丢弃本 Future 时，TCP
    // stream 也随之释放，从而让取消请求不遗留后台隧道。
    let proxy_uri: Uri = proxy
        .url
        .as_deref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "代理 URL 缺失"))?
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "代理 URL 无效"))?;
    let proxy_authority = proxy_uri
        .authority()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "代理 URL 缺少主机"))?;
    let proxy_host = proxy_authority.host().trim_matches(['[', ']']);
    let proxy_port = proxy_authority.port_u16().unwrap_or(80);
    let target_authority = target
        .authority()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "gRPC Endpoint 缺少主机"))?;
    let target_authority = if target_authority.port_u16().is_some() {
        target_authority.to_string()
    } else {
        let port = if target.scheme_str() == Some("https") {
            443
        } else {
            80
        };
        format!("{target_authority}:{port}")
    };
    let mut stream = TcpStream::connect((proxy_host, proxy_port)).await?;
    let mut request =
        format!("CONNECT {target_authority} HTTP/1.1\r\nHost: {target_authority}\r\n").into_bytes();
    if let (Some(username), Some(password)) = (&proxy.username, &proxy.password) {
        let credentials = format!("{username}:{password}");
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
        request.extend_from_slice(b"Proxy-Authorization: Basic ");
        request.extend_from_slice(encoded.as_bytes());
        request.extend_from_slice(b"\r\n");
    }
    request.extend_from_slice(b"\r\n");
    stream.write_all(&request).await?;

    let mut response = Vec::with_capacity(1024);
    loop {
        let mut byte = [0_u8; 1];
        stream.read_exact(&mut byte).await?;
        response.push(byte[0]);
        if response.ends_with(b"\r\n\r\n") {
            break;
        }
        if response.len() > MAX_PROXY_RESPONSE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "代理 CONNECT 响应超过限制",
            ));
        }
    }
    let status_line = response
        .split(|byte| *byte == b'\n')
        .next()
        .and_then(|line| std::str::from_utf8(line).ok())
        .unwrap_or_default();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "代理 CONNECT 响应无效"))?;
    if !(200..300).contains(&status) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "代理 CONNECT 被拒绝",
        ));
    }
    Ok(TokioIo::new(stream))
}

pub(super) fn request_metadata(
    parameters: &[ApiParameter],
    variables: &std::collections::BTreeMap<String, String>,
) -> DomainResult<MetadataMap> {
    let mut metadata = MetadataMap::with_capacity(parameters.len());
    for (name, value, _) in parameters
        .iter()
        .map(|parameter| expand_parameter(parameter, variables, "gRPC Metadata"))
        .collect::<DomainResult<Vec<ExpandedParameter>>>()?
    {
        if name.ends_with("-bin") {
            let key = BinaryMetadataKey::from_bytes(name.as_bytes())
                .map_err(|_| DomainError::InvalidConfig("gRPC Binary Metadata 名称无效".into()))?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(value.as_bytes())
                .map_err(|_| {
                    DomainError::InvalidConfig("gRPC Binary Metadata 值必须是 Base64".into())
                })?;
            metadata.append_bin(key, BinaryMetadataValue::from_bytes(&bytes));
        } else {
            let key = AsciiMetadataKey::from_bytes(name.as_bytes())
                .map_err(|_| DomainError::InvalidConfig("gRPC Metadata 名称无效".into()))?;
            let value = MetadataValue::try_from(value.as_str())
                .map_err(|_| DomainError::InvalidConfig("gRPC Metadata 值无效".into()))?;
            metadata.append(key, value);
        }
    }
    Ok(metadata)
}

/// 把 HTTP/gRPC 共用的认证配置转换为 gRPC Metadata；OAuth2 令牌只在本次调用的内存路径中出现。
pub(super) struct AuthMetadataContext<'a> {
    pub(super) variables: &'a std::collections::BTreeMap<String, String>,
    pub(super) oauth2: &'a OAuth2TokenProvider,
    pub(super) tls: &'a ApiTlsConfig,
    pub(super) proxy: &'a ApiProxyConfig,
    pub(super) timeout: Duration,
    pub(super) cancelled: ApiCancellation,
    pub(super) force_refresh: bool,
}

pub(super) async fn apply_auth_metadata(
    auth: &ApiAuth,
    metadata: &mut MetadataMap,
    context: AuthMetadataContext<'_>,
) -> DomainResult<()> {
    if matches!(auth, ApiAuth::None) {
        return Ok(());
    }
    if metadata.contains_key("authorization") {
        return Err(DomainError::InvalidConfig(
            "gRPC Metadata 不能与认证配置同时设置 authorization".into(),
        ));
    }
    let (name, value) = match auth {
        ApiAuth::None => return Ok(()),
        ApiAuth::Basic { username, password } => {
            let username = expand_template(
                username,
                context.variables,
                "Basic 用户名",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?;
            let password = expand_template(
                password,
                context.variables,
                "Basic 密码",
                MAX_API_PARAMETER_VALUE_BYTES,
            )?;
            use base64::Engine as _;
            let encoded =
                base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
            ("authorization".to_string(), format!("Basic {encoded}"))
        }
        ApiAuth::Bearer { token } => (
            "authorization".into(),
            format!(
                "Bearer {}",
                expand_template(
                    token,
                    context.variables,
                    "Bearer Token",
                    MAX_API_PARAMETER_VALUE_BYTES,
                )?
            ),
        ),
        ApiAuth::OAuth2 { config } => {
            let config = resolve_oauth2_config(config, context.variables)?;
            let token = context
                .oauth2
                .access_token(
                    &config,
                    context.tls,
                    context.proxy,
                    context.timeout,
                    context.cancelled,
                    context.force_refresh,
                )
                .await?;
            (
                "authorization".into(),
                format!("{} {}", token.scheme, token.value),
            )
        }
        ApiAuth::ApiKey {
            name,
            value,
            location,
        } => {
            if *location == ApiKeyLocation::Query {
                return Err(DomainError::InvalidConfig(
                    "gRPC API Key 只支持 Metadata Header 位置".into(),
                ));
            }
            (
                expand_template(
                    name,
                    context.variables,
                    "API Key 名称",
                    MAX_API_PARAMETER_VALUE_BYTES,
                )?,
                expand_template(
                    value,
                    context.variables,
                    "API Key 值",
                    MAX_API_PARAMETER_VALUE_BYTES,
                )?,
            )
        }
    };
    let key = AsciiMetadataKey::from_bytes(name.as_bytes())
        .map_err(|_| DomainError::InvalidConfig("gRPC 认证 Metadata 名称无效".into()))?;
    let value = MetadataValue::try_from(value.as_str())
        .map_err(|_| DomainError::InvalidConfig("gRPC 认证 Metadata 值无效".into()))?;
    metadata.insert(key, value);
    Ok(())
}

pub(super) fn success_snapshot(
    response: Response<DynamicMessage>,
    started: Instant,
) -> DomainResult<ApiResponseSnapshot> {
    let (metadata, message, _) = response.into_parts();
    let body = serde_json::to_vec(&message)
        .map_err(|_| DomainError::QueryFailed("gRPC 响应消息序列化失败".into()))?;
    let (body, size_bytes, truncated) = bounded_body(body);
    ApiResponseSnapshot::new(ApiResponseSnapshotParts {
        protocol: ApiProtocol::Grpc,
        status: ApiResponseStatus::Grpc { code: "ok".into() },
        headers: Vec::new(),
        metadata: response_metadata(&metadata)?,
        body,
        elapsed_millis: elapsed_millis(started),
        size_bytes,
        truncated,
        error: None,
    })
    .map_err(DomainError::InvalidConfig)
}

pub(super) fn status_snapshot(
    status: Status,
    started: Instant,
) -> DomainResult<ApiResponseSnapshot> {
    let body = status.message().as_bytes().to_vec();
    let (body, size_bytes, truncated) = bounded_body(body);
    ApiResponseSnapshot::new(ApiResponseSnapshotParts {
        protocol: ApiProtocol::Grpc,
        status: ApiResponseStatus::Grpc {
            code: format!("{:?}", status.code()),
        },
        headers: Vec::new(),
        metadata: response_metadata(status.metadata())?,
        body,
        elapsed_millis: elapsed_millis(started),
        size_bytes,
        truncated,
        error: None,
    })
    .map_err(DomainError::InvalidConfig)
}

pub(super) fn response_metadata(metadata: &MetadataMap) -> DomainResult<Vec<ApiParameter>> {
    if metadata.len() > MAX_API_PARAMETER_COUNT {
        return Err(DomainError::InvalidConfig(
            "gRPC 响应 Metadata 数量超过上限".into(),
        ));
    }
    metadata
        .iter()
        .map(|entry| match entry {
            KeyAndValueRef::Ascii(key, value) => Ok(ApiParameter::new(
                key.as_str(),
                value.to_str().unwrap_or_default(),
                is_sensitive_metadata(key.as_str()),
            )),
            KeyAndValueRef::Binary(key, value) => {
                let bytes = value.to_bytes().map_err(|_| {
                    DomainError::InvalidConfig("gRPC Binary Metadata 无法解码".into())
                })?;
                let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                Ok(ApiParameter::new(
                    key.as_str(),
                    encoded,
                    is_sensitive_metadata(key.as_str()),
                ))
            }
        })
        .collect()
}

fn is_sensitive_metadata(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "authorization"
        || name == "cookie"
        || name == "set-cookie"
        || name.contains("token")
        || name.contains("secret")
        || name.contains("password")
        || name.contains("api-key")
}

fn bounded_body(body: Vec<u8>) -> (Vec<u8>, u64, bool) {
    let size = body.len() as u64;
    let truncated = body.len() > MAX_API_RESPONSE_BODY_BYTES;
    let body = if truncated {
        body[..MAX_API_RESPONSE_BODY_BYTES].to_vec()
    } else {
        body
    };
    (body, size, truncated)
}

fn elapsed_millis(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

pub(super) fn ensure_not_cancelled(cancelled: &ApiCancellation) -> DomainResult<()> {
    check_not_cancelled(cancelled)
}

pub(super) fn ensure_tls_provider() -> DomainResult<()> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        return Err(DomainError::InvalidConfig(
            "无法安装 TLS 加密 Provider".into(),
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct NoCertificateVerification;

impl ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ED25519,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
        ]
    }
}
