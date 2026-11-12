use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _, button::ButtonVariants as _, h_flex, v_flex,
};
use gpui_kit::{
    Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, prelude::*, px,
};
use ramag_app::{
    PluginCatalogEntry, PluginDiagnostic, PluginInstallation, PluginInstallationState,
    PluginInstallationStore, PluginLifecycleStage, PluginPackageInstaller, PluginState,
    StaticPluginHost,
};
use ramag_domain::PluginId;

/// 展示插件包状态、静态插件生命周期、注册错误和当前可用入口。
pub(crate) struct PluginDiagnosticsView {
    host: Arc<StaticPluginHost>,
    installations: Vec<PluginInstallation>,
    installation_store: Option<Arc<PluginInstallationStore>>,
    package_installer: Option<Arc<PluginPackageInstaller>>,
    pending_uninstall: Option<PluginId>,
    installation_error: Option<String>,
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
            installation_store: None,
            package_installer: None,
            pending_uninstall: None,
            installation_error: None,
        }
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
                self.pending_uninstall.as_ref(),
                self.installation_error.as_deref(),
                cx,
            ))
            .child(render_catalog(&catalog, cx))
            .child(render_available_entries(&available, cx))
            .child(render_diagnostics(&diagnostics, cx))
    }
}

fn render_installations(
    installations: &[PluginInstallation],
    can_toggle: bool,
    can_uninstall: bool,
    pending_uninstall: Option<&PluginId>,
    load_error: Option<&str>,
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
                div()
                    .text_xs()
                    .text_color(theme.danger)
                    .child(format!("最近错误：{message}"))
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
        if let Some(error) = load_error {
            render_empty_state(
                "plugin-installations",
                IconName::CircleX,
                "插件状态读取失败",
                error.to_owned(),
                theme,
            )
            .into_any_element()
        } else {
            render_empty_state(
                "plugin-installations",
                IconName::Folder,
                "暂无可安装插件",
                "安装包状态会显示在这里；当前页面不会启动外部代码。",
                theme,
            )
            .into_any_element()
        }
    } else {
        v_flex()
            .w_full()
            .min_w_0()
            .gap(px(8.0))
            .when_some(load_error, |content, error| {
                content.child(
                    div()
                        .text_xs()
                        .text_color(theme.danger)
                        .child(error.to_owned()),
                )
            })
            .children(rows)
            .into_any_element()
    };
    let confirmation = pending_uninstall.and_then(|plugin_id| {
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
        .child(crate::pulse_ui::pulse_display_heading(
            "已安装插件",
            18.0,
            cx,
        ))
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
