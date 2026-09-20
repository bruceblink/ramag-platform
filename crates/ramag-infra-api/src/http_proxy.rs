//! HTTP/gRPC 共用的代理模板展开逻辑。

use std::collections::BTreeMap;

use ramag_domain::entities::{
    ApiProxyConfig, MAX_API_PROXY_CREDENTIAL_BYTES, MAX_API_PROXY_URL_BYTES,
};
use ramag_domain::error::{DomainError, Result};

use super::expand_template;

/// 展开代理配置并立即校验结果，确保传输层不会使用未经约束的环境变量内容。
///
/// 代理认证字段在请求执行副本中展开，但不会写入错误文本或响应历史；展开后的 URL 和
/// 认证成对关系会再次经过领域校验，避免模板变量把不安全的 scheme、路径或内嵌凭据带入
/// reqwest 和 tonic 的连接器。
pub(crate) fn resolve_proxy_config(
    proxy: &ApiProxyConfig,
    variables: &BTreeMap<String, String>,
) -> Result<ApiProxyConfig> {
    let resolved = ApiProxyConfig {
        url: proxy
            .url
            .as_ref()
            .map(|value| expand_template(value, variables, "代理 URL", MAX_API_PROXY_URL_BYTES))
            .transpose()?,
        username: proxy
            .username
            .as_ref()
            .map(|value| {
                expand_template(
                    value,
                    variables,
                    "代理用户名",
                    MAX_API_PROXY_CREDENTIAL_BYTES,
                )
            })
            .transpose()?,
        password: proxy
            .password
            .as_ref()
            .map(|value| {
                expand_template(value, variables, "代理密码", MAX_API_PROXY_CREDENTIAL_BYTES)
            })
            .transpose()?,
    };
    resolved
        .validate_resolved()
        .map_err(DomainError::InvalidConfig)?;
    Ok(resolved)
}
