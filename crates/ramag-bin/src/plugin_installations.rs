//! 启动期可安装插件状态、目录和下载缓存的装配。

use super::*;

pub(super) fn load_plugin_installations() -> PluginInstallationLoad {
    let root = if let Some(path) = std::env::var_os("RAMAG_PLUGIN_STATE_DIR") {
        let path = std::path::PathBuf::from(path);
        if !path.is_absolute() {
            warn!(
                operation = "plugin_installation_load",
                "插件状态目录必须是绝对路径，忽略环境变量"
            );
            return (None, None, Vec::new(), Vec::new(), None, None, Vec::new());
        }
        path
    } else {
        let Some(dirs) = directories::ProjectDirs::from("com", "ramag", "ramag") else {
            warn!(
                operation = "plugin_installation_load",
                "无法定位插件状态目录"
            );
            return (None, None, Vec::new(), Vec::new(), None, None, Vec::new());
        };
        dirs.data_dir().join("plugins")
    };
    let package_root = root.join("packages");
    let store = Arc::new(ramag_app::PluginInstallationStore::new(&root));
    let package_installer = Arc::new(ramag_app::PluginPackageInstaller::new(
        package_root,
        store.clone(),
    ));
    let package_downloader = match ramag_app::PluginPackageDownloader::new() {
        Ok(downloader) => Some(Arc::new(downloader)),
        Err(error) => {
            warn!(
                operation = "plugin_package_downloader_init",
                error = %error,
                "插件包下载器初始化失败"
            );
            None
        }
    };
    let package_download_root = root.join("downloads");
    let catalog = load_plugin_catalog(&root);
    let cached_packages =
        match ramag_app::PluginPackageCache::new(package_download_root.clone()).restore(&catalog) {
            Ok(packages) => packages,
            Err(error) => {
                warn!(
                    operation = "plugin_package_cache_restore",
                    error = %error,
                    "插件包下载缓存恢复失败"
                );
                Vec::new()
            }
        };
    match store.load() {
        Ok(installations) => {
            info!(
                operation = "plugin_installation_load",
                count = installations.len(),
                "installable plugin states loaded"
            );
            (
                Some(store),
                Some(package_installer),
                installations,
                catalog,
                package_downloader,
                Some(package_download_root),
                cached_packages,
            )
        }
        Err(error) => {
            warn!(
                operation = "plugin_installation_load",
                error = %error,
                "plugin installation state unavailable"
            );
            (
                Some(store),
                Some(package_installer),
                Vec::new(),
                catalog,
                package_downloader,
                Some(package_download_root),
                cached_packages,
            )
        }
    }
}

/// 启动时读取本地目录候选；目录损坏只影响展示，不阻止插件状态和内置工具启动。
fn load_plugin_catalog(root: &std::path::Path) -> Vec<ramag_app::PluginPackageCatalogEntry> {
    let path = root.join("catalog.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            warn!(
                operation = "plugin_catalog_load",
                path = %path.display(),
                error = %error,
                "本地插件目录读取失败"
            );
            return Vec::new();
        }
    };
    match ramag_app::PluginPackageCatalog::parse_json(&bytes) {
        Ok(catalog) => {
            info!(
                operation = "plugin_catalog_load",
                path = %path.display(),
                count = catalog.entries().len(),
                "本地插件目录已加载"
            );
            catalog.entries.to_vec()
        }
        Err(error) => {
            warn!(
                operation = "plugin_catalog_load",
                path = %path.display(),
                error = %error,
                "本地插件目录校验失败"
            );
            Vec::new()
        }
    }
}
