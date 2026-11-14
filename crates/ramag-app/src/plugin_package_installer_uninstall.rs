use std::fs;
use std::path::Path;

/// 恢复卸载前的版本目录，发现目标被替换时直接失败以避免误删外部目录。
pub(super) fn restore_uninstall_directory(
    target: &Path,
    backup: &Path,
) -> Result<(), std::io::Error> {
    match fs::symlink_metadata(target) {
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "待恢复插件目标目录已被替换",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::rename(backup, target)
}

/// 只删除已经为空的插件 ID 目录，保留其他版本目录。
pub(super) fn remove_empty_plugin_directory(
    packages_root: &Path,
    plugin_id: &ramag_domain::PluginId,
) {
    let plugin_dir = packages_root.join(plugin_id.as_str());
    if let Ok(metadata) = fs::symlink_metadata(&plugin_dir)
        && metadata.is_dir()
        && !metadata.file_type().is_symlink()
    {
        let _ = fs::remove_dir(&plugin_dir);
    }
}
