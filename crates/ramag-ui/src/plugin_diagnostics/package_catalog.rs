use super::*;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, h_flex, input::Input, v_flex,
};
use gpui_kit::{IntoElement, ParentElement, Styled, div, px};
use ramag_app::{
    PluginInstallation, PluginPackageCache, PluginPackageCachedArchive, PluginPackageCatalogEntry,
};

static PACKAGE_DOWNLOAD_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 按当前目录重新核验下载缓存；摘要、路径或文件异常时交给调用方决定降级方式。
pub(super) fn restore_catalog_cache(
    cache: Option<PluginPackageCache>,
    entries: &[PluginPackageCatalogEntry],
) -> Result<Vec<PluginPackageCachedArchive>, String> {
    cache
        .map(|cache| cache.restore(entries).map_err(|error| error.to_string()))
        .transpose()
        .map(|packages| packages.unwrap_or_default())
}

/// 展示目录源输入和手动刷新动作；不在渲染或启动时访问网络。
pub(super) fn render_catalog_source_controls(
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
pub(super) fn render_package_catalog(
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

pub(super) fn package_download_key(entry: &PluginPackageCatalogEntry) -> String {
    format!("{}-{}", entry.id, entry.version)
}

pub(super) fn package_download_destination(
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
