use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{
    MAX_PLUGIN_PACKAGE_BYTES, PLUGIN_PACKAGE_MANIFEST_FILE_NAME, PluginPackageInstallError,
};

pub(crate) fn package_digest(root: &Path) -> Result<String, PluginPackageInstallError> {
    let mut files = BTreeMap::new();
    collect_files(root, root, &mut files)?;
    let mut total_bytes = 0_u64;
    let mut hasher = Sha256::new();
    for (relative, path) in files {
        if relative == Path::new(PLUGIN_PACKAGE_MANIFEST_FILE_NAME) {
            continue;
        }
        let bytes = fs::read(&path).map_err(|source| PluginPackageInstallError::Io {
            operation: "读取插件包文件",
            source,
        })?;
        total_bytes = total_bytes.saturating_add(bytes.len() as u64);
        if total_bytes > MAX_PLUGIN_PACKAGE_BYTES {
            return Err(PluginPackageInstallError::PackageTooLarge {
                bytes: total_bytes,
                max: MAX_PLUGIN_PACKAGE_BYTES,
            });
        }
        let relative = relative.to_string_lossy();
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(&bytes);
        hasher.update([0]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub(super) fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut BTreeMap<PathBuf, PathBuf>,
) -> Result<(), PluginPackageInstallError> {
    let mut entries = fs::read_dir(current)
        .map_err(|source| PluginPackageInstallError::Io {
            operation: "扫描插件包目录",
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| PluginPackageInstallError::Io {
            operation: "读取插件包目录项",
            source,
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|source| PluginPackageInstallError::Io {
                operation: "检查插件包路径",
                source,
            })?;
        if metadata.file_type().is_symlink() {
            return Err(PluginPackageInstallError::SymlinkEntry { path });
        }
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| PluginPackageInstallError::InvalidRelativePath { path: path.clone() })?
                .to_path_buf();
            if relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err(PluginPackageInstallError::InvalidRelativePath { path });
            }
            files.insert(relative, path);
        } else {
            return Err(PluginPackageInstallError::UnsupportedEntry { path });
        }
    }
    Ok(())
}
