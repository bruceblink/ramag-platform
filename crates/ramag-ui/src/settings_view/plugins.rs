//! 插件设置页的服务与启动状态注入。

use super::*;

impl SettingsView {
    /// 注入启动时读取的可安装插件状态，并刷新插件设置页。
    pub fn set_plugin_installations(
        &mut self,
        installations: Vec<PluginInstallation>,
        cx: &mut Context<Self>,
    ) {
        self.plugin_diagnostics
            .update(cx, |view, cx| view.set_installations(installations, cx));
        cx.notify();
    }

    /// 注入已校验的本地目录候选，并刷新插件设置页展示。
    pub fn set_plugin_catalog_entries(
        &mut self,
        entries: Vec<PluginPackageCatalogEntry>,
        cx: &mut Context<Self>,
    ) {
        self.plugin_diagnostics
            .update(cx, |view, cx| view.set_catalog_entries(entries, cx));
        cx.notify();
    }

    /// 注入插件状态存储，使插件设置页可以保存启用和禁用操作。
    pub fn set_plugin_installation_store(
        &mut self,
        installation_store: Arc<PluginInstallationStore>,
        cx: &mut Context<Self>,
    ) {
        self.plugin_diagnostics.update(cx, |view, cx| {
            view.set_installation_store(installation_store, cx)
        });
        cx.notify();
    }

    /// 注入插件包安装器，使插件设置页可以执行确认后的卸载操作。
    pub fn set_plugin_package_installer(
        &mut self,
        package_installer: Arc<PluginPackageInstaller>,
        cx: &mut Context<Self>,
    ) {
        self.plugin_diagnostics.update(cx, |view, cx| {
            view.set_package_installer(package_installer, cx)
        });
        cx.notify();
    }

    /// 注入插件包下载器，使目录候选可以保存到应用缓存。
    pub fn set_plugin_package_downloader(
        &mut self,
        package_downloader: Arc<PluginPackageDownloader>,
        cx: &mut Context<Self>,
    ) {
        self.plugin_diagnostics.update(cx, |view, cx| {
            view.set_package_downloader(package_downloader, cx)
        });
        cx.notify();
    }

    /// 注入插件包下载缓存目录，限制设置页的写入范围。
    pub fn set_plugin_package_download_root(
        &mut self,
        package_download_root: PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.plugin_diagnostics.update(cx, |view, cx| {
            view.set_package_download_root(package_download_root, cx)
        });
        cx.notify();
    }

    /// 注入启动时核验通过的下载缓存，恢复目录候选的安装动作。
    pub fn set_plugin_downloaded_packages(
        &mut self,
        packages: Vec<ramag_app::PluginPackageCachedArchive>,
        cx: &mut Context<Self>,
    ) {
        self.plugin_diagnostics
            .update(cx, |view, cx| view.set_downloaded_packages(packages, cx));
        cx.notify();
    }

    /// 注入远程插件目录获取器；设置页仍只在用户点击刷新时联网。
    pub fn set_plugin_catalog_fetcher(
        &mut self,
        fetcher: Arc<PluginPackageCatalogFetcher>,
        cx: &mut Context<Self>,
    ) {
        self.plugin_diagnostics
            .update(cx, |view, cx| view.set_catalog_fetcher(fetcher, cx));
        cx.notify();
    }

    /// 注入远程插件目录本地存储。
    pub fn set_plugin_catalog_store(
        &mut self,
        store: Arc<PluginPackageCatalogStore>,
        cx: &mut Context<Self>,
    ) {
        self.plugin_diagnostics
            .update(cx, |view, cx| view.set_catalog_store(store, cx));
        cx.notify();
    }

    /// 初始化远程目录源输入；网络请求仍由设置页按钮显式触发。
    pub fn set_plugin_catalog_source_url(
        &mut self,
        source_url: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.plugin_diagnostics.update(cx, |view, cx| {
            view.set_catalog_source_url(source_url, window, cx)
        });
        cx.notify();
    }
}
