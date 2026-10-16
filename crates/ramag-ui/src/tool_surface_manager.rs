//! Searchable controls for showing tools on Home and the activity bar.

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Selectable, Sizable as _, h_flex,
    input::{Input, InputState},
    popover::Popover,
    switch::Switch,
    v_flex,
};
use gpui_kit::{
    Anchor, App, AppContext as _, BorrowAppContext as _, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, ScrollHandle, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, div, px,
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
    anchor_surface: ToolDragSurface,
    search: Entity<InputState>,
    query: String,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl ToolSurfaceManager {
    pub(crate) fn new(
        registry: Arc<ToolRegistry>,
        anchor_surface: ToolDragSurface,
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
            anchor_surface,
            search,
            query: String::new(),
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Update one surface preference and refresh both views without changing the other surface.
    fn set_tool_visibility(
        &mut self,
        id: &str,
        surface: ToolDragSurface,
        visible: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self.registry.visibility(id) else {
            return;
        };
        let changed = match surface {
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
        visibility: ToolVisibility,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let meta = tool.meta();
        let id = meta.id.clone();
        let selector = format!("tool-manager-row-{id}");
        let name = meta.name.clone();
        let description = meta.description.clone();
        let icon = crate::activity_bar::ActivityBar::icon_for_meta(meta);
        let muted = cx.theme().muted_foreground;
        let accent = cx.theme().accent;
        let border = cx.theme().border;
        let hover = cx.theme().muted;
        let home_toggle =
            self.render_surface_toggle(&id, ToolDragSurface::Home, "首页", visibility.home, cx);
        let sidebar_toggle = self.render_surface_toggle(
            &id,
            ToolDragSurface::ActivityBar,
            "侧边栏",
            visibility.sidebar,
            cx,
        );

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
            .child(
                v_flex()
                    .flex_none()
                    .gap(px(4.0))
                    .child(home_toggle)
                    .child(sidebar_toggle),
            )
    }

    /// Keep both placement controls beside each tool so users can see and change either surface.
    fn render_surface_toggle(
        &self,
        id: &str,
        surface: ToolDragSurface,
        label: &'static str,
        visible: bool,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let selector = format!("tool-manager-{}-{id}", surface_key(surface));
        let tool_id = id.to_owned();
        let tooltip = match (surface, visible) {
            (ToolDragSurface::Home, true) => "已显示在首页，点击隐藏",
            (ToolDragSurface::Home, false) => "点击显示在首页",
            (ToolDragSurface::ActivityBar, true) => "已显示在侧边栏，点击隐藏",
            (ToolDragSurface::ActivityBar, false) => "点击显示在侧边栏",
        };
        let switch: Switch = crate::clickable_switch(selector.clone())
            .checked(visible)
            .tooltip(tooltip)
            .on_click(cx.listener(move |this, next_visible: &bool, _, cx| {
                this.set_tool_visibility(&tool_id, surface, *next_visible, cx);
            }));
        let control_selector = format!("tool-manager-{}-control-{id}", surface_key(surface));
        let control_debug = control_selector.clone();
        let muted = cx.theme().muted_foreground;

        h_flex()
            .id(control_selector)
            .debug_selector(move || control_debug.clone())
            .w(px(96.0))
            .items_center()
            .justify_between()
            .gap(px(6.0))
            .child(div().text_xs().text_color(muted).child(label))
            .child(switch)
            .into_any_element()
    }
}

impl Render for ToolSurfaceManager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut all_tools = self.registry.list();
        all_tools.sort_by_key(|tool| tool.meta().name.to_lowercase());
        let home_count = all_tools
            .iter()
            .filter(|tool| self.is_visible(&tool.meta().id, ToolDragSurface::Home))
            .count();
        let sidebar_count = all_tools
            .iter()
            .filter(|tool| self.is_visible(&tool.meta().id, ToolDragSurface::ActivityBar))
            .count();
        let matching_tools = all_tools
            .into_iter()
            .filter(|tool| tool_matches(tool.meta(), &self.query))
            .collect::<Vec<_>>();
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let window_width = f32::from(window.bounds().size.width);
        let right_reserve = match self.anchor_surface {
            ToolDragSurface::Home => 20.0,
            ToolDragSurface::ActivityBar => crate::activity_bar::BAR_WIDTH + 20.0,
        };
        let manager_width = (window_width - right_reserve).clamp(240.0, MANAGER_WIDTH);
        let empty_message = if self.registry.count() == 0 {
            "没有已注册的工具"
        } else {
            "没有匹配的工具"
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
            matching_tools
                .into_iter()
                .map(|tool| {
                    let visibility = self
                        .registry
                        .visibility(&tool.meta().id)
                        .unwrap_or_default();
                    self.render_tool_row(tool, visibility, cx)
                        .into_any_element()
                })
                .collect()
        };

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
                            .id("tool-manager-title")
                            .debug_selector(|| "tool-manager-title".into())
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child("工具显示位置"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("首页 {home_count} 个 · 侧边栏 {sidebar_count} 个")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("关闭入口只会隐藏对应位置；工具、配置和数据会保留。"),
                    ),
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
                    .child(v_flex().w_full().gap(px(6.0)).p(px(5.0)).children(rows)),
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

/// Attach the searchable tool placement manager to its Home or activity-bar control.
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

fn surface_visible(visibility: ToolVisibility, surface: ToolDragSurface) -> bool {
    match surface {
        ToolDragSurface::Home => visibility.home,
        ToolDragSurface::ActivityBar => visibility.sidebar,
    }
}

impl ToolSurfaceManager {
    fn is_visible(&self, id: &str, surface: ToolDragSurface) -> bool {
        self.registry
            .visibility(id)
            .is_some_and(|visibility| surface_visible(visibility, surface))
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
