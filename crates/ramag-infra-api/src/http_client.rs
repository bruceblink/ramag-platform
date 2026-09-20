//! HTTP 客户端构造和 TLS/代理材料装载。

use std::time::Duration;

use ramag_domain::entities::{ApiProxyConfig, ApiTlsConfig, ApiTlsVerify};
use ramag_domain::error::{DomainError, Result};
use reqwest::Client;

use super::read_tls_file;

/// 为一组 TLS 和显式代理配置创建独立客户端。
///
/// 客户端关闭自动重定向和环境代理，确保请求只经过调用方明确配置的代理；证书、私钥和
/// Basic 认证只进入 reqwest 内存对象，不写入日志或错误文本。证书文件仍由父模块按统一
/// 大小上限读取，避免这里产生另一套资源限制。
pub(super) fn build_client(tls: &ApiTlsConfig, proxy: &ApiProxyConfig) -> Result<Client> {
    proxy
        .validate_resolved()
        .map_err(DomainError::InvalidConfig)?;
    let mut builder = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .user_agent(concat!("Ramag/", env!("CARGO_PKG_VERSION")));
    if let Some(proxy_url) = proxy.url.as_deref() {
        let mut configured = reqwest::Proxy::all(proxy_url)
            .map_err(|_| DomainError::InvalidConfig("代理 URL 无效".into()))?;
        if let (Some(username), Some(password)) = (&proxy.username, &proxy.password) {
            configured = configured.basic_auth(username, password);
        }
        builder = builder.proxy(configured);
    }
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
