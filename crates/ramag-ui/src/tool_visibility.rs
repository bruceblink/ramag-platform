use std::sync::Arc;

use gpui_kit::component::{ActiveTheme, Icon, h_flex, switch::Switch, v_flex};
use gpui_kit::{Context, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px};
use ramag_app::{ToolRegistry, ToolVisibility};

use crate::activity_bar::ActivityBar;
use crate::tool_layout::{notify_tool_layout_changed, persist_tool_visibility};

/// Manage independent visibility for each registered tool on Home and the activity bar.
pub(crate) struct ToolVisibilityView {
    registry: Arc<ToolRegistry>,
}

impl ToolVisibilityView {
    pub(crate) fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }

    /// Apply one surface change, persist the new preference, and refresh both consumers.
    fn toggle_surface(&self, tool_id: &str, home: bool, cx: &mut Context<Self>) {
        let Some(current) = self.registry.visibility(tool_id) else {
            return;
        };
        let changed = if home {
            self.registry.set_home_visible(tool_id, !current.home)
        } else {
            self.registry.set_sidebar_visible(tool_id, !current.sidebar)
        };
        if changed {
            persist_tool_visibility(&self.registry, cx);
            notify_tool_layout_changed(cx);
            cx.notify();
        }
    }
}

impl Render for ToolVisibilityView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let tools = self.registry.list_all();
        let entries = tools
            .into_iter()
            .map(|tool| {
                let id = tool.meta().id.clone();
                let name = tool.meta().name.clone();
                let description = tool.meta().description.clone();
                let visibility = self.registry.visibility(&id).unwrap_or_default();
                render_tool_entry(
                    id,
                    name,
                    description,
                    visibility,
                    ActivityBar::icon_for_meta(tool.meta()),
                    cx,
                )
            })
            .collect::<Vec<_>>();

        v_flex()
            .id("tool-visibility")
            .debug_selector(|| "tool-visibility".into())
            .w_full()
            .min_w_0()
            .gap(px(12.0))
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("关闭入口只会隐藏对应位置。工具、配置和已保存数据都会保留。"),
            )
            .children(entries)
    }
}

fn render_tool_entry(
    id: String,
    name: String,
    description: String,
    visibility: ToolVisibility,
    icon: Icon,
    cx: &mut Context<ToolVisibilityView>,
) -> gpui_kit::AnyElement {
    let theme = cx.theme();
    let row_id = format!("settings-tool-entry-{id}");
    let home_id = format!("settings-tool-home-{id}");
    let sidebar_id = format!("settings-tool-sidebar-{id}");
    let home_tool_id = id.clone();
    let sidebar_tool_id = id;
    let border = theme.border;
    let background = theme.secondary;
    let foreground = theme.foreground;
    let muted = theme.muted_foreground;
    let accent = theme.accent;

    let home_switch: Switch = crate::clickable_switch(home_id.clone())
        .checked(visibility.home)
        .on_click(cx.listener(move |view, _: &bool, _, cx| {
            view.toggle_surface(&home_tool_id, true, cx);
        }));
    let sidebar_switch: Switch = crate::clickable_switch(sidebar_id.clone())
        .checked(visibility.sidebar)
        .on_click(cx.listener(move |view, _: &bool, _, cx| {
            view.toggle_surface(&sidebar_tool_id, false, cx);
        }));

    v_flex()
        .id(row_id.clone())
        .debug_selector(move || row_id.clone())
        .w_full()
        .min_w_0()
        .p(px(12.0))
        .gap(px(10.0))
        .border_1()
        .border_color(border.opacity(0.78))
        .rounded(px(6.0))
        .bg(background)
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .items_start()
                .gap(px(8.0))
                .child(icon.text_color(accent))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(div().text_sm().text_color(foreground).child(name))
                        .child(div().text_xs().text_color(muted).child(description)),
                ),
        )
        .child(
            h_flex()
                .w_full()
                .flex_wrap()
                .items_center()
                .gap(px(20.0))
                .child(surface_toggle("首页", home_id, home_switch, muted))
                .child(surface_toggle("侧边栏", sidebar_id, sidebar_switch, muted)),
        )
        .into_any_element()
}

fn surface_toggle(
    label: &'static str,
    id: String,
    switch: Switch,
    muted: gpui_kit::Hsla,
) -> impl IntoElement {
    h_flex()
        .id(id.clone())
        .debug_selector(move || id.clone())
        .items_center()
        .gap(px(6.0))
        .child(div().text_xs().text_color(muted).child(label))
        .child(switch)
}

#[cfg(test)]
#[path = "tool_visibility_tests.rs"]
mod tests;
