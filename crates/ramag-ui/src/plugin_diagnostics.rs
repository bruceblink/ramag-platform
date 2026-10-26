use std::sync::Arc;

use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::{Context, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px};
use ramag_app::{
    PluginCatalogEntry, PluginDiagnostic, PluginLifecycleStage, PluginState, StaticPluginHost,
};

/// 展示静态插件生命周期、注册错误和当前可用入口。
pub(crate) struct PluginDiagnosticsView {
    host: Arc<StaticPluginHost>,
}

impl PluginDiagnosticsView {
    pub(crate) fn new(host: Arc<StaticPluginHost>) -> Self {
        Self { host }
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
            .child(render_catalog(&catalog, cx))
            .child(render_available_entries(&available, cx))
            .child(render_diagnostics(&diagnostics, cx))
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
    guidance: &'static str,
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
                .child(guidance),
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
