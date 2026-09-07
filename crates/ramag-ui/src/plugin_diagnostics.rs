use std::sync::Arc;

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use ramag_app::{PluginDiagnostic, PluginLifecycleStage, PluginState, StaticPluginHost};

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
        let theme = cx.theme();
        let diagnostics = self.host.diagnostics();
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
            .gap(px(16.0))
            .child(render_summary(
                diagnostics.len(),
                ready_count,
                failed_count,
                theme.border,
                theme.muted_foreground,
                theme.danger,
            ))
            .child(render_available_entries(
                &available,
                theme.border,
                theme.muted_foreground,
                theme.success,
            ))
            .child(render_diagnostics(
                &diagnostics,
                theme.border,
                theme.muted_foreground,
                theme.foreground,
                theme.success,
                theme.warning,
                theme.danger,
            ))
    }
}

fn render_summary(
    total: usize,
    ready: usize,
    failed: usize,
    border: gpui::Hsla,
    muted: gpui::Hsla,
    danger: gpui::Hsla,
) -> impl IntoElement {
    v_flex()
        .id("plugin-summary")
        .debug_selector(|| "plugin-summary".into())
        .w_full()
        .min_w_0()
        .p(px(16.0))
        .gap(px(8.0))
        .border_1()
        .border_color(border)
        .rounded(px(8.0))
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child("运行概览"),
        )
        .child(
            div()
                .text_sm()
                .text_color(muted)
                .child(format!("共记录 {total} 个插件，已就绪 {ready} 个")),
        )
        .when(failed > 0, |summary| {
            summary.child(
                div()
                    .text_sm()
                    .text_color(danger)
                    .child(format!("有 {failed} 个插件需要处理")),
            )
        })
}

fn render_available_entries(
    tools: &[Arc<dyn ramag_domain::Tool>],
    border: gpui::Hsla,
    muted: gpui::Hsla,
    success: gpui::Hsla,
) -> impl IntoElement {
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
                        .bg(success),
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
                                .text_color(muted)
                                .child(format!("入口 ID：{}", meta.id)),
                        ),
                )
                .into_any_element()
        })
        .collect::<Vec<_>>();
    let body = if rows.is_empty() {
        div()
            .text_sm()
            .text_color(muted)
            .child("当前没有可用入口。")
            .into_any_element()
    } else {
        v_flex()
            .w_full()
            .min_w_0()
            .gap(px(10.0))
            .children(rows)
            .into_any_element()
    };

    v_flex()
        .id("plugin-available")
        .debug_selector(|| "plugin-available".into())
        .w_full()
        .min_w_0()
        .p(px(16.0))
        .gap(px(12.0))
        .border_1()
        .border_color(border)
        .rounded(px(8.0))
        .child(
            div()
                .text_base()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child("可用入口"),
        )
        .child(body)
}

fn render_diagnostics(
    diagnostics: &[PluginDiagnostic],
    border: gpui::Hsla,
    muted: gpui::Hsla,
    foreground: gpui::Hsla,
    success: gpui::Hsla,
    warning: gpui::Hsla,
    danger: gpui::Hsla,
) -> impl IntoElement {
    let rows = diagnostics
        .iter()
        .map(|diagnostic| {
            render_diagnostic_row(
                diagnostic, border, muted, foreground, success, warning, danger,
            )
        })
        .collect::<Vec<_>>();
    let body = if rows.is_empty() {
        div()
            .text_sm()
            .text_color(muted)
            .child("尚未记录插件。")
            .into_any_element()
    } else {
        v_flex()
            .w_full()
            .min_w_0()
            .gap(px(10.0))
            .children(rows)
            .into_any_element()
    };

    v_flex()
        .id("plugin-state-list")
        .debug_selector(|| "plugin-state-list".into())
        .w_full()
        .min_w_0()
        .p(px(16.0))
        .gap(px(12.0))
        .border_1()
        .border_color(border)
        .rounded(px(8.0))
        .child(
            div()
                .text_base()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child("插件状态"),
        )
        .child(body)
}

fn render_diagnostic_row(
    diagnostic: &PluginDiagnostic,
    border: gpui::Hsla,
    muted: gpui::Hsla,
    foreground: gpui::Hsla,
    success: gpui::Hsla,
    warning: gpui::Hsla,
    danger: gpui::Hsla,
) -> gpui::AnyElement {
    let descriptor = &diagnostic.descriptor;
    let state_color = state_color(diagnostic.state, success, warning, danger, muted);
    let row_id = format!("plugin-state-{}", descriptor.id);
    let mut row = v_flex()
        .id(row_id.clone())
        .debug_selector(move || row_id.clone())
        .w_full()
        .min_w_0()
        .p(px(12.0))
        .gap(px(8.0))
        .border_1()
        .border_color(border)
        .rounded(px(8.0))
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
                                .text_color(foreground)
                                .child(descriptor.name.clone()),
                        )
                        .child(div().text_xs().text_color(muted).child(format!(
                            "{} · 入口 ID：{}",
                            descriptor.id, descriptor.entry_id
                        ))),
                )
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(state_color)
                        .child(state_label(diagnostic.state)),
                ),
        );
    if let Some(failure) = &diagnostic.failure {
        row = row.child(
            div()
                .w_full()
                .min_w_0()
                .text_xs()
                .text_color(danger)
                .child(format!(
                    "{}失败：{}",
                    stage_label(failure.stage),
                    failure.message
                )),
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

fn state_color(
    state: PluginState,
    success: gpui::Hsla,
    warning: gpui::Hsla,
    danger: gpui::Hsla,
    muted: gpui::Hsla,
) -> gpui::Hsla {
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
