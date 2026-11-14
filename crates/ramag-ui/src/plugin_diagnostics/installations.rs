use super::*;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, button::ButtonVariants as _,
    h_flex, v_flex,
};
use gpui_kit::{IntoElement, ParentElement, Styled, div, px};
use ramag_app::PluginInstallation;
use ramag_domain::PluginId;

/// 安装列表的动作状态只读快照，避免渲染函数接收过多彼此相关的参数。
pub(super) struct InstallationRenderState<'a> {
    pub(super) pending_uninstall: Option<&'a PluginId>,
    pub(super) operation_error: Option<&'a str>,
    pub(super) installing_archive: bool,
    pub(super) package_operation_busy: bool,
}

pub(super) fn render_installations(
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
