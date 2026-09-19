//! gRPC Endpoint、TLS、Metadata 和响应快照适配。

use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use http::Uri;
use prost_reflect::DynamicMessage;
use ramag_domain::entities::{
    ApiCancellation, ApiParameter, ApiProtocol, ApiResponseSnapshot, ApiResponseSnapshotParts,
    ApiResponseStatus, ApiTlsConfig, ApiTlsVerify, MAX_API_PARAMETER_COUNT,
    MAX_API_RESPONSE_BODY_BYTES,
};
use ramag_domain::error::{DomainError, Result as DomainResult};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use tonic::metadata::{
    AsciiMetadataKey, BinaryMetadataKey, BinaryMetadataValue, KeyAndValueRef, MetadataMap,
    MetadataValue,
};
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};
use tonic::{Response, Status};

use crate::http::{
    ExpandedParameter, ensure_not_cancelled as check_not_cancelled, expand_parameter, read_tls_file,
};

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
    cancelled: ApiCancellation,
) -> DomainResult<Channel> {
    tokio::select! {
        result = endpoint.connect() => result.map_err(|_| DomainError::ConnectionFailed("gRPC 连接失败".into())),
        _ = crate::http::wait_until_cancelled(cancelled) => Err(DomainError::Cancelled("gRPC 连接已取消".into())),
    }
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

fn response_metadata(metadata: &MetadataMap) -> DomainResult<Vec<ApiParameter>> {
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
