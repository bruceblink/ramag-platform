//! 本地插件包目录清单的解析和校验；不联网、不下载、不解包、不执行插件代码。

use std::collections::HashSet;

use ramag_domain::{
    PluginApiVersion, PluginCapability, PluginId, PluginPackageRuntime, PluginRegistrationError,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

/// 当前支持的插件包目录清单格式版本。
pub const CURRENT_PLUGIN_CATALOG_FORMAT_VERSION: u16 = 1;
/// 目录清单 JSON 的最大字节数。
pub const MAX_PLUGIN_CATALOG_MANIFEST_BYTES: usize = 1024 * 1024;
/// 单个目录清单最多包含的包条目数。
pub const MAX_PLUGIN_CATALOG_PACKAGE_ENTRIES: usize = 256;
/// 目录清单中的显示名称最大字节数。
pub const MAX_PLUGIN_CATALOG_NAME_BYTES: usize = 128;
/// 目录清单中的下载地址最大字节数。
pub const MAX_PLUGIN_CATALOG_URL_BYTES: usize = 2048;
/// 目录清单中的包版本最大字节数。
pub const MAX_PLUGIN_CATALOG_VERSION_BYTES: usize = 64;
/// 目录清单中的 SHA-256 摘要长度。
pub const PLUGIN_PACKAGE_SHA256_HEX_BYTES: usize = 64;

const HOST_PLUGIN_API_VERSION: PluginApiVersion = PluginApiVersion::new(1, 0);

/// 可序列化的本地插件包目录清单；只保存候选包元数据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginPackageCatalog {
    pub format_version: u16,
    pub entries: Vec<PluginPackageCatalogEntry>,
}

impl PluginPackageCatalog {
    /// 从受限 JSON 字节解析目录清单，并在返回前校验全部条目。
    pub fn parse_json(bytes: &[u8]) -> Result<Self, PluginPackageCatalogError> {
        if bytes.len() > MAX_PLUGIN_CATALOG_MANIFEST_BYTES {
            return Err(PluginPackageCatalogError::ManifestTooLarge {
                bytes: bytes.len(),
                max: MAX_PLUGIN_CATALOG_MANIFEST_BYTES,
            });
        }
        let catalog =
            serde_json::from_slice::<Self>(bytes).map_err(PluginPackageCatalogError::Json)?;
        catalog.validate()?;
        Ok(catalog)
    }

    /// 校验目录格式、条目边界、下载地址和包元数据。
    pub fn validate(&self) -> Result<(), PluginPackageCatalogError> {
        if self.format_version != CURRENT_PLUGIN_CATALOG_FORMAT_VERSION {
            return Err(PluginPackageCatalogError::UnsupportedFormat {
                version: self.format_version,
            });
        }
        if self.entries.len() > MAX_PLUGIN_CATALOG_PACKAGE_ENTRIES {
            return Err(PluginPackageCatalogError::TooManyEntries {
                max: MAX_PLUGIN_CATALOG_PACKAGE_ENTRIES,
            });
        }
        let mut identities = HashSet::with_capacity(self.entries.len());
        for (index, entry) in self.entries.iter().enumerate() {
            entry.validate_at(index)?;
            if !identities.insert((entry.id.clone(), entry.version.clone())) {
                return Err(PluginPackageCatalogError::DuplicateEntry {
                    plugin_id: entry.id.clone(),
                    version: entry.version.clone(),
                });
            }
        }
        Ok(())
    }

    /// 返回目录候选的只读切片；调用方仍需自行决定是否展示或安装。
    pub fn entries(&self) -> &[PluginPackageCatalogEntry] {
        &self.entries
    }
}

/// 一个可供本地安装器后续使用的插件包候选；本结构不执行下载。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginPackageCatalogEntry {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub api_version: PluginApiVersion,
    pub runtime: PluginPackageRuntime,
    pub package_url: String,
    pub package_sha256: String,
    pub capabilities: Vec<PluginCapability>,
}

impl PluginPackageCatalogEntry {
    /// 校验单个候选，供下载器在执行外部输入前复用。
    pub fn validate(&self) -> Result<(), PluginPackageCatalogError> {
        self.validate_at(0)
    }

    fn validate_at(&self, index: usize) -> Result<(), PluginPackageCatalogError> {
        self.id
            .validate()
            .map_err(|source| PluginPackageCatalogError::EntryId { index, source })?;
        validate_text(
            &self.name,
            MAX_PLUGIN_CATALOG_NAME_BYTES,
            PluginPackageCatalogError::InvalidName { index },
        )?;
        if !self.api_version.is_supported_by(HOST_PLUGIN_API_VERSION) {
            return Err(PluginPackageCatalogError::UnsupportedApi {
                index,
                catalog: self.api_version,
                host: HOST_PLUGIN_API_VERSION,
            });
        }
        if self.runtime != PluginPackageRuntime::Process {
            return Err(PluginPackageCatalogError::UnsupportedRuntime { index });
        }
        if self.version.len() > MAX_PLUGIN_CATALOG_VERSION_BYTES
            || semver::Version::parse(&self.version).is_err()
        {
            return Err(PluginPackageCatalogError::InvalidVersion {
                index,
                version: self.version.clone(),
            });
        }
        validate_package_url(&self.package_url, index)?;
        validate_sha256(&self.package_sha256, index)?;
        let mut capabilities = HashSet::with_capacity(self.capabilities.len());
        for capability in &self.capabilities {
            if !capability.is_known() {
                return Err(PluginPackageCatalogError::UnknownCapability {
                    index,
                    capability: capability.as_str().to_owned(),
                });
            }
            if !capabilities.insert(capability) {
                return Err(PluginPackageCatalogError::DuplicateCapability {
                    index,
                    capability: capability.as_str().to_owned(),
                });
            }
        }
        Ok(())
    }
}

fn validate_text<E>(value: &str, max_bytes: usize, error: E) -> Result<(), E> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(error);
    }
    Ok(())
}

fn validate_package_url(value: &str, index: usize) -> Result<(), PluginPackageCatalogError> {
    if value.len() > MAX_PLUGIN_CATALOG_URL_BYTES {
        return Err(PluginPackageCatalogError::InvalidUrl {
            index,
            url: value.to_owned(),
        });
    }
    let parsed = Url::parse(value).map_err(|_| PluginPackageCatalogError::InvalidUrl {
        index,
        url: value.to_owned(),
    })?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(PluginPackageCatalogError::InvalidUrl {
            index,
            url: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_sha256(value: &str, index: usize) -> Result<(), PluginPackageCatalogError> {
    if value.len() != PLUGIN_PACKAGE_SHA256_HEX_BYTES
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        || value.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(PluginPackageCatalogError::InvalidSha256 {
            index,
            value: value.to_owned(),
        });
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum PluginPackageCatalogError {
    #[error("插件目录清单超过 {max} 字节（实际 {bytes} 字节）")]
    ManifestTooLarge { bytes: usize, max: usize },
    #[error("插件目录清单 JSON 无法解析：{0}")]
    Json(serde_json::Error),
    #[error("插件目录格式版本 {version} 不受支持")]
    UnsupportedFormat { version: u16 },
    #[error("插件目录条目超过 {max} 项")]
    TooManyEntries { max: usize },
    #[error("插件目录条目重复：插件 `{plugin_id}` / 版本 `{version}`")]
    DuplicateEntry {
        plugin_id: PluginId,
        version: String,
    },
    #[error("插件目录第 {index} 项 ID 无效：{source}")]
    EntryId {
        index: usize,
        source: PluginRegistrationError,
    },
    #[error("插件目录第 {index} 项名称无效")]
    InvalidName { index: usize },
    #[error("插件目录第 {index} 项 API {catalog} 不受宿主 API {host} 支持")]
    UnsupportedApi {
        index: usize,
        catalog: PluginApiVersion,
        host: PluginApiVersion,
    },
    #[error("插件目录第 {index} 项运行时不受支持")]
    UnsupportedRuntime { index: usize },
    #[error("插件目录第 {index} 项版本无效：{version}")]
    InvalidVersion { index: usize, version: String },
    #[error("插件目录第 {index} 项下载地址无效：{url}")]
    InvalidUrl { index: usize, url: String },
    #[error("插件目录第 {index} 项 SHA-256 无效：{value}")]
    InvalidSha256 { index: usize, value: String },
    #[error("插件目录第 {index} 项声明未知能力：{capability}")]
    UnknownCapability { index: usize, capability: String },
    #[error("插件目录第 {index} 项能力重复：{capability}")]
    DuplicateCapability { index: usize, capability: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::PluginCapability;

    fn entry(id: &str, version: &str) -> PluginPackageCatalogEntry {
        PluginPackageCatalogEntry {
            id: PluginId::new(id).expect("valid ID"),
            name: "示例插件".into(),
            version: version.into(),
            api_version: HOST_PLUGIN_API_VERSION,
            runtime: PluginPackageRuntime::Process,
            package_url: "https://plugins.example.test/example.tar.gz".into(),
            package_sha256: "a".repeat(PLUGIN_PACKAGE_SHA256_HEX_BYTES),
            capabilities: vec![PluginCapability::new("ui.entry")],
        }
    }

    fn json(catalog: &PluginPackageCatalog) -> Vec<u8> {
        serde_json::to_vec(catalog).expect("catalog JSON")
    }

    #[test]
    fn parses_valid_catalog_and_round_trips() {
        let catalog = PluginPackageCatalog {
            format_version: CURRENT_PLUGIN_CATALOG_FORMAT_VERSION,
            entries: vec![entry("catalog.example", "1.2.3")],
        };
        let parsed = PluginPackageCatalog::parse_json(&json(&catalog)).expect("valid catalog");
        assert_eq!(parsed, catalog);
        assert_eq!(parsed.entries().len(), 1);
    }

    #[test]
    fn rejects_duplicate_entries_and_unsupported_format() {
        let duplicate = PluginPackageCatalog {
            format_version: CURRENT_PLUGIN_CATALOG_FORMAT_VERSION,
            entries: vec![
                entry("catalog.example", "1.0.0"),
                entry("catalog.example", "1.0.0"),
            ],
        };
        assert!(matches!(
            PluginPackageCatalog::parse_json(&json(&duplicate)),
            Err(PluginPackageCatalogError::DuplicateEntry { .. })
        ));
        let unsupported = PluginPackageCatalog {
            format_version: CURRENT_PLUGIN_CATALOG_FORMAT_VERSION + 1,
            entries: Vec::new(),
        };
        assert!(matches!(
            PluginPackageCatalog::parse_json(&json(&unsupported)),
            Err(PluginPackageCatalogError::UnsupportedFormat { .. })
        ));
    }

    #[test]
    fn rejects_bad_urls_versions_and_digests() {
        let mut bad_url = entry("catalog.url", "1.0.0");
        bad_url.package_url = "http://plugins.example.test/example.tar.gz".into();
        assert!(matches!(
            PluginPackageCatalog::parse_json(&json(&PluginPackageCatalog {
                format_version: CURRENT_PLUGIN_CATALOG_FORMAT_VERSION,
                entries: vec![bad_url],
            })),
            Err(PluginPackageCatalogError::InvalidUrl { .. })
        ));
        let mut bad_version = entry("catalog.version", "latest");
        bad_version.package_url = "https://plugins.example.test/version.tar.gz".into();
        assert!(matches!(
            PluginPackageCatalog::parse_json(&json(&PluginPackageCatalog {
                format_version: CURRENT_PLUGIN_CATALOG_FORMAT_VERSION,
                entries: vec![bad_version],
            })),
            Err(PluginPackageCatalogError::InvalidVersion { .. })
        ));
        let mut bad_digest = entry("catalog.digest", "1.0.0");
        bad_digest.package_sha256 = "A".repeat(PLUGIN_PACKAGE_SHA256_HEX_BYTES);
        assert!(matches!(
            PluginPackageCatalog::parse_json(&json(&PluginPackageCatalog {
                format_version: CURRENT_PLUGIN_CATALOG_FORMAT_VERSION,
                entries: vec![bad_digest],
            })),
            Err(PluginPackageCatalogError::InvalidSha256 { .. })
        ));
    }

    #[test]
    fn rejects_incompatible_api_unknown_or_duplicate_capabilities() {
        let mut bad_api = entry("catalog.api", "1.0.0");
        bad_api.api_version = PluginApiVersion::new(2, 0);
        assert!(matches!(
            PluginPackageCatalog::parse_json(&json(&PluginPackageCatalog {
                format_version: CURRENT_PLUGIN_CATALOG_FORMAT_VERSION,
                entries: vec![bad_api],
            })),
            Err(PluginPackageCatalogError::UnsupportedApi { .. })
        ));
        let mut unknown = entry("catalog.capability", "1.0.0");
        unknown.capabilities = vec![PluginCapability::new("network.any")];
        assert!(matches!(
            PluginPackageCatalog::parse_json(&json(&PluginPackageCatalog {
                format_version: CURRENT_PLUGIN_CATALOG_FORMAT_VERSION,
                entries: vec![unknown],
            })),
            Err(PluginPackageCatalogError::UnknownCapability { .. })
        ));
        let mut duplicate = entry("catalog.duplicate-capability", "1.0.0");
        duplicate.capabilities = vec![
            PluginCapability::new("ui.entry"),
            PluginCapability::new("ui.entry"),
        ];
        assert!(matches!(
            PluginPackageCatalog::parse_json(&json(&PluginPackageCatalog {
                format_version: CURRENT_PLUGIN_CATALOG_FORMAT_VERSION,
                entries: vec![duplicate],
            })),
            Err(PluginPackageCatalogError::DuplicateCapability { .. })
        ));
    }

    #[test]
    fn rejects_catalog_resource_limits() {
        let entries = (0..=MAX_PLUGIN_CATALOG_PACKAGE_ENTRIES)
            .map(|index| entry(&format!("catalog.{index}"), "1.0.0"))
            .collect();
        assert!(matches!(
            PluginPackageCatalog::parse_json(&json(&PluginPackageCatalog {
                format_version: CURRENT_PLUGIN_CATALOG_FORMAT_VERSION,
                entries,
            })),
            Err(PluginPackageCatalogError::TooManyEntries { .. })
        ));
        let oversized = vec![b' '; MAX_PLUGIN_CATALOG_MANIFEST_BYTES + 1];
        assert!(matches!(
            PluginPackageCatalog::parse_json(&oversized),
            Err(PluginPackageCatalogError::ManifestTooLarge { .. })
        ));
        assert!(matches!(
            PluginPackageCatalog::parse_json(br#"{"format_version":1,"entries":[],"extra":true}"#),
            Err(PluginPackageCatalogError::Json(_))
        ));
    }
}
