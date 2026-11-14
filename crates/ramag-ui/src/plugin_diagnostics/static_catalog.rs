use super::*;

use gpui_kit::component::{ActiveTheme as _, IconName, h_flex, v_flex};
use gpui_kit::{IntoElement, ParentElement, Styled, div, px};
use ramag_app::PluginCatalogEntry;

pub(super) fn render_catalog(
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

pub(super) fn render_summary(
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
