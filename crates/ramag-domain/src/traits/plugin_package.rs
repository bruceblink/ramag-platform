//! 可安装插件包的清单、资源上限和入口路径校验。

use super::*;

/// 可安装插件清单的格式和字段上限。
pub const CURRENT_PLUGIN_PACKAGE_FORMAT_VERSION: u16 = 1;
pub const MAX_PLUGIN_PACKAGE_VERSION_BYTES: usize = 64;
pub const MAX_PLUGIN_PACKAGE_ENTRYPOINT_BYTES: usize = 256;
pub const MAX_PLUGIN_PACKAGE_SHA256_BYTES: usize = 64;

/// 第一阶段只允许进程外运行时；宿主暂不启动或加载外部代码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginPackageRuntime {
    Process,
}

/// 可安装插件包的元数据；安装器和后续进程宿主共享此接口约定。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPackageManifest {
    pub format_version: u16,
    pub id: PluginId,
    pub api_version: PluginApiVersion,
    pub name: String,
    pub version: String,
    pub runtime: PluginPackageRuntime,
    pub entrypoint: String,
    pub capabilities: Vec<PluginCapability>,
    pub package_sha256: String,
}

impl PluginPackageManifest {
    /// 创建一个待安装的进程插件清单；调用方仍需在保存前执行 `validate`。
    pub fn new(
        id: PluginId,
        name: impl Into<String>,
        version: impl Into<String>,
        entrypoint: impl Into<String>,
        package_sha256: impl Into<String>,
    ) -> Self {
        Self {
            format_version: CURRENT_PLUGIN_PACKAGE_FORMAT_VERSION,
            id,
            api_version: CURRENT_PLUGIN_API_VERSION,
            name: name.into(),
            version: version.into(),
            runtime: PluginPackageRuntime::Process,
            entrypoint: entrypoint.into(),
            capabilities: Vec::new(),
            package_sha256: package_sha256.into(),
        }
    }

    pub fn with_api_version(mut self, api_version: PluginApiVersion) -> Self {
        self.api_version = api_version;
        self
    }

    pub fn with_capabilities<I>(mut self, capabilities: I) -> Self
    where
        I: IntoIterator<Item = PluginCapability>,
    {
        self.capabilities = capabilities.into_iter().collect();
        self
    }

    /// 校验安装包身份、运行时入口、宿主 API、能力和包摘要。
    pub fn validate(&self) -> Result<(), PluginPackageManifestError> {
        if self.format_version != CURRENT_PLUGIN_PACKAGE_FORMAT_VERSION {
            return Err(PluginPackageManifestError::UnsupportedFormat {
                version: self.format_version,
            });
        }
        self.id.validate().map_err(PluginPackageManifestError::Id)?;
        if !self.api_version.is_supported_by(CURRENT_PLUGIN_API_VERSION) {
            return Err(PluginPackageManifestError::UnsupportedApiVersion {
                plugin: self.api_version,
                host: CURRENT_PLUGIN_API_VERSION,
            });
        }
        validate_text("插件包名称", &self.name, MAX_PLUGIN_NAME_BYTES, false)
            .map_err(PluginPackageManifestError::Field)?;
        semver::Version::parse(&self.version).map_err(|_| {
            PluginPackageManifestError::InvalidVersion {
                version: self.version.clone(),
            }
        })?;
        if self.version.len() > MAX_PLUGIN_PACKAGE_VERSION_BYTES {
            return Err(PluginPackageManifestError::FieldTooLong {
                field: "插件包版本",
                max_bytes: MAX_PLUGIN_PACKAGE_VERSION_BYTES,
            });
        }
        validate_relative_entrypoint(&self.entrypoint)?;
        if self.capabilities.len() > MAX_PLUGIN_CAPABILITIES {
            return Err(PluginPackageManifestError::TooManyCapabilities {
                max: MAX_PLUGIN_CAPABILITIES,
            });
        }
        let mut capabilities = HashSet::with_capacity(self.capabilities.len());
        for capability in &self.capabilities {
            if !capability.is_known() {
                return Err(PluginPackageManifestError::UnknownCapability {
                    capability: capability.as_str().to_owned(),
                });
            }
            if !capabilities.insert(capability) {
                return Err(PluginPackageManifestError::DuplicateCapability {
                    capability: capability.as_str().to_owned(),
                });
            }
        }
        if self.package_sha256.len() != MAX_PLUGIN_PACKAGE_SHA256_BYTES
            || !self
                .package_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || self
                .package_sha256
                .bytes()
                .any(|byte| byte.is_ascii_uppercase())
        {
            return Err(PluginPackageManifestError::InvalidSha256 {
                value: self.package_sha256.clone(),
            });
        }
        Ok(())
    }
}

/// 可安装清单错误；错误中保留字段原因，但不读取或执行插件包内容。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginPackageManifestError {
    #[error("插件包格式版本 {version} 不受当前宿主支持")]
    UnsupportedFormat { version: u16 },
    #[error("插件包 ID 无效：{0}")]
    Id(PluginRegistrationError),
    #[error("插件 API {plugin} 不受宿主 API {host} 支持")]
    UnsupportedApiVersion {
        plugin: PluginApiVersion,
        host: PluginApiVersion,
    },
    #[error("{0}")]
    Field(PluginRegistrationError),
    #[error("插件包版本不是有效的 SemVer：{version}")]
    InvalidVersion { version: String },
    #[error("{field}超过 {max_bytes} 字节")]
    FieldTooLong {
        field: &'static str,
        max_bytes: usize,
    },
    #[error("插件包入口路径无效：{entrypoint}")]
    InvalidEntrypoint { entrypoint: String },
    #[error("插件包能力超过 {max} 项")]
    TooManyCapabilities { max: usize },
    #[error("未知插件能力：{capability}")]
    UnknownCapability { capability: String },
    #[error("插件包能力重复：{capability}")]
    DuplicateCapability { capability: String },
    #[error("插件包 SHA-256 摘要无效：{value}")]
    InvalidSha256 { value: String },
}

fn validate_relative_entrypoint(entrypoint: &str) -> Result<(), PluginPackageManifestError> {
    if entrypoint.is_empty() || entrypoint.len() > MAX_PLUGIN_PACKAGE_ENTRYPOINT_BYTES {
        return Err(PluginPackageManifestError::InvalidEntrypoint {
            entrypoint: entrypoint.to_owned(),
        });
    }
    if entrypoint.starts_with('/')
        || entrypoint.contains('\\')
        || entrypoint.contains(':')
        || entrypoint.chars().any(char::is_control)
        || entrypoint
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(PluginPackageManifestError::InvalidEntrypoint {
            entrypoint: entrypoint.to_owned(),
        });
    }
    Ok(())
}
