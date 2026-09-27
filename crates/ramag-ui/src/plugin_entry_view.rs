//! 多入口插件的标准原生 GPUI 元数据入口。

use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::{Context, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px};
use ramag_domain::{PluginEntryDataKind, PluginEntryDescriptor};

/// 在没有专用工作台视图时，提供统一的原生入口说明面板。
///
/// 该面板只展示已校验的入口边界，不执行插件逻辑；真正的输入提交和任务取消由后续
/// `PLAT-005` 运行时接入，避免在没有执行适配器时伪造成功结果。
pub struct StandardPluginEntryView {
    plugin_id: String,
    entry: PluginEntryDescriptor,
}

impl StandardPluginEntryView {
    pub(crate) fn new(plugin_id: impl Into<String>, entry: PluginEntryDescriptor) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            entry,
        }
    }
}

impl Render for StandardPluginEntryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let entry_id = self.entry.id.clone();
        let input_kind = data_kind_label(self.entry.input.kind);
        let output_kind = data_kind_label(self.entry.output.kind);
        let input_limit = format_bytes(self.entry.input.max_bytes);
        let output_limit = format_bytes(self.entry.output.max_bytes);

        v_flex()
            .id(format!("plugin-entry-view-{}", self.entry.id))
            .debug_selector({
                let id = format!("plugin-entry-view-{}", self.entry.id);
                move || id.clone()
            })
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(24.0))
            .gap(px(16.0))
            .child(
                v_flex()
                    .id("plugin-entry-header")
                    .debug_selector(|| "plugin-entry-header".into())
                    .w_full()
                    .min_w_0()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(self.entry.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("插件 {} · 入口 {}", self.plugin_id, entry_id)),
                    )
                    .when(!self.entry.description.is_empty(), |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(theme.secondary_foreground)
                                .child(self.entry.description.clone()),
                        )
                    }),
            )
            .child(
                h_flex()
                    .id("plugin-entry-contract")
                    .debug_selector(|| "plugin-entry-contract".into())
                    .w_full()
                    .min_w_0()
                    .gap(px(12.0))
                    .child(contract_card(
                        "输入",
                        input_kind,
                        input_limit,
                        theme.border,
                        theme.secondary,
                        theme.muted_foreground,
                    ))
                    .child(contract_card(
                        "输出",
                        output_kind,
                        output_limit,
                        theme.border,
                        theme.secondary,
                        theme.muted_foreground,
                    )),
            )
            .child(
                v_flex()
                    .id("plugin-entry-unbound")
                    .debug_selector(|| "plugin-entry-unbound".into())
                    .w_full()
                    .min_w_0()
                    .p(px(16.0))
                    .gap(px(6.0))
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(8.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child("原生入口已登记"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("当前入口只有清单描述，执行适配将在入口任务运行时接入。"),
                    ),
            )
    }
}

fn contract_card(
    title: &'static str,
    kind: &'static str,
    limit: String,
    border: gpui_kit::Hsla,
    background: gpui_kit::Hsla,
    muted: gpui_kit::Hsla,
) -> impl IntoElement {
    v_flex()
        .id(format!("plugin-entry-contract-{title}"))
        .debug_selector(move || format!("plugin-entry-contract-{title}"))
        .flex_1()
        .min_w_0()
        .p(px(14.0))
        .gap(px(4.0))
        .bg(background)
        .border_1()
        .border_color(border)
        .rounded(px(8.0))
        .child(div().text_xs().text_color(muted).child(title))
        .child(div().text_sm().child(kind))
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child(format!("最大 {limit}")),
        )
}

fn data_kind_label(kind: PluginEntryDataKind) -> &'static str {
    match kind {
        PluginEntryDataKind::Text => "文本",
        PluginEntryDataKind::Json => "JSON",
        PluginEntryDataKind::Binary => "二进制",
    }
}

fn format_bytes(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{} MiB", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{} KiB", bytes / 1024)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
#[path = "plugin_entry_view_tests.rs"]
mod tests;
