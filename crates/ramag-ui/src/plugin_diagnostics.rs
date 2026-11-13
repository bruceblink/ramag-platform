use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Sizable as _, button::ButtonVariants as _,
    h_flex, input::Input, input::InputState, v_flex,
};
use gpui_kit::{
    Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, prelude::*, px,
};
use ramag_app::{
    PluginCatalogEntry, PluginDiagnostic, PluginInstallation, PluginInstallationState,
    PluginInstallationStore, PluginLifecycleStage, PluginPackageCache, PluginPackageCachedArchive,
    PluginPackageCatalogEntry, PluginPackageCatalogFetcher, PluginPackageCatalogStore,
    PluginPackageDownloader, PluginPackageInstaller, PluginState, StaticPluginHost,
};
use ramag_domain::PluginId;

static PACKAGE_DOWNLOAD_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
        self.catalog_refreshing = true;
        self.catalog_refresh_error = None;
        cx.notify();
        let task = cx.background_executor().spawn({
            let source_url = source_url.clone();
            async move { fetcher.fetch_and_store(&source_url, &store).await }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| {
                view.catalog_refreshing = false;
                match result {
                    Ok(catalog) => {
                        view.set_catalog_entries(catalog.entries, cx);
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

impl Render for PluginDiagnosticsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let diagnostics = self.host.diagnostics();
        let catalog = self.host.catalog();
        let available = self.host.registry().list();
        let failed_count = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.failure.is_some())
            .count();
        let ready_count = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.state == PluginState::Ready)
            .count();

        v_flex()
            .id("plugin-diagnostics")
            .debug_selector(|| "plugin-diagnostics".into())
            .w_full()
            .min_w_0()
            .gap(px(12.0))
            .child(render_summary(
                diagnostics.len(),
                ready_count,
                failed_count,
                cx,
            ))
            .child(render_installations(
                &self.installations,
                self.installation_store.is_some(),
                self.package_installer.is_some(),
                InstallationRenderState {
                    pending_uninstall: self.pending_uninstall.as_ref(),
                    operation_error: self.installation_error.as_deref(),
                    installing_archive: self.installing_archive,
                    package_operation_busy: self.downloading_package.is_some()
                        || self.installing_package.is_some(),
                },
                cx,
            ))
            .when_some(self.catalog_source_url.as_ref(), |content, source_input| {
                content.child(render_catalog_source_controls(
                    source_input,
                    self.catalog_fetcher.is_some() && self.catalog_store.is_some(),
                    self.catalog_refreshing,
                    self.catalog_refresh_error.as_deref(),
                    cx,
                ))
            })
            .child(render_package_catalog(
                &self.catalog_entries,
                &self.installations,
                self.downloading_package.as_deref(),
                self.installing_package.as_deref(),
                &self.downloaded_packages,
                self.download_error.as_deref(),
                cx,
            ))
            .child(render_catalog(&catalog, cx))
            .child(render_available_entries(&available, cx))
            .child(render_diagnostics(&diagnostics, cx))
    }
}

/// 安装列表的动作状态只读快照，避免渲染函数接收过多彼此相关的参数。
struct InstallationRenderState<'a> {
    pending_uninstall: Option<&'a PluginId>,
    operation_error: Option<&'a str>,
    installing_archive: bool,
    package_operation_busy: bool,
}

fn render_installations(
    installations: &[PluginInstallation],
    can_toggle: bool,
    can_uninstall: bool,
    state: InstallationRenderState<'_>,
    cx: &mut Context<PluginDiagnosticsView>,
) -> impl IntoElement {
    let theme = cx.theme();
    let rows = installations
        .iter()
        .map(|installation| {
            let manifest = &installation.manifest;
            let row_id = format!("plugin-installation-{}", manifest.id);
            let toggle_id = format!("plugin-installation-toggle-{}", manifest.id);
            let uninstall_id = format!("plugin-installation-uninstall-{}", manifest.id);
            let plugin_id = manifest.id.clone();
            let uninstall_plugin_id = manifest.id.clone();
            let status = installation_state_label(installation.state);
            let toggle_label = installation_toggle_label(installation.state);
            let runtime = match manifest.runtime {
                ramag_domain::PluginPackageRuntime::Process => "进程",
            };
            let status_color = installation_state_color(installation.state, theme);
            let error = installation.last_error.as_deref().map(|message| {
                crate::settings_view::settings_error_notice(
                    format!(
                        "plugin-installation-{manifest_id}-recent-error",
                        manifest_id = manifest.id
                    ),
                    "最近错误",
                    message.to_owned(),
                    theme,
                )
                .into_any_element()
            });
            v_flex()
                .id(row_id.clone())
                .debug_selector(move || row_id.clone())
                .w_full()
                .min_w_0()
                .gap(px(4.0))
                .p(px(10.0))
                .bg(theme.background)
                .border_1()
                .border_color(theme.border)
                .rounded(px(6.0))
                .child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .flex_wrap()
                        .items_start()
                        .gap(px(8.0))
                        .child(
                            div()
                                .mt(px(4.0))
                                .size(px(8.0))
                                .flex_none()
                                .rounded(px(4.0))
                                .bg(status_color),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.foreground)
                                        .child(manifest.name.clone()),
                                )
                                .child(div().text_xs().text_color(theme.muted_foreground).child(
                                    format!(
                                        "插件 {} · 版本 {} · API {}",
                                        manifest.id, manifest.version, manifest.api_version
                                    ),
                                )),
                        )
                        .child(
                            h_flex()
                                .flex_none()
                                .max_w_full()
                                .flex_wrap()
                                .items_center()
                                .gap(px(6.0))
                                .child(crate::pulse_ui::pulse_status_badge_with_label(
                                    crate::pulse_ui::PulseStatus::Current,
                                    status,
                                    cx,
                                ))
                                .when(can_toggle, |actions| {
                                    actions.child(
                                        crate::clickable_button(toggle_id.clone())
                                            .debug_selector(move || toggle_id.clone())
                                            .xsmall()
                                            .outline()
                                            .label(toggle_label)
                                            .on_click(cx.listener(
                                                move |view, _: &gpui_kit::ClickEvent, _, cx| {
                                                    view.toggle_installation(plugin_id.clone(), cx);
                                                },
                                            )),
                                    )
                                })
                                .when(can_uninstall, |actions| {
                                    actions.child(
                                        crate::clickable_button(uninstall_id.clone())
                                            .debug_selector(move || uninstall_id.clone())
                                            .xsmall()
                                            .danger()
                                            .label("卸载")
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.request_uninstall(
                                                    uninstall_plugin_id.clone(),
                                                    cx,
                                                );
                                            })),
                                    )
                                }),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("运行时：{runtime} · 入口：{}", manifest.entrypoint)),
                )
                .children(error)
                .into_any_element()
        })
        .collect::<Vec<_>>();
    let body = if rows.is_empty() {
        v_flex()
            .w_full()
            .min_w_0()
            .gap(px(8.0))
            .when_some(state.operation_error, |content, error| {
                content.child(crate::settings_view::settings_error_notice(
                    "plugin-installations-error",
                    "插件操作失败",
                    error.to_owned(),
                    theme,
                ))
            })
            .child(render_empty_state(
                "plugin-installations",
                IconName::Folder,
                "暂无可安装插件",
                "安装包状态会显示在这里；当前页面不会启动外部代码。",
                theme,
            ))
            .into_any_element()
    } else {
        v_flex()
            .w_full()
            .min_w_0()
            .gap(px(8.0))
            .when_some(state.operation_error, |content, error| {
                content.child(crate::settings_view::settings_error_notice(
                    "plugin-installations-error",
                    "插件操作失败",
                    error.to_owned(),
                    theme,
                ))
            })
            .children(rows)
            .into_any_element()
    };
    let confirmation = state.pending_uninstall.and_then(|plugin_id| {
        installations
            .iter()
            .find(|installation| installation.manifest.id == *plugin_id)
            .map(|installation| {
                let plugin_id = installation.manifest.id.clone();
                v_flex()
                    .id("plugin-installation-confirmation")
                    .debug_selector(|| "plugin-installation-confirmation".into())
                    .gap(px(8.0))
                    .p(px(10.0))
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(6.0))
                    .child(div().text_sm().text_color(theme.foreground).child(format!(
                        "确认卸载“{}”？这会删除本地插件包和安装状态。",
                        installation.manifest.name
                    )))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(
                                crate::clickable_button("plugin-installation-cancel-uninstall")
                                    .debug_selector(|| {
                                        "plugin-installation-cancel-uninstall".into()
                                    })
                                    .small()
                                    .ghost()
                                    .label("取消")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.cancel_uninstall(cx);
                                    })),
                            )
                            .child(
                                crate::clickable_button("plugin-installation-confirm-uninstall")
                                    .debug_selector(|| {
                                        "plugin-installation-confirm-uninstall".into()
                                    })
                                    .small()
                                    .danger()
                                    .label("确认卸载")
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.confirm_uninstall(plugin_id.clone(), cx);
                                    })),
                            ),
                    )
                    .into_any_element()
            })
    });

    crate::pulse_ui::pulse_panel(cx)
        .id("plugin-installations")
        .debug_selector(|| "plugin-installations".into())
        .gap(px(12.0))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(crate::pulse_ui::pulse_display_heading(
                    "已安装插件",
                    18.0,
                    cx,
                ))
                .child(
                    crate::clickable_button("plugin-installation-install-archive")
                        .debug_selector(|| "plugin-installation-install-archive".into())
                        .small()
                        .outline()
                        .label(if state.installing_archive {
                            "安装中…"
                        } else {
                            "安装本地归档"
                        })
                        .disabled(state.package_operation_busy || state.installing_archive)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.pick_archive(cx);
                        })),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("显示插件包版本、运行时和本地状态；启用或禁用只保存状态，不启动外部进程。"),
        )
        .child(body)
        .children(confirmation)
}

fn installation_state_label(state: PluginInstallationState) -> &'static str {
    match state {
        PluginInstallationState::Installed => "已安装",
        PluginInstallationState::Enabled => "已启用",
        PluginInstallationState::Disabled => "已禁用",
        PluginInstallationState::Quarantined => "已隔离",
    }
}

fn installation_toggle_label(state: PluginInstallationState) -> &'static str {
    match state {
        PluginInstallationState::Enabled => "禁用",
        PluginInstallationState::Installed
        | PluginInstallationState::Disabled
        | PluginInstallationState::Quarantined => "启用",
    }
}

fn installation_state_color(
    state: PluginInstallationState,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::Hsla {
    match state {
        PluginInstallationState::Installed => theme.muted_foreground,
        PluginInstallationState::Enabled => theme.success,
        PluginInstallationState::Disabled => theme.warning,
        PluginInstallationState::Quarantined => theme.danger,
    }
}

/// 展示目录源输入和手动刷新动作；不在渲染或启动时访问网络。
fn render_catalog_source_controls(
    source_input: &gpui_kit::Entity<InputState>,
    can_refresh: bool,
    refreshing: bool,
    error: Option<&str>,
    cx: &mut Context<PluginDiagnosticsView>,
) -> impl IntoElement {
    let theme = cx.theme();
    let button_label = if refreshing {
        "刷新中…"
    } else {
        "刷新目录"
    };
    v_flex()
        .id("plugin-package-catalog-source")
        .debug_selector(|| "plugin-package-catalog-source".into())
        .w_full()
        .min_w_0()
        .gap(px(8.0))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .flex_wrap()
                .items_center()
                .gap(px(8.0))
                .child(
                    Input::new(source_input)
                        .id("plugin-package-catalog-source-input")
                        .flex_1()
                        .min_w(px(220.0))
                        .small(),
                )
                .child(
                    crate::clickable_button("plugin-package-catalog-refresh")
                        .debug_selector(|| "plugin-package-catalog-refresh".into())
                        .small()
                        .outline()
                        .label(button_label)
                        .disabled(!can_refresh || refreshing)
                        .on_click(cx.listener(|view, _, _, cx| view.refresh_catalog(cx))),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("仅在点击“刷新目录”后请求 HTTPS 清单；成功后保存目录源地址。"),
        )
        .when_some(error, |content, error| {
            content.child(crate::settings_view::settings_error_notice(
                "plugin-package-catalog-source-error",
                "目录刷新失败",
                error.to_owned(),
                theme,
            ))
        })
}

/// 展示本地目录中已校验的安装候选；候选页不发起网络请求或代码执行。
fn render_package_catalog(
    entries: &[PluginPackageCatalogEntry],
    installations: &[PluginInstallation],
    downloading_package: Option<&str>,
    installing_package: Option<&str>,
    downloaded_packages: &HashMap<String, PathBuf>,
    download_error: Option<&str>,
    cx: &mut Context<PluginDiagnosticsView>,
) -> impl IntoElement {
    let theme = cx.theme();
    let rows = entries
        .iter()
        .map(|entry| {
            let row_id = format!("plugin-package-catalog-{}-{}", entry.id, entry.version);
            let package_key = package_download_key(entry);
            let download_id = format!("plugin-package-catalog-download-{package_key}");
            let install_id = format!("plugin-package-catalog-install-{package_key}");
            let url_id = format!("plugin-package-catalog-url-{package_key}");
            let is_downloading = downloading_package == Some(package_key.as_str());
            let is_installing = installing_package == Some(package_key.as_str());
            let is_downloaded = downloaded_packages.contains_key(&package_key);
            let is_installed = installations.iter().any(|installation| {
                installation.manifest.id == entry.id
                    && installation.manifest.version == entry.version
            });
            let action_label = if is_downloading {
                "下载中…"
            } else if is_installing {
                "安装中…"
            } else if is_installed {
                "已安装"
            } else if is_downloaded {
                "安装"
            } else {
                "下载"
            };
            let status_color = if is_downloading || is_installing {
                theme.warning
            } else if is_downloaded || is_installed {
                theme.success
            } else {
                theme.muted_foreground
            };
            let download_entry = entry.clone();
            let install_entry = entry.clone();
            let capabilities = if entry.capabilities.is_empty() {
                "无额外能力".to_owned()
            } else {
                entry
                    .capabilities
                    .iter()
                    .map(|capability| capability.as_str())
                    .collect::<Vec<_>>()
                    .join("、")
            };
            v_flex()
                .id(row_id.clone())
                .debug_selector(move || row_id.clone())
                .w_full()
                .min_w_0()
                .gap(px(4.0))
                .p(px(10.0))
                .bg(theme.background)
                .border_1()
                .border_color(theme.border)
                .rounded(px(6.0))
                .child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .flex_wrap()
                        .items_start()
                        .gap(px(8.0))
                        .child(
                            div()
                                .mt(px(4.0))
                                .size(px(8.0))
                                .flex_none()
                                .rounded(px(4.0))
                                .bg(theme.muted_foreground),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.foreground)
                                        .child(entry.name.clone()),
                                )
                                .child(div().text_xs().text_color(theme.muted_foreground).child(
                                    format!(
                                        "插件 {} · 版本 {} · API {}",
                                        entry.id, entry.version, entry.api_version
                                    ),
                                )),
                        )
                        .child(
                            h_flex()
                                .flex_none()
                                .max_w_full()
                                .flex_wrap()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(status_color)
                                        .debug_selector(move || {
                                            format!("plugin-package-catalog-status-{package_key}")
                                        })
                                        .child(if is_downloading {
                                            "下载中"
                                        } else if is_installing {
                                            "安装中"
                                        } else if is_installed {
                                            "已安装"
                                        } else if is_downloaded {
                                            "已下载"
                                        } else {
                                            "待下载"
                                        }),
                                )
                                .child(
                                    crate::clickable_button(if is_downloaded && !is_installed {
                                        install_id.clone()
                                    } else {
                                        download_id.clone()
                                    })
                                    .debug_selector(move || {
                                        if is_downloaded && !is_installed {
                                            install_id.clone()
                                        } else {
                                            download_id.clone()
                                        }
                                    })
                                    .xsmall()
                                    .outline()
                                    .label(action_label)
                                    .disabled(is_downloading || is_installing || is_installed)
                                    .on_click(cx.listener(
                                        move |view, _, _, cx| {
                                            if is_downloaded && !is_installed {
                                                view.install_downloaded_package(
                                                    install_entry.clone(),
                                                    cx,
                                                );
                                            } else {
                                                view.download_package(download_entry.clone(), cx);
                                            }
                                        },
                                    )),
                                ),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .whitespace_normal()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("运行时：进程 · 能力：{capabilities}")),
                )
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .debug_selector(move || url_id.clone())
                        .child(format!("下载地址：{}", entry.package_url)),
                )
                .into_any_element()
        })
        .collect::<Vec<_>>();
    let body = if rows.is_empty() {
        v_flex()
            .w_full()
            .min_w_0()
            .gap(px(8.0))
            .when_some(download_error, |content, error| {
                content.child(crate::settings_view::settings_error_notice(
                    "plugin-package-catalog-error",
                    "插件包操作失败",
                    error.to_owned(),
                    theme,
                ))
            })
            .child(render_empty_state(
                "plugin-package-catalog",
                IconName::Folder,
                "暂无本地目录候选",
                "将已校验的 catalog.json 放入插件状态目录后会显示在这里；当前不会联网或执行代码。",
                theme,
            ))
            .into_any_element()
    } else {
        v_flex()
            .w_full()
            .min_w_0()
            .gap(px(8.0))
            .when_some(download_error, |content, error| {
                content.child(crate::settings_view::settings_error_notice(
                    "plugin-package-catalog-error",
                    "插件包操作失败",
                    error.to_owned(),
                    theme,
                ))
            })
            .children(rows)
            .into_any_element()
    };

    crate::pulse_ui::pulse_panel(cx)
        .id("plugin-package-catalog")
        .debug_selector(|| "plugin-package-catalog".into())
        .gap(px(12.0))
        .child(crate::pulse_ui::pulse_display_heading(
            "可安装插件目录",
            18.0,
            cx,
        ))
        .child(div().text_xs().text_color(theme.muted_foreground).child(
            "只展示本地清单中的已校验元数据；下载和安装使用独立缓存与状态校验，不会自动执行。",
        ))
        .child(body)
}

fn package_download_key(entry: &PluginPackageCatalogEntry) -> String {
    format!("{}-{}", entry.id, entry.version)
}

fn package_download_destination(
    download_root: &Path,
    entry: &PluginPackageCatalogEntry,
) -> PathBuf {
    let sequence = PACKAGE_DOWNLOAD_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    download_root.join(format!(
        "{}-{}-{}-{sequence}.tar.gz",
        entry.id,
        entry.version,
        std::process::id(),
    ))
}

fn render_catalog(
    entries: &[PluginCatalogEntry],
    cx: &mut Context<PluginDiagnosticsView>,
) -> impl IntoElement {
    let theme = cx.theme();
    let rows = entries
        .iter()
        .map(|entry| {
            let row_id = format!("plugin-catalog-{}-{}", entry.plugin_id, entry.entry_id);
            let platform = match (entry.desktop, entry.web) {
                (true, true) => "桌面原生 / Web",
                (true, false) => "桌面原生",
                (false, true) => "Web",
                (false, false) => "未声明平台",
            };
            let capabilities = if entry.capabilities.is_empty() {
                "无额外能力".to_owned()
            } else {
                entry
                    .capabilities
                    .iter()
                    .map(|capability| capability.as_str())
                    .collect::<Vec<_>>()
                    .join("、")
            };
            v_flex()
                .id(row_id.clone())
                .debug_selector(move || row_id.clone())
                .w_full()
                .min_w_0()
                .gap(px(4.0))
                .p(px(10.0))
                .bg(theme.background)
                .border_1()
                .border_color(theme.border)
                .rounded(px(6.0))
                .child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .items_start()
                        .gap(px(8.0))
                        .child(
                            div()
                                .mt(px(4.0))
                                .size(px(8.0))
                                .flex_none()
                                .rounded(px(4.0))
                                .bg(theme.success),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.foreground)
                                        .child(entry.name.clone()),
                                )
                                .child(div().text_xs().text_color(theme.muted_foreground).child(
                                    format!(
                                        "插件 {} · 入口 {} · API {}",
                                        entry.plugin_id, entry.entry_id, entry.api_version
                                    ),
                                )),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("平台：{platform} · 能力：{capabilities}")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "数据：{} · 状态：{}",
                            entry.data_handling, entry.acceptance
                        )),
                )
                .into_any_element()
        })
        .collect::<Vec<_>>();
    let body = if rows.is_empty() {
        render_empty_state(
            "plugin-catalog",
            IconName::Info,
            "暂无第一方入口",
            "宿主审核后随应用发布的静态入口会显示在这里。",
            cx.theme(),
        )
        .into_any_element()
    } else {
        v_flex()
            .w_full()
            .min_w_0()
            .gap(px(8.0))
            .children(rows)
            .into_any_element()
    };

    crate::pulse_ui::pulse_panel(cx)
        .id("plugin-catalog")
        .debug_selector(|| "plugin-catalog".into())
        .gap(px(12.0))
        .child(crate::pulse_ui::pulse_display_heading(
            "第一方工具目录",
            18.0,
            cx,
        ))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("只展示随应用发布并由宿主审查的静态入口；目录不会执行未信任代码。"),
        )
        .child(body)
}

fn render_summary(
    total: usize,
    ready: usize,
    failed: usize,
    cx: &mut Context<PluginDiagnosticsView>,
) -> impl IntoElement {
    let failed_detail = (failed > 0).then(|| format!("{failed} 个需要处理"));
    crate::pulse_ui::pulse_panel(cx)
        .id("plugin-summary")
        .debug_selector(|| "plugin-summary".into())
        .gap(px(10.0))
        .child(crate::pulse_ui::pulse_display_heading("运行概览", 18.0, cx))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .flex_wrap()
                .gap(px(8.0))
                .child(crate::pulse_ui::pulse_metric_card(
                    "插件总数",
                    total.to_string(),
                    Some("个"),
                    None::<&str>,
                    cx,
                ))
                .child(crate::pulse_ui::pulse_metric_card(
                    "已就绪",
                    ready.to_string(),
                    Some("个"),
                    None::<&str>,
                    cx,
                ))
                .child(crate::pulse_ui::pulse_metric_card(
                    "待处理",
                    failed.to_string(),
                    Some("个"),
                    failed_detail,
                    cx,
                )),
        )
}

fn render_available_entries(
    tools: &[Arc<dyn ramag_domain::Tool>],
    cx: &mut Context<PluginDiagnosticsView>,
) -> impl IntoElement {
    let theme = cx.theme();
    let rows = tools
        .iter()
        .map(|tool| {
            let meta = tool.meta();
            h_flex()
                .id(format!("plugin-available-{}", meta.id))
                .debug_selector({
                    let id = format!("plugin-available-{}", meta.id);
                    move || id.clone()
                })
                .w_full()
                .min_w_0()
                .items_start()
                .gap(px(8.0))
                .child(
                    div()
                        .mt(px(4.0))
                        .size(px(8.0))
                        .flex_none()
                        .rounded(px(4.0))
                        .bg(theme.success),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(div().text_sm().child(meta.name.clone()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!("入口 ID：{}", meta.id)),
                        ),
                )
                .into_any_element()
        })
        .collect::<Vec<_>>();
    let body = if rows.is_empty() {
        render_empty_state(
            "plugin-available",
            IconName::Folder,
            "当前没有可用入口",
            "插件注册完成且通过宿主初始化后，会显示在这里。",
            cx.theme(),
        )
        .into_any_element()
    } else {
        v_flex()
            .w_full()
            .min_w_0()
            .gap(px(10.0))
            .children(rows)
            .into_any_element()
    };

    crate::pulse_ui::pulse_panel(cx)
        .id("plugin-available")
        .debug_selector(|| "plugin-available".into())
        .gap(px(12.0))
        .child(crate::pulse_ui::pulse_display_heading("可用入口", 18.0, cx))
        .child(body)
}

fn render_diagnostics(
    diagnostics: &[PluginDiagnostic],
    cx: &mut Context<PluginDiagnosticsView>,
) -> impl IntoElement {
    let rows = diagnostics
        .iter()
        .map(|diagnostic| render_diagnostic_row(diagnostic, cx))
        .collect::<Vec<_>>();
    let body = if rows.is_empty() {
        render_empty_state(
            "plugin-diagnostics",
            IconName::Info,
            "尚未记录插件状态",
            "插件启动、失败和恢复信息会显示在这里。",
            cx.theme(),
        )
        .into_any_element()
    } else {
        v_flex()
            .w_full()
            .min_w_0()
            .gap(px(10.0))
            .children(rows)
            .into_any_element()
    };

    crate::pulse_ui::pulse_panel(cx)
        .id("plugin-state-list")
        .debug_selector(|| "plugin-state-list".into())
        .gap(px(12.0))
        .child(crate::pulse_ui::pulse_display_heading("插件状态", 18.0, cx))
        .child(body)
}

/// 为插件诊断页的空列表提供一致的图标、层级和下一步说明。
fn render_empty_state(
    section: &'static str,
    icon: IconName,
    title: &'static str,
    guidance: impl Into<SharedString>,
    theme: &gpui_kit::component::Theme,
) -> impl IntoElement {
    let state_id = format!("{section}-empty-state");
    let heading_id = format!("{section}-empty-heading");
    let guidance_id = format!("{section}-empty-guidance");
    v_flex()
        .id(state_id.clone())
        .debug_selector(move || state_id.clone())
        .w_full()
        .items_center()
        .gap(px(6.0))
        .px(px(16.0))
        .py(px(18.0))
        .child(
            h_flex()
                .id(heading_id.clone())
                .debug_selector(move || heading_id.clone())
                .items_center()
                .gap(px(8.0))
                .child(Icon::new(icon).small().text_color(theme.muted_foreground))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .child(title),
                ),
        )
        .child(
            div()
                .id(guidance_id.clone())
                .debug_selector(move || guidance_id.clone())
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(guidance.into()),
        )
}

fn render_diagnostic_row(
    diagnostic: &PluginDiagnostic,
    cx: &mut Context<PluginDiagnosticsView>,
) -> gpui_kit::AnyElement {
    let descriptor = &diagnostic.descriptor;
    let theme = cx.theme();
    let state_color = state_color(
        diagnostic.state,
        theme.success,
        theme.warning,
        theme.danger,
        theme.muted_foreground,
    );
    let row_id = format!("plugin-state-{}", descriptor.id);
    let failure_id = format!("{row_id}-failure");
    let entry_count = descriptor.entry_descriptors().len();
    let mut row =
        v_flex()
            .id(row_id.clone())
            .debug_selector(move || row_id.clone())
            .w_full()
            .min_w_0()
            .p(px(12.0))
            .gap(px(8.0))
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded(px(6.0))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_start()
                    .gap(px(8.0))
                    .child(
                        div()
                            .mt(px(4.0))
                            .size(px(8.0))
                            .flex_none()
                            .rounded(px(4.0))
                            .bg(state_color),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(descriptor.name.clone()),
                            )
                            .child(div().text_xs().text_color(theme.muted_foreground).child(
                                format!(
                                    "{} · {entry_count} 个入口 · 主入口 ID：{}",
                                    descriptor.id, descriptor.entry_id
                                ),
                            )),
                    )
                    .child(div().flex_none().child(
                        crate::pulse_ui::pulse_status_badge_with_label(
                            plugin_pulse_status(diagnostic.state),
                            state_label(diagnostic.state),
                            cx,
                        ),
                    )),
            );
    if let Some(failure) = &diagnostic.failure {
        let failure_id_for_selector = failure_id.clone();
        row = row.child(
            h_flex()
                .id(failure_id)
                .debug_selector(move || failure_id_for_selector.clone())
                .w_full()
                .min_w_0()
                .items_start()
                .gap(px(8.0))
                .p(px(8.0))
                .border_1()
                .border_color(theme.danger.opacity(0.28))
                .rounded(px(6.0))
                .bg(theme.danger.opacity(0.06))
                .child(
                    Icon::new(IconName::CircleX)
                        .small()
                        .text_color(theme.danger),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(theme.danger)
                                .child(format!("{}失败", stage_label(failure.stage))),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(failure.message.clone()),
                        ),
                ),
        );
    }
    row.into_any_element()
}

fn state_label(state: PluginState) -> &'static str {
    match state {
        PluginState::Registered => "已注册",
        PluginState::Initializing => "初始化中",
        PluginState::Ready => "已就绪",
        PluginState::ShuttingDown => "关闭中",
        PluginState::Failed => "失败",
        PluginState::Unloaded => "已卸载",
    }
}

fn plugin_pulse_status(state: PluginState) -> crate::pulse_ui::PulseStatus {
    match state {
        PluginState::Ready => crate::pulse_ui::PulseStatus::Current,
        PluginState::Failed => crate::pulse_ui::PulseStatus::Failed,
        PluginState::Unloaded => crate::pulse_ui::PulseStatus::Unavailable,
        PluginState::Registered | PluginState::Initializing | PluginState::ShuttingDown => {
            crate::pulse_ui::PulseStatus::Warming
        }
    }
}

fn state_color(
    state: PluginState,
    success: gpui_kit::Hsla,
    warning: gpui_kit::Hsla,
    danger: gpui_kit::Hsla,
    muted: gpui_kit::Hsla,
) -> gpui_kit::Hsla {
    match state {
        PluginState::Ready => success,
        PluginState::Registered | PluginState::Initializing | PluginState::ShuttingDown => warning,
        PluginState::Failed => danger,
        PluginState::Unloaded => muted,
    }
}

fn stage_label(stage: PluginLifecycleStage) -> &'static str {
    match stage {
        PluginLifecycleStage::Registration => "注册",
        PluginLifecycleStage::Initialize => "初始化",
        PluginLifecycleStage::Shutdown => "关闭",
    }
}

#[cfg(test)]
#[path = "plugin_diagnostics_tests.rs"]
mod tests;
