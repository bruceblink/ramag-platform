use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use ramag_domain::traits::CURRENT_PLUGIN_API_VERSION;
use ramag_domain::{PluginCapability, PluginPackageManifest};
use tokio::process::Command;

use super::{PluginProcessError, PluginProcessMessage, PluginResponse};

pub(super) fn verify_package_digest(
    package_root: &Path,
    manifest: &PluginPackageManifest,
) -> Result<(), PluginProcessError> {
    let actual =
        crate::plugin_package_installer::package_digest(package_root).map_err(|error| {
            PluginProcessError::PackageVerification {
                message: error.to_string(),
            }
        })?;
    if actual != manifest.package_sha256 {
        return Err(PluginProcessError::PackageDigestMismatch {
            expected: manifest.package_sha256.clone(),
            actual,
        });
    }
    Ok(())
}

/// 清空继承环境，只保留启动插件所需的搜索路径和宿主元数据。
pub(super) fn configure_plugin_environment(
    command: &mut Command,
    _manifest: &PluginPackageManifest,
    _package_root: &Path,
) {
    let path = std::env::var_os("PATH");
    command.env_clear();
    if let Some(path) = path {
        command.env("PATH", path);
    }
    #[cfg(target_os = "windows")]
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system_root);
    }
}

/// 解析并验证插件包内入口，拒绝绝对路径、父目录和符号链接逃逸。
pub fn resolve_plugin_entrypoint(
    package_root: &Path,
    manifest: &PluginPackageManifest,
) -> Result<PathBuf, PluginProcessError> {
    let root = canonical_package_root(package_root)?;
    let relative = Path::new(&manifest.entrypoint);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(PluginProcessError::InvalidEntrypoint {
            entrypoint: manifest.entrypoint.clone(),
        });
    }
    let mut cursor = root.clone();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(PluginProcessError::InvalidEntrypoint {
                entrypoint: manifest.entrypoint.clone(),
            });
        };
        cursor.push(segment);
        let metadata = std::fs::symlink_metadata(&cursor)
            .map_err(|source| PluginProcessError::Path { source })?;
        if metadata.file_type().is_symlink() {
            return Err(PluginProcessError::SymlinkEntrypoint { path: cursor });
        }
    }
    let entrypoint = cursor
        .canonicalize()
        .map_err(|source| PluginProcessError::Path { source })?;
    if !entrypoint.starts_with(&root) {
        return Err(PluginProcessError::EntrypointEscape { path: entrypoint });
    }
    if !entrypoint.is_file() {
        return Err(PluginProcessError::EntrypointNotFile { path: entrypoint });
    }
    Ok(entrypoint)
}

pub(super) fn canonical_package_root(package_root: &Path) -> Result<PathBuf, PluginProcessError> {
    if !package_root.is_absolute() {
        return Err(PluginProcessError::PackageRootNotAbsolute {
            path: package_root.to_path_buf(),
        });
    }
    let metadata = std::fs::symlink_metadata(package_root)
        .map_err(|source| PluginProcessError::Path { source })?;
    if metadata.file_type().is_symlink() {
        return Err(PluginProcessError::PackageRootSymlink {
            path: package_root.to_path_buf(),
        });
    }
    let root = package_root
        .canonicalize()
        .map_err(|source| PluginProcessError::Path { source })?;
    if !root.is_dir() {
        return Err(PluginProcessError::PackageRootNotDirectory { path: root });
    }
    Ok(root)
}

pub(super) fn validate_granted_capabilities(
    capabilities: &[PluginCapability],
) -> Result<(), PluginProcessError> {
    let mut seen = HashSet::with_capacity(capabilities.len());
    for capability in capabilities {
        if !capability.is_known() {
            return Err(PluginProcessError::UnknownCapability {
                capability: capability.as_str().to_owned(),
            });
        }
        if !seen.insert(capability) {
            return Err(PluginProcessError::DuplicateCapability {
                capability: capability.as_str().to_owned(),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_ready(
    manifest: &PluginPackageManifest,
    message: PluginProcessMessage,
) -> Result<(), PluginProcessError> {
    let PluginProcessMessage::Ready(ready) = message else {
        return Err(PluginProcessError::UnexpectedMessage);
    };
    if ready.plugin_id != manifest.id {
        return Err(PluginProcessError::HandshakeIdentity {
            expected: manifest.id.to_string(),
            actual: ready.plugin_id.to_string(),
        });
    }
    if ready.api_version.major != manifest.api_version.major
        || !ready
            .api_version
            .is_supported_by(CURRENT_PLUGIN_API_VERSION)
    {
        return Err(PluginProcessError::HandshakeApi {
            expected: manifest.api_version,
            actual: ready.api_version,
        });
    }
    let declared = manifest
        .capabilities
        .iter()
        .map(|capability| capability.as_str())
        .collect::<HashSet<_>>();
    for capability in &ready.capabilities {
        if !declared.contains(&capability.as_str()) {
            return Err(PluginProcessError::HandshakeCapability {
                capability: capability.as_str().to_owned(),
            });
        }
    }
    Ok(())
}

pub(super) fn response_result(response: PluginResponse) -> Result<Vec<u8>, PluginProcessError> {
    if response.ok {
        Ok(response.output)
    } else {
        Err(PluginProcessError::Remote {
            message: response
                .error
                .unwrap_or_else(|| "插件返回未说明的失败".into()),
        })
    }
}
