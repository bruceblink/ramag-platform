//! 已签名插件目录的来源信任和 Ed25519 校验。
//!
//! 旧版 `format_version = 1` 目录仍可解析和展示，但不会被标记为可信。
//! 只有外层 `format_version = 2` 的签名目录，并且签名 key 和来源 URL
//! 同时通过信任策略核验后，才允许后续下载或安装流程使用可信状态。

use std::collections::HashMap;

use base64::Engine;
use ring::signature::{self, UnparsedPublicKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::{
    MAX_PLUGIN_CATALOG_MANIFEST_BYTES, MAX_PLUGIN_CATALOG_URL_BYTES, PluginPackageCatalog,
    PluginPackageCatalogError,
};

/// 当前支持的已签名目录外层格式版本。
pub const CURRENT_PLUGIN_CATALOG_SIGNED_FORMAT_VERSION: u16 = 2;
/// 当前签名算法固定为 Ed25519，不接受运行时算法切换。
pub const PLUGIN_CATALOG_SIGNATURE_ALGORITHM: &str = "ed25519";
/// 应用内置发行 key 的标识。
pub const BUILTIN_PLUGIN_CATALOG_KEY_ID: &str = "ramag-release-2026-v1";
/// 应用内置发行目录的来源 URL 前缀。
pub const BUILTIN_PLUGIN_CATALOG_SOURCE_PREFIX: &str = "https://plugins.ramag.dev/";

const SIGNATURE_DOMAIN_SEPARATOR: &[u8] = b"ramag-plugin-catalog-v1\0";
const MAX_PLUGIN_CATALOG_KEY_ID_BYTES: usize = 64;
const MAX_PLUGIN_CATALOG_SIGNATURE_TEXT_BYTES: usize = 128;
const ED25519_PUBLIC_KEY_BYTES: usize = 32;
const ED25519_SIGNATURE_BYTES: usize = 64;

// 该公钥只作为应用内置的发行信任锚点。对应私钥不进入仓库、构建环境或测试代码。
const BUILTIN_PLUGIN_CATALOG_PUBLIC_KEY: [u8; ED25519_PUBLIC_KEY_BYTES] = [
    0x98, 0x8a, 0x46, 0xa6, 0x10, 0x78, 0xa7, 0x69, 0xf5, 0x3d, 0x63, 0xc0, 0x63, 0x8f, 0x92, 0x19,
    0x21, 0xe8, 0xc7, 0xae, 0x58, 0x92, 0xa3, 0xf9, 0x10, 0xf5, 0xcd, 0x2b, 0x73, 0xd6, 0xa3, 0xc8,
];

/// 已签名目录外层结构；`payload` 的 JSON 字节由固定 canonical 序列化规则签名。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginPackageCatalogSignedEnvelope {
    pub format_version: u16,
    pub key_id: String,
    pub payload: PluginPackageCatalog,
    /// URL-safe、无填充的 Ed25519 签名，解码后必须是 64 字节。
    pub signature: String,
}

/// 目录解析后的信任状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginPackageCatalogTrust {
    /// 兼容旧版无签名目录，只能展示，不能解锁可信下载或安装动作。
    LegacyUnsigned,
    /// 已通过内置或显式注入的可信 key 和来源 URL 校验。
    Verified { key_id: String },
}

impl PluginPackageCatalogTrust {
    /// 返回该目录是否可以作为可信来源进入后续动作。
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified { .. })
    }
}

/// 目录和信任结果的组合；调用方必须同时传递两者，避免丢失信任状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPackageCatalogDocument {
    pub catalog: PluginPackageCatalog,
    pub trust: PluginPackageCatalogTrust,
}

impl PluginPackageCatalogDocument {
    /// 解析旧版无签名目录或新版已签名目录。
    pub fn parse_json(
        bytes: &[u8],
        source_url: &str,
        trust_policy: &PluginPackageCatalogTrustPolicy,
    ) -> Result<Self, PluginPackageCatalogSignatureError> {
        if bytes.len() > MAX_PLUGIN_CATALOG_MANIFEST_BYTES {
            return Err(PluginPackageCatalogSignatureError::ManifestTooLarge {
                bytes: bytes.len(),
                max: MAX_PLUGIN_CATALOG_MANIFEST_BYTES,
            });
        }
        let version = serde_json::from_slice::<CatalogFormatProbe>(bytes)
            .map_err(PluginPackageCatalogSignatureError::Json)?
            .format_version;
        match version {
            crate::CURRENT_PLUGIN_CATALOG_FORMAT_VERSION => {
                let catalog = PluginPackageCatalog::parse_json(bytes)
                    .map_err(PluginPackageCatalogSignatureError::LegacyCatalog)?;
                Ok(Self {
                    catalog,
                    trust: PluginPackageCatalogTrust::LegacyUnsigned,
                })
            }
            CURRENT_PLUGIN_CATALOG_SIGNED_FORMAT_VERSION => {
                let envelope = serde_json::from_slice::<PluginPackageCatalogSignedEnvelope>(bytes)
                    .map_err(PluginPackageCatalogSignatureError::Json)?;
                verify_signed_envelope(envelope, source_url, trust_policy)
            }
            version => {
                Err(PluginPackageCatalogSignatureError::UnsupportedEnvelopeFormat { version })
            }
        }
    }

    /// 返回目录候选；信任状态由 `trust()` 单独读取。
    pub fn entries(&self) -> &[crate::PluginPackageCatalogEntry] {
        self.catalog.entries()
    }

    /// 返回来源信任状态。
    pub fn trust(&self) -> &PluginPackageCatalogTrust {
        &self.trust
    }
}

/// 一个可用于签名核验的可信 key 和来源路径绑定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPackageCatalogTrustedKey {
    key_id: String,
    public_key: [u8; ED25519_PUBLIC_KEY_BYTES],
    source_prefix: String,
}

impl PluginPackageCatalogTrustedKey {
    /// 创建可信 key；公钥必须是原始 32 字节 Ed25519 公钥，来源前缀必须是 HTTPS。
    pub fn new(
        key_id: impl Into<String>,
        public_key: impl AsRef<[u8]>,
        source_prefix: impl Into<String>,
    ) -> Result<Self, PluginPackageCatalogSignatureError> {
        let key_id = key_id.into();
        validate_key_id(&key_id)?;
        let public_key = public_key.as_ref();
        if public_key.len() != ED25519_PUBLIC_KEY_BYTES {
            return Err(PluginPackageCatalogSignatureError::InvalidPublicKey);
        }
        let mut key = [0_u8; ED25519_PUBLIC_KEY_BYTES];
        key.copy_from_slice(public_key);
        let source_prefix = source_prefix.into();
        validate_source_prefix(&source_prefix)?;
        Ok(Self {
            key_id,
            public_key: key,
            source_prefix,
        })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn public_key(&self) -> &[u8; ED25519_PUBLIC_KEY_BYTES] {
        &self.public_key
    }

    pub fn source_prefix(&self) -> &str {
        &self.source_prefix
    }
}

/// 目录签名核验策略；key 只能由应用内置配置或明确注入的本地策略提供。
#[derive(Debug, Clone, Default)]
pub struct PluginPackageCatalogTrustPolicy {
    keys: HashMap<String, PluginPackageCatalogTrustedKey>,
}

impl PluginPackageCatalogTrustPolicy {
    /// 创建 key 集合；重复 key ID 直接拒绝，避免来源策略被后写值覆盖。
    pub fn new<I>(keys: I) -> Result<Self, PluginPackageCatalogSignatureError>
    where
        I: IntoIterator<Item = PluginPackageCatalogTrustedKey>,
    {
        let mut trusted = HashMap::new();
        for key in keys {
            if trusted.insert(key.key_id.clone(), key).is_some() {
                return Err(PluginPackageCatalogSignatureError::DuplicateTrustedKey);
            }
        }
        Ok(Self { keys: trusted })
    }

    /// 返回应用内置发行信任策略。
    pub fn builtin() -> Self {
        let key = PluginPackageCatalogTrustedKey {
            key_id: BUILTIN_PLUGIN_CATALOG_KEY_ID.to_owned(),
            public_key: BUILTIN_PLUGIN_CATALOG_PUBLIC_KEY,
            source_prefix: BUILTIN_PLUGIN_CATALOG_SOURCE_PREFIX.to_owned(),
        };
        Self {
            keys: HashMap::from([(key.key_id.clone(), key)]),
        }
    }

    /// 按 key ID 查询可信 key，供诊断和策略检查使用。
    pub fn trusted_key(&self, key_id: &str) -> Option<&PluginPackageCatalogTrustedKey> {
        self.keys.get(key_id)
    }
}

/// 生成签名输入：固定域分隔符加 payload 的结构化 JSON 字节。
pub fn canonical_plugin_catalog_signature_message(
    catalog: &PluginPackageCatalog,
) -> Result<Vec<u8>, PluginPackageCatalogSignatureError> {
    catalog
        .validate()
        .map_err(PluginPackageCatalogSignatureError::Payload)?;
    let payload = serde_json::to_vec(catalog)
        .map_err(PluginPackageCatalogSignatureError::CanonicalSerialization)?;
    let mut message = Vec::with_capacity(SIGNATURE_DOMAIN_SEPARATOR.len() + payload.len());
    message.extend_from_slice(SIGNATURE_DOMAIN_SEPARATOR);
    message.extend_from_slice(&payload);
    Ok(message)
}

fn verify_signed_envelope(
    envelope: PluginPackageCatalogSignedEnvelope,
    source_url: &str,
    trust_policy: &PluginPackageCatalogTrustPolicy,
) -> Result<PluginPackageCatalogDocument, PluginPackageCatalogSignatureError> {
    if envelope.format_version != CURRENT_PLUGIN_CATALOG_SIGNED_FORMAT_VERSION {
        return Err(
            PluginPackageCatalogSignatureError::UnsupportedEnvelopeFormat {
                version: envelope.format_version,
            },
        );
    }
    validate_key_id(&envelope.key_id)?;
    if envelope.signature.len() > MAX_PLUGIN_CATALOG_SIGNATURE_TEXT_BYTES {
        return Err(PluginPackageCatalogSignatureError::InvalidSignatureEncoding);
    }
    let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(envelope.signature.as_bytes())
        .map_err(|_| PluginPackageCatalogSignatureError::InvalidSignatureEncoding)?;
    if signature.len() != ED25519_SIGNATURE_BYTES {
        return Err(PluginPackageCatalogSignatureError::InvalidSignatureEncoding);
    }
    let trusted_key = trust_policy.trusted_key(&envelope.key_id).ok_or_else(|| {
        PluginPackageCatalogSignatureError::UnknownKey {
            key_id: envelope.key_id.clone(),
        }
    })?;
    if !source_matches_prefix(source_url, trusted_key.source_prefix()) {
        return Err(PluginPackageCatalogSignatureError::UntrustedSource);
    }
    let message = canonical_plugin_catalog_signature_message(&envelope.payload)?;
    UnparsedPublicKey::new(&signature::ED25519, trusted_key.public_key())
        .verify(&message, &signature)
        .map_err(|_| PluginPackageCatalogSignatureError::InvalidSignature)?;
    Ok(PluginPackageCatalogDocument {
        catalog: envelope.payload,
        trust: PluginPackageCatalogTrust::Verified {
            key_id: envelope.key_id,
        },
    })
}

fn validate_key_id(key_id: &str) -> Result<(), PluginPackageCatalogSignatureError> {
    if key_id.is_empty()
        || key_id.len() > MAX_PLUGIN_CATALOG_KEY_ID_BYTES
        || key_id.chars().any(char::is_control)
    {
        return Err(PluginPackageCatalogSignatureError::InvalidKeyId);
    }
    Ok(())
}

fn validate_source_prefix(source_prefix: &str) -> Result<(), PluginPackageCatalogSignatureError> {
    if source_prefix.is_empty()
        || source_prefix.len() > MAX_PLUGIN_CATALOG_URL_BYTES
        || !source_prefix.ends_with('/')
    {
        return Err(PluginPackageCatalogSignatureError::InvalidSourcePrefix);
    }
    let parsed = Url::parse(source_prefix)
        .map_err(|_| PluginPackageCatalogSignatureError::InvalidSourcePrefix)?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(PluginPackageCatalogSignatureError::InvalidSourcePrefix);
    }
    Ok(())
}

fn source_matches_prefix(source_url: &str, source_prefix: &str) -> bool {
    let Ok(source) = Url::parse(source_url) else {
        return false;
    };
    let Ok(prefix) = Url::parse(source_prefix) else {
        return false;
    };
    source.scheme() == prefix.scheme()
        && source.host() == prefix.host()
        && source.port_or_known_default() == prefix.port_or_known_default()
        && source.username().is_empty()
        && source.password().is_none()
        && source.query().is_none()
        && source.fragment().is_none()
        && source.path().starts_with(prefix.path())
}

#[derive(Debug, Deserialize)]
struct CatalogFormatProbe {
    format_version: u16,
}

#[derive(Debug, Error)]
pub enum PluginPackageCatalogSignatureError {
    #[error("插件目录清单超过 {max} 字节（实际 {bytes} 字节）")]
    ManifestTooLarge { bytes: usize, max: usize },
    #[error("插件目录签名 JSON 无法解析：{0}")]
    Json(serde_json::Error),
    #[error("插件目录外层格式版本 {version} 不受支持")]
    UnsupportedEnvelopeFormat { version: u16 },
    #[error("旧版插件目录清单无效：{0}")]
    LegacyCatalog(PluginPackageCatalogError),
    #[error("插件目录签名 payload 无效：{0}")]
    Payload(PluginPackageCatalogError),
    #[error("插件目录签名 payload 无法序列化：{0}")]
    CanonicalSerialization(serde_json::Error),
    #[error("插件目录签名 key ID 无效")]
    InvalidKeyId,
    #[error("插件目录签名公钥长度无效")]
    InvalidPublicKey,
    #[error("插件目录来源前缀无效")]
    InvalidSourcePrefix,
    #[error("插件目录签名 key ID 重复")]
    DuplicateTrustedKey,
    #[error("插件目录签名 key 未知：{key_id}")]
    UnknownKey { key_id: String },
    #[error("插件目录来源不在可信 key 的绑定范围内")]
    UntrustedSource,
    #[error("插件目录签名编码无效")]
    InvalidSignatureEncoding,
    #[error("插件目录签名校验失败")]
    InvalidSignature,
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use ramag_domain::{PluginApiVersion, PluginCapability, PluginId, PluginPackageRuntime};
    use ring::signature::{Ed25519KeyPair, KeyPair};

    const TEST_SEED: [u8; 32] = [7; 32];

    fn catalog() -> PluginPackageCatalog {
        PluginPackageCatalog {
            format_version: crate::CURRENT_PLUGIN_CATALOG_FORMAT_VERSION,
            entries: vec![crate::PluginPackageCatalogEntry {
                id: PluginId::new("signed.example").expect("valid ID"),
                name: "签名插件".into(),
                version: "1.2.3".into(),
                api_version: PluginApiVersion::new(1, 0),
                runtime: PluginPackageRuntime::Process,
                package_url: "https://plugins.example.test/packages/signed.tar.gz".into(),
                archive_sha256: "a".repeat(crate::PLUGIN_PACKAGE_SHA256_HEX_BYTES),
                capabilities: vec![PluginCapability::new("ui.entry")],
            }],
        }
    }

    fn key_pair() -> Ed25519KeyPair {
        Ed25519KeyPair::from_seed_unchecked(&TEST_SEED).expect("test key pair")
    }

    fn policy(pair: &Ed25519KeyPair) -> PluginPackageCatalogTrustPolicy {
        PluginPackageCatalogTrustPolicy::new([PluginPackageCatalogTrustedKey::new(
            "test-key",
            pair.public_key().as_ref(),
            "https://plugins.example.test/",
        )
        .expect("test trusted key")])
        .expect("test trust policy")
    }

    fn signed_bytes(pair: &Ed25519KeyPair, catalog: &PluginPackageCatalog) -> Vec<u8> {
        let message = canonical_plugin_catalog_signature_message(catalog).expect("canonical bytes");
        let envelope = PluginPackageCatalogSignedEnvelope {
            format_version: CURRENT_PLUGIN_CATALOG_SIGNED_FORMAT_VERSION,
            key_id: "test-key".into(),
            payload: catalog.clone(),
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(pair.sign(&message).as_ref()),
        };
        serde_json::to_vec(&envelope).expect("signed envelope JSON")
    }

    #[test]
    fn verifies_signed_catalog_and_source_binding() {
        let pair = key_pair();
        let bytes = signed_bytes(&pair, &catalog());
        let document = PluginPackageCatalogDocument::parse_json(
            &bytes,
            "https://plugins.example.test/catalog.json",
            &policy(&pair),
        )
        .expect("valid signed catalog");
        assert_eq!(document.entries().len(), 1);
        assert_eq!(
            document.trust(),
            &PluginPackageCatalogTrust::Verified {
                key_id: "test-key".into()
            }
        );
        assert!(document.trust().is_verified());
    }

    #[test]
    fn legacy_catalog_is_parseable_but_unverified() {
        let catalog = catalog();
        let bytes = serde_json::to_vec(&catalog).expect("legacy catalog JSON");
        let pair = key_pair();
        let document = PluginPackageCatalogDocument::parse_json(
            &bytes,
            "https://plugins.example.test/catalog.json",
            &policy(&pair),
        )
        .expect("legacy catalog");
        assert_eq!(document.trust(), &PluginPackageCatalogTrust::LegacyUnsigned);
        assert!(!document.trust().is_verified());
    }

    #[test]
    fn rejects_tampered_payload_unknown_key_and_untrusted_source() {
        let pair = key_pair();
        let mut value: serde_json::Value =
            serde_json::from_slice(&signed_bytes(&pair, &catalog())).expect("signed JSON");
        value["payload"]["entries"][0]["name"] = serde_json::Value::String("篡改".into());
        let tampered = serde_json::to_vec(&value).expect("tampered JSON");
        assert!(matches!(
            PluginPackageCatalogDocument::parse_json(
                &tampered,
                "https://plugins.example.test/catalog.json",
                &policy(&pair),
            ),
            Err(PluginPackageCatalogSignatureError::InvalidSignature)
        ));

        let unknown_policy = PluginPackageCatalogTrustPolicy::default();
        let bytes = signed_bytes(&pair, &catalog());
        assert!(matches!(
            PluginPackageCatalogDocument::parse_json(
                &bytes,
                "https://plugins.example.test/catalog.json",
                &unknown_policy,
            ),
            Err(PluginPackageCatalogSignatureError::UnknownKey { .. })
        ));
        assert!(matches!(
            PluginPackageCatalogDocument::parse_json(
                &bytes,
                "https://other.example.test/catalog.json",
                &policy(&pair),
            ),
            Err(PluginPackageCatalogSignatureError::UntrustedSource)
        ));
    }

    #[test]
    fn rejects_invalid_signature_encoding_and_duplicate_keys() {
        let pair = key_pair();
        let mut value: serde_json::Value =
            serde_json::from_slice(&signed_bytes(&pair, &catalog())).expect("signed JSON");
        value["signature"] = serde_json::Value::String("%%%".into());
        let invalid = serde_json::to_vec(&value).expect("invalid signature JSON");
        assert!(matches!(
            PluginPackageCatalogDocument::parse_json(
                &invalid,
                "https://plugins.example.test/catalog.json",
                &policy(&pair),
            ),
            Err(PluginPackageCatalogSignatureError::InvalidSignatureEncoding)
        ));

        let trusted = PluginPackageCatalogTrustedKey::new(
            "duplicate",
            pair.public_key().as_ref(),
            "https://plugins.example.test/",
        )
        .expect("trusted key");
        assert!(matches!(
            PluginPackageCatalogTrustPolicy::new([trusted.clone(), trusted]),
            Err(PluginPackageCatalogSignatureError::DuplicateTrustedKey)
        ));
    }

    #[test]
    fn builtin_policy_has_a_source_bound_key() {
        let policy = PluginPackageCatalogTrustPolicy::builtin();
        let key = policy
            .trusted_key(BUILTIN_PLUGIN_CATALOG_KEY_ID)
            .expect("builtin key");
        assert_eq!(key.source_prefix(), BUILTIN_PLUGIN_CATALOG_SOURCE_PREFIX);
        assert_eq!(key.public_key().len(), ED25519_PUBLIC_KEY_BYTES);
    }

    #[test]
    fn source_prefix_requires_https_and_trailing_path_boundary() {
        let pair = key_pair();
        assert!(matches!(
            PluginPackageCatalogTrustedKey::new(
                "bad",
                pair.public_key().as_ref(),
                "http://plugins.example.test/",
            ),
            Err(PluginPackageCatalogSignatureError::InvalidSourcePrefix)
        ));
        assert!(matches!(
            PluginPackageCatalogTrustedKey::new(
                "bad",
                pair.public_key().as_ref(),
                "https://plugins.example.test/catalog.json",
            ),
            Err(PluginPackageCatalogSignatureError::InvalidSourcePrefix)
        ));
        assert!(!source_matches_prefix(
            "https://plugins.example.test/catalog-extra.json",
            "https://plugins.example.test/catalog/",
        ));
    }
}
