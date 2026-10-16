//! Searchable add and remove actions for one tool surface at a time.

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Selectable, Sizable as _, h_flex,
    input::{Input, InputState},
    popover::Popover,
    v_flex,
};
use gpui_kit::{
    Anchor, App, AppContext as _, BorrowAppContext as _, ClickEvent, Context, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, ScrollHandle,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px,
};
use ramag_app::{ToolRegistry, ToolVisibility};
use ramag_domain::Tool;

use crate::tool_layout::{
    ToolDragSurface, ToolLayoutGlobal, notify_tool_layout_changed, persist_tool_visibility,
};

const MANAGER_WIDTH: f32 = 360.0;
const MANAGER_MAX_HEIGHT: f32 = 440.0;

pub(crate) struct ToolSurfaceManager {
    registry: Arc<ToolRegistry>,
    surface: ToolDragSurface,
    search: Entity<InputState>,
    query: String,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl ToolSurfaceManager {
    pub(crate) fn new(
        registry: Arc<ToolRegistry>,
        surface: ToolDragSurface,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.update_default_global::<ToolLayoutGlobal, _>(|_, _| {});
        let search = cx.new(|cx| {
            crate::bounded_search_input(window, cx).placeholder("搜索工具名称、说明或 ID")
        });
        let subscriptions = vec![
            cx.observe(&search, |this, _, cx| {
                this.query = this.search.read(cx).value().trim().to_lowercase();
                cx.notify();
            }),
            cx.observe_global::<ToolLayoutGlobal>(|_, cx| cx.notify()),
        ];
        Self {
            registry,
            surface,
            search,
            query: String::new(),
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Set only this manager's surface preference, then refresh both views.
    fn set_tool_visibility(&mut self, id: &str, visible: bool, cx: &mut Context<Self>) {
        let Some(current) = self.registry.visibility(id) else {
            return;
        };
        let changed = match self.surface {
            ToolDragSurface::Home => {
                current.home != visible && self.registry.set_home_visible(id, visible)
            }
            ToolDragSurface::ActivityBar => {
                current.sidebar != visible && self.registry.set_sidebar_visible(id, visible)
            }
        };
        if changed {
            persist_tool_visibility(&self.registry, cx);
            notify_tool_layout_changed(cx);
            cx.notify();
        }
    }

    fn render_tool_row(
        &self,
        tool: Arc<dyn Tool>,
        is_visible: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let meta = tool.meta();
        let id = meta.id.clone();
        let surface = self.surface;
        let selector = format!("tool-manager-row-{}-{id}", surface_key(surface));
        let action = if is_visible { "remove" } else { "add" };
        let action_selector = format!("tool-manager-{action}-{}-{id}", surface_key(surface));
        let name = meta.name.clone();
        let description = meta.description.clone();
        let icon = crate::activity_bar::ActivityBar::icon_for_meta(meta);
        let muted = cx.theme().muted_foreground;
        let accent = cx.theme().accent;
        let border = cx.theme().border;
        let hover = cx.theme().muted;
        let action_label = if is_visible { "移除" } else { "添加" };
        let action_id = action_selector.clone();
        let next_visible = !is_visible;
        let action_button = crate::clickable_button(action_selector.clone())
            .debug_selector(move || action_id.clone())
            .small()
            .outline()
            .label(action_label)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.set_tool_visibility(&id, next_visible, cx);
            }));

        h_flex()
            .id(selector.clone())
            .debug_selector(move || selector.clone())
            .w_full()
            .min_w_0()
            .items_center()
            .gap(px(10.0))
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(border.opacity(0.65))
            .hover(move |style| style.bg(hover))
            .child(icon.size(px(18.0)).flex_none().text_color(accent))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(description),
                    ),
            )
            .child(h_flex().items_center().flex_none().child(action_button))
    }
}

impl Render for ToolSurfaceManager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let surface = self.surface;
        let all_tools = self.registry.list();
        let visible_count = all_tools
            .iter()
            .filter(|tool| self.is_visible(&tool.meta().id))
            .count();
        let available_count = all_tools.len().saturating_sub(visible_count);
        let matching_tools = all_tools
            .into_iter()
            .filter(|tool| tool_matches(tool.meta(), &self.query))
            .collect::<Vec<_>>();
        let added_tools = matching_tools
            .iter()
            .filter(|tool| self.is_visible(&tool.meta().id))
            .cloned()
            .collect::<Vec<_>>();
        let available_tools = matching_tools
            .iter()
            .filter(|tool| !self.is_visible(&tool.meta().id))
            .cloned()
            .collect::<Vec<_>>();
        let added_count = added_tools.len();
        let available_match_count = available_tools.len();
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let window_width = f32::from(window.bounds().size.width);
        let right_reserve = match self.surface {
            ToolDragSurface::Home => 20.0,
            ToolDragSurface::ActivityBar => crate::activity_bar::BAR_WIDTH + 20.0,
        };
        let manager_width = (window_width - right_reserve).clamp(240.0, MANAGER_WIDTH);
        let empty_message = if self.registry.count() == 0 {
            "没有已注册的工具"
        } else {
            "没有匹配的工具"
        };
        let added_empty_message = if self.query.is_empty() {
            "还没有已添加的工具"
        } else {
            "没有匹配的已添加工具"
        };
        let available_empty_message = if self.query.is_empty() {
            "所有工具都已添加"
        } else {
            "没有匹配的可添加工具"
        };
        let rows = if matching_tools.is_empty() {
            vec![
                div()
                    .w_full()
                    .id("tool-manager-empty")
                    .debug_selector(|| "tool-manager-empty".into())
                    .px(px(12.0))
                    .py(px(16.0))
                    .text_sm()
                    .text_color(muted)
                    .child(empty_message)
                    .into_any_element(),
            ]
        } else {
            let added_rows: Vec<_> = if added_tools.is_empty() {
                let selector = format!("tool-manager-added-empty-{}", surface_key(surface));
                let debug_selector = selector.clone();
                vec![
                    div()
                        .id(selector)
                        .debug_selector(move || debug_selector.clone())
                        .px(px(10.0))
                        .py(px(8.0))
                        .text_xs()
                        .text_color(muted)
                        .child(added_empty_message)
                        .into_any_element(),
                ]
            } else {
                added_tools
                    .into_iter()
                    .map(|tool| self.render_tool_row(tool, true, cx).into_any_element())
                    .collect()
            };
            let available_rows: Vec<_> = if available_tools.is_empty() {
                let selector = format!("tool-manager-available-empty-{}", surface_key(surface));
                let debug_selector = selector.clone();
                vec![
                    div()
                        .id(selector)
                        .debug_selector(move || debug_selector.clone())
                        .px(px(10.0))
                        .py(px(8.0))
                        .text_xs()
                        .text_color(muted)
                        .child(available_empty_message)
                        .into_any_element(),
                ]
            } else {
                available_tools
                    .into_iter()
                    .map(|tool| self.render_tool_row(tool, false, cx).into_any_element())
                    .collect()
            };
            let added_section_selector =
                format!("tool-manager-added-section-{}", surface_key(surface));
            let added_section_debug = added_section_selector.clone();
            let available_section_selector =
                format!("tool-manager-available-section-{}", surface_key(surface));
            let available_section_debug = available_section_selector.clone();
            vec![
                v_flex()
                    .id(added_section_selector)
                    .debug_selector(move || added_section_debug.clone())
                    .w_full()
                    .gap(px(5.0))
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .px(px(4.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .child(format!("已添加到{}", surface_name(surface))),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(added_count.to_string()),
                            ),
                    )
                    .children(added_rows)
                    .into_any_element(),
                v_flex()
                    .id(available_section_selector)
                    .debug_selector(move || available_section_debug.clone())
                    .w_full()
                    .gap(px(5.0))
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .px(px(4.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .child(format!("可添加到{}", surface_name(surface))),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(available_match_count.to_string()),
                            ),
                    )
                    .children(available_rows)
                    .into_any_element(),
            ]
        };

        let title_selector = format!("tool-manager-title-{}", surface_key(surface));
        let title_id = title_selector.clone();
        let current_surface = surface;

        v_flex()
            .id("tool-manager-body")
            .debug_selector(|| "tool-manager-body".into())
            .w(px(manager_width))
            .max_h(px(MANAGER_MAX_HEIGHT))
            .gap(px(8.0))
            .child(
                v_flex()
                    .gap(px(3.0))
                    .child(
                        div()
                            .id(title_selector)
                            .debug_selector(move || title_id.clone())
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(format!("管理{}工具", surface_name(current_surface))),
                    )
                    .child(div().text_xs().text_color(muted).child(format!(
                        "已添加 {visible_count} 个 · 可添加 {available_count} 个"
                    )))
                    .child(div().text_xs().text_color(muted).child(format!(
                        "移除只隐藏{}入口，工具和数据会保留。",
                        surface_name(current_surface)
                    ))),
            )
            .child(
                div()
                    .id("tool-manager-search")
                    .debug_selector(|| "tool-manager-search".into())
                    .w_full()
                    .child(
                        Input::new(&self.search)
                            .small()
                            .prefix(Icon::new(IconName::Search).small().text_color(muted)),
                    ),
            )
            .child(
                div()
                    .id("tool-manager-scroll")
                    .debug_selector(|| "tool-manager-scroll".into())
                    .w_full()
                    .max_h(px(MANAGER_MAX_HEIGHT - 148.0))
                    .border_1()
                    .border_color(border.opacity(0.65))
                    .rounded(px(8.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(v_flex().w_full().gap(px(10.0)).p(px(5.0)).children(rows)),
            )
    }
}

pub(crate) fn create_tool_surface_manager(
    registry: Arc<ToolRegistry>,
    surface: ToolDragSurface,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ToolSurfaceManager> {
    cx.new(|cx| ToolSurfaceManager::new(registry, surface, window, cx))
}

/// Attach the surface-specific add/remove manager to its fixed control.
pub(crate) fn tool_surface_manager_popover(
    manager: Entity<ToolSurfaceManager>,
    surface: ToolDragSurface,
    trigger: impl Selectable + IntoElement + 'static,
) -> impl IntoElement {
    let anchor = match surface {
        ToolDragSurface::Home => Anchor::BottomRight,
        ToolDragSurface::ActivityBar => Anchor::RightCenter,
    };
    Popover::new(format!("tool-manager-{}", surface_key(surface)))
        .anchor(anchor)
        .trigger(trigger)
        .content(move |_, _, _| manager.clone())
}

fn surface_key(surface: ToolDragSurface) -> &'static str {
    match surface {
        ToolDragSurface::Home => "home",
        ToolDragSurface::ActivityBar => "sidebar",
    }
}

fn surface_name(surface: ToolDragSurface) -> &'static str {
    match surface {
        ToolDragSurface::Home => "首页",
        ToolDragSurface::ActivityBar => "侧边栏",
    }
}

fn surface_visible(visibility: ToolVisibility, surface: ToolDragSurface) -> bool {
    match surface {
        ToolDragSurface::Home => visibility.home,
        ToolDragSurface::ActivityBar => visibility.sidebar,
    }
}

impl ToolSurfaceManager {
    fn is_visible(&self, id: &str) -> bool {
        self.registry
            .visibility(id)
            .is_some_and(|visibility| surface_visible(visibility, self.surface))
    }
}

fn tool_matches(meta: &ramag_domain::ToolMeta, query: &str) -> bool {
    query.is_empty()
        || [&meta.name, &meta.id, &meta.description]
            .iter()
            .any(|value| value.to_lowercase().contains(query))
}

#[cfg(test)]
#[path = "tool_surface_manager_tests.rs"]
mod tests;
