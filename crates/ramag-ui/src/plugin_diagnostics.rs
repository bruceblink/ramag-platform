use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::input::InputState;
use gpui_kit::{Context, Window, prelude::*};
use ramag_app::{
    PluginInstallation, PluginInstallationState, PluginInstallationStore, PluginPackageCache,
    PluginPackageCachedArchive, PluginPackageCatalogEntry, PluginPackageCatalogFetcher,
    PluginPackageCatalogStore, PluginPackageDownloader, PluginPackageInstaller, StaticPluginHost,
};
use ramag_domain::PluginId;

/// 展示插件包状态、静态插件生命周期、注册错误和当前可用入口。
pub(crate) struct PluginDiagnosticsView {
    host: Arc<StaticPluginHost>,
    installations: Vec<PluginInstallation>,
    catalog_entries: Vec<PluginPackageCatalogEntry>,
    installation_store: Option<Arc<PluginInstallationStore>>,
    package_installer: Option<Arc<PluginPackageInstaller>>,
    package_downloader: Option<Arc<PluginPackageDownloader>>,
    package_download_root: Option<PathBuf>,
    package_cache: Option<PluginPackageCache>,
    catalog_source_url: Option<gpui_kit::Entity<InputState>>,
    catalog_fetcher: Option<Arc<PluginPackageCatalogFetcher>>,
    catalog_store: Option<Arc<PluginPackageCatalogStore>>,
    catalog_refreshing: bool,
    catalog_refresh_error: Option<String>,
    installing_archive: bool,
    downloading_package: Option<String>,
    installing_package: Option<String>,
    downloaded_packages: HashMap<String, PathBuf>,
    pending_uninstall: Option<PluginId>,
    installation_error: Option<String>,
    download_error: Option<String>,
}

impl PluginDiagnosticsView {
    /// 注入已加载的安装状态，避免渲染过程读取文件或触发外部插件行为。
    pub(crate) fn with_installations(
        host: Arc<StaticPluginHost>,
        installations: Vec<PluginInstallation>,
    ) -> Self {
        Self {
            host,
            installations,
            catalog_entries: Vec::new(),
            installation_store: None,
            package_installer: None,
            package_downloader: None,
            package_download_root: None,
            package_cache: None,
            catalog_source_url: None,
            catalog_fetcher: None,
            catalog_store: None,
            catalog_refreshing: false,
            catalog_refresh_error: None,
            installing_archive: false,
            downloading_package: None,
            installing_package: None,
            downloaded_packages: HashMap::new(),
            pending_uninstall: None,
            installation_error: None,
            download_error: None,
        }
    }

    /// 初始化或替换目录源输入；仅更新控件，不发起网络请求。
    pub(crate) fn set_catalog_source_url(
        &mut self,
        source_url: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.catalog_source_url = Some(cx.new(|cx| {
            InputState::new(window, cx)
                .validate(|value, _| value.len() <= ramag_app::MAX_PLUGIN_CATALOG_URL_BYTES)
                .placeholder("https://example.com/ramag/catalog.json")
                .default_value(source_url.unwrap_or_default())
        }));
        self.catalog_refresh_error = None;
        cx.notify();
    }

    /// 注入安装状态存储；设置页按钮通过该存储原子更新启用状态。
    pub(crate) fn set_installation_store(
        &mut self,
        installation_store: Arc<PluginInstallationStore>,
        cx: &mut Context<Self>,
    ) {
        self.installation_store = Some(installation_store);
        cx.notify();
    }

    /// 注入包安装器，使设置页可以在确认后安全卸载本地插件。
    pub(crate) fn set_package_installer(
        &mut self,
        package_installer: Arc<PluginPackageInstaller>,
        cx: &mut Context<Self>,
    ) {
        self.package_installer = Some(package_installer);
        cx.notify();
    }

    /// 更新已加载的安装状态，并通知设置页重新布局。
    pub(crate) fn set_installations(
        &mut self,
        installations: Vec<PluginInstallation>,
        cx: &mut Context<Self>,
    ) {
        self.installations = installations;
        self.pending_uninstall = None;
        self.installation_error = None;
        cx.notify();
    }

    /// 更新已校验的本地目录候选；仅改变展示，不触发下载或安装。
    pub(crate) fn set_catalog_entries(
        &mut self,
        entries: Vec<PluginPackageCatalogEntry>,
        cx: &mut Context<Self>,
    ) {
        self.catalog_entries = entries;
        self.downloading_package = None;
        self.installing_package = None;
        self.downloaded_packages.clear();
        self.download_error = None;
        self.catalog_refresh_error = None;
        cx.notify();
    }

    /// 注入包下载器；下载任务只写入独立缓存目录，不调用安装器。
    pub(crate) fn set_package_downloader(
        &mut self,
        package_downloader: Arc<PluginPackageDownloader>,
        cx: &mut Context<Self>,
    ) {
        self.package_downloader = Some(package_downloader);
        cx.notify();
    }

    /// 注入插件状态目录下的下载缓存根目录，避免 UI 选择任意写入路径。
    pub(crate) fn set_package_download_root(
        &mut self,
        package_download_root: PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.package_cache = Some(PluginPackageCache::new(package_download_root.clone()));
        self.package_download_root = Some(package_download_root);
        cx.notify();
    }

    /// 注入受限远程目录获取器；设置页不会在注入时发起请求。
    pub(crate) fn set_catalog_fetcher(
        &mut self,
        catalog_fetcher: Arc<PluginPackageCatalogFetcher>,
        cx: &mut Context<Self>,
    ) {
        self.catalog_fetcher = Some(catalog_fetcher);
        cx.notify();
    }

    /// 注入目录本地存储；保存失败时获取器保留旧清单文件。
    pub(crate) fn set_catalog_store(
        &mut self,
        catalog_store: Arc<PluginPackageCatalogStore>,
        cx: &mut Context<Self>,
    ) {
        self.catalog_store = Some(catalog_store);
        cx.notify();
    }

    /// 按用户输入手动获取目录；成功后才保存 URL 偏好并刷新候选。
    fn refresh_catalog(&mut self, cx: &mut Context<Self>) {
        if self.catalog_refreshing {
            return;
        }
        let Some(source_input) = self.catalog_source_url.as_ref() else {
            self.catalog_refresh_error = Some("目录源输入尚未初始化".into());
            cx.notify();
            return;
        };
        let source_url = source_input.read(cx).value().trim().to_owned();
        if source_url.is_empty() {
            self.catalog_refresh_error = Some("请输入 HTTPS 目录源地址".into());
            cx.notify();
            return;
        }
        let Some(fetcher) = self.catalog_fetcher.clone() else {
            self.catalog_refresh_error = Some("远程目录获取器尚未初始化".into());
            cx.notify();
            return;
        };
        let Some(store) = self.catalog_store.clone() else {
            self.catalog_refresh_error = Some("远程目录存储尚未初始化".into());
            cx.notify();
            return;
        };
        let cache = self.package_cache.clone();
        self.catalog_refreshing = true;
        self.catalog_refresh_error = None;
        cx.notify();
        let task = cx.background_executor().spawn({
            let source_url = source_url.clone();
            async move {
                let catalog = fetcher.fetch_and_store_on_managed_runtime(&source_url, &store)?;
                let downloaded_packages = match restore_catalog_cache(cache, &catalog.entries) {
                    Ok(packages) => packages,
                    Err(error) => {
                        tracing::warn!(
                            operation = "plugin_package_cache_restore",
                            error = %error,
                            "目录刷新后的插件包缓存恢复失败"
                        );
                        Vec::new()
                    }
                };
                Ok::<_, ramag_app::PluginPackageCatalogFetchError>((catalog, downloaded_packages))
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| {
                view.catalog_refreshing = false;
                match result {
                    Ok((catalog, downloaded_packages)) => {
                        view.set_catalog_entries(catalog.entries, cx);
                        view.set_downloaded_packages(downloaded_packages, cx);
                        view.catalog_refresh_error = None;
                        crate::preferences::persist_preference_latest(
                            crate::PLUGIN_CATALOG_SOURCE_URL_PREF_KEY,
                            source_url,
                            cx,
                        );
                    }
                    Err(error) => {
                        view.catalog_refresh_error = Some(error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 注入启动时已经核验的缓存归档，恢复目录候选的“已下载”状态。
    pub(crate) fn set_downloaded_packages(
        &mut self,
        packages: Vec<PluginPackageCachedArchive>,
        cx: &mut Context<Self>,
    ) {
        self.downloaded_packages = packages
            .into_iter()
            .map(|package| (package.key(), package.path))
            .collect();
        cx.notify();
    }

    /// 选择本地 `tar.gz` 归档并在后台安装；文件选择取消时不改变当前状态。
    fn pick_archive(&mut self, cx: &mut Context<Self>) {
        if self.installing_archive
            || self.downloading_package.is_some()
            || self.installing_package.is_some()
        {
            return;
        }
        let Some(installer) = self.package_installer.clone() else {
            self.installation_error = Some("插件安装器尚未初始化".into());
            cx.notify();
            return;
        };
        self.installing_archive = true;
        self.installation_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let picked = rfd::AsyncFileDialog::new()
                .add_filter("插件归档", &["tar.gz", "tgz"])
                .pick_file()
                .await;
            let result = if let Some(handle) = picked {
                let path = handle.path().to_path_buf();
                Some(
                    cx.background_executor()
                        .spawn(async move { installer.install_from_archive(&path) })
                        .await,
                )
            } else {
                None
            };
            let _ = this.update(cx, |view, cx| {
                view.installing_archive = false;
                if let Some(result) = result {
                    match result {
                        Ok(installation) => {
                            if let Some(existing) = view
                                .installations
                                .iter_mut()
                                .find(|current| current.manifest.id == installation.manifest.id)
                            {
                                *existing = installation;
                            } else {
                                view.installations.push(installation);
                            }
                            view.installation_error = None;
                        }
                        Err(error) => {
                            view.installation_error = Some(format!("插件归档安装失败：{error}"));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 切换插件启用状态；只更新持久化状态，不启动外部进程。
    fn toggle_installation(&mut self, plugin_id: PluginId, cx: &mut Context<Self>) {
        let Some(store) = self.installation_store.clone() else {
            return;
        };
        let Some(current_state) = self
            .installations
            .iter()
            .find(|installation| installation.manifest.id == plugin_id)
            .map(|installation| installation.state)
        else {
            return;
        };
        let next_state = match current_state {
            PluginInstallationState::Enabled => PluginInstallationState::Disabled,
            PluginInstallationState::Installed
            | PluginInstallationState::Disabled
            | PluginInstallationState::Quarantined => PluginInstallationState::Enabled,
        };
        match store.set_state(&plugin_id, next_state, None) {
            Ok(updated) => {
                if let Some(current) = self
                    .installations
                    .iter_mut()
                    .find(|installation| installation.manifest.id == plugin_id)
                {
                    *current = updated;
                }
                self.installation_error = None;
            }
            Err(error) => {
                self.installation_error = Some(format!("插件状态保存失败：{error}"));
            }
        }
        cx.notify();
    }

    /// 记录待确认的卸载请求，不在第一次点击时修改文件。
    fn request_uninstall(&mut self, plugin_id: PluginId, cx: &mut Context<Self>) {
        if self.installing_archive
            || self.downloading_package.is_some()
            || self.installing_package.is_some()
        {
            return;
        }
        if self
            .installations
            .iter()
            .any(|installation| installation.manifest.id == plugin_id)
        {
            self.pending_uninstall = Some(plugin_id);
            self.installation_error = None;
            cx.notify();
        }
    }

    /// 取消待确认的卸载请求。
    fn cancel_uninstall(&mut self, cx: &mut Context<Self>) {
        self.pending_uninstall = None;
        cx.notify();
    }

    /// 执行已确认的卸载，并在失败时保留当前插件行和错误信息。
    fn confirm_uninstall(&mut self, plugin_id: PluginId, cx: &mut Context<Self>) {
        let Some(installer) = self.package_installer.clone() else {
            self.installation_error = Some("插件卸载器尚未初始化".into());
            self.pending_uninstall = None;
            cx.notify();
            return;
        };
        match installer.uninstall(&plugin_id) {
            Ok(true) => {
                self.installations
                    .retain(|installation| installation.manifest.id != plugin_id);
                self.installation_error = None;
            }
            Ok(false) => {
                self.installation_error = Some("插件安装记录不存在，未执行卸载".into());
            }
            Err(error) => {
                self.installation_error = Some(format!("插件卸载失败：{error}"));
            }
        }
        self.pending_uninstall = None;
        cx.notify();
    }

    /// 下载一个已校验目录候选到应用缓存；下载完成后只显示缓存文件，不自动安装。
    fn download_package(&mut self, entry: PluginPackageCatalogEntry, cx: &mut Context<Self>) {
        let key = package_download_key(&entry);
        if self.installing_archive
            || self.downloading_package.is_some()
            || self.installing_package.is_some()
            || self.downloaded_packages.contains_key(&key)
        {
            return;
        }
        let Some(downloader) = self.package_downloader.clone() else {
            self.download_error = Some("插件下载器尚未初始化".into());
            cx.notify();
            return;
        };
        let Some(download_root) = self.package_download_root.clone() else {
            self.download_error = Some("插件下载缓存目录尚未初始化".into());
            cx.notify();
            return;
        };
        let destination = package_download_destination(&download_root, &entry);
        let plugin_id = entry.id.clone();
        let version = entry.version.clone();
        let cache = self.package_cache.clone();
        let cache_entry = entry.clone();
        self.downloading_package = Some(key.clone());
        self.download_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let path = downloader
                        .download_to_managed_runtime(&entry, &destination)
                        .await
                        .map_err(|error| error.to_string())?;
                    let Some(cache) = cache else {
                        let _ = std::fs::remove_file(&path);
                        return Err("插件包缓存未初始化".to_owned());
                    };
                    if let Err(error) = cache.record_download(&cache_entry, &path) {
                        let _ = std::fs::remove_file(&path);
                        return Err(format!("插件包缓存记录失败：{error}"));
                    }
                    Ok::<_, String>(path)
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.downloading_package = None;
                match result {
                    Ok(path) => {
                        view.downloaded_packages.insert(key, path);
                        view.download_error = None;
                    }
                    Err(error) => {
                        tracing::warn!(
                            operation = "plugin_package_download",
                            plugin_id = %plugin_id,
                            version = %version,
                            error = %error,
                            "插件包下载失败"
                        );
                        view.download_error = Some(format!("插件包下载失败：{error}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 安装当前会话已校验的目录缓存，并在安装成功后清理缓存文件。
    fn install_downloaded_package(
        &mut self,
        entry: PluginPackageCatalogEntry,
        cx: &mut Context<Self>,
    ) {
        let key = package_download_key(&entry);
        if self.installing_archive
            || self.downloading_package.is_some()
            || self.installing_package.is_some()
        {
            return;
        }
        let Some(archive_path) = self.downloaded_packages.get(&key).cloned() else {
            return;
        };
        let Some(installer) = self.package_installer.clone() else {
            self.download_error = Some("插件安装器尚未初始化".into());
            cx.notify();
            return;
        };
        let plugin_id = entry.id.clone();
        let version = entry.version.clone();
        self.installing_package = Some(key.clone());
        self.download_error = None;
        cx.notify();
        let archive_for_install = archive_path.clone();
        let cache = self.package_cache.clone();
        let cache_entry = entry.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(
                    async move { installer.install_catalog_archive(&entry, &archive_for_install) },
                )
                .await;
            let cleanup_error = if result.is_ok() {
                match cache {
                    Some(cache) => cache
                        .remove_download(&cache_entry, &archive_path)
                        .err()
                        .map(|error| error.to_string()),
                    None => std::fs::remove_file(&archive_path)
                        .err()
                        .filter(|error| error.kind() != std::io::ErrorKind::NotFound)
                        .map(|error| error.to_string()),
                }
            } else {
                None
            };
            let _ = this.update(cx, |view, cx| {
                view.installing_package = None;
                match result {
                    Ok(installation) => {
                        if let Some(existing) = view
                            .installations
                            .iter_mut()
                            .find(|current| current.manifest.id == installation.manifest.id)
                        {
                            *existing = installation;
                        } else {
                            view.installations.push(installation);
                        }
                        view.downloaded_packages.remove(&key);
                        if let Some(error) = cleanup_error {
                            tracing::warn!(
                                operation = "plugin_package_cache_cleanup",
                                plugin_id = %plugin_id,
                                version = %version,
                                path = %archive_path.display(),
                                error = %error,
                                "插件包安装后缓存清理失败"
                            );
                        }
                        view.download_error = None;
                    }
                    Err(error) => {
                        tracing::warn!(
                            operation = "plugin_package_install",
                            plugin_id = %plugin_id,
                            version = %version,
                            error = %error,
                            "目录候选插件安装失败"
                        );
                        view.download_error = Some(format!("插件包安装失败：{error}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[path = "plugin_diagnostics/diagnostics.rs"]
mod diagnostics;
#[path = "plugin_diagnostics/installations.rs"]
mod installations;
#[path = "plugin_diagnostics/package_catalog.rs"]
mod package_catalog;
#[path = "plugin_diagnostics/render.rs"]
mod render;
#[path = "plugin_diagnostics/static_catalog.rs"]
mod static_catalog;

use diagnostics::{render_available_entries, render_diagnostics, render_empty_state};
use installations::{InstallationRenderState, render_installations};
use package_catalog::{package_download_destination, package_download_key};
use package_catalog::{
    render_catalog_source_controls, render_package_catalog, restore_catalog_cache,
};
use static_catalog::{render_catalog, render_summary};

#[cfg(test)]
#[path = "plugin_diagnostics_tests.rs"]
mod tests;
