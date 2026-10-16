//! One searchable manager for Home and sidebar entry visibility.

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Selectable, Sizable as _,
    button::ButtonVariants as _,
    h_flex,
    input::{Input, InputState},
    popover::Popover,
    switch::Switch,
    v_flex,
};
use gpui_kit::{
    Anchor, App, AppContext as _, BorrowAppContext as _, ClickEvent, Context, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, ScrollHandle,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px,
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

    /// Change only the selected surface and refresh both views.
    fn toggle_tool_visibility(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(current) = self.registry.visibility(id) else {
            return;
        };
        let changed = match self.surface {
            ToolDragSurface::Home => self.registry.set_home_visible(id, !current.home),
            ToolDragSurface::ActivityBar => self.registry.set_sidebar_visible(id, !current.sidebar),
        };
        if changed {
            persist_tool_visibility(&self.registry, cx);
            notify_tool_layout_changed(cx);
            cx.notify();
        }
    }

    fn select_surface(&mut self, surface: ToolDragSurface, cx: &mut Context<Self>) {
        if self.surface != surface {
            self.surface = surface;
            cx.notify();
        }
    }

    fn render_tool_row(&self, tool: Arc<dyn Tool>, cx: &mut Context<Self>) -> impl IntoElement {
        let meta = tool.meta();
        let id = meta.id.clone();
        let surface = self.surface;
        let selector = format!("tool-manager-row-{}-{id}", surface_key(surface));
        let toggle_selector = format!("tool-manager-toggle-{}-{id}", surface_key(surface));
        let name = meta.name.clone();
        let description = meta.description.clone();
        let icon = crate::activity_bar::ActivityBar::icon_for_meta(meta);
        let accent = cx.theme().accent;
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let hover = cx.theme().muted;
        let is_visible = self
            .registry
            .visibility(&id)
            .is_some_and(|visibility| surface_visible(visibility, surface));
        let action_name = format!("在{}显示", surface_name(surface));
        let toggle_id = toggle_selector.clone();
        let visibility_switch: Switch = crate::clickable_switch(toggle_selector.clone())
            .checked(is_visible)
            .on_click(cx.listener(move |this, _: &bool, _, cx| {
                this.toggle_tool_visibility(&id, cx);
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
            .child(
                h_flex()
                    .items_center()
                    .gap(px(6.0))
                    .flex_none()
                    .child(div().text_xs().text_color(muted).child(action_name))
                    .child(
                        div()
                            .id(toggle_id.clone())
                            .debug_selector(move || toggle_id.clone())
                            .items_center()
                            .child(visibility_switch),
                    ),
            )
    }
}

impl Render for ToolSurfaceManager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let all_tools = self.registry.list();
        let home_count = all_tools
            .iter()
            .filter(|tool| {
                self.registry
                    .visibility(&tool.meta().id)
                    .is_some_and(|visibility| visibility.home)
            })
            .count();
        let sidebar_count = all_tools
            .iter()
            .filter(|tool| {
                self.registry
                    .visibility(&tool.meta().id)
                    .is_some_and(|visibility| visibility.sidebar)
            })
            .count();
        let tools = all_tools
            .into_iter()
            .filter(|tool| tool_matches(tool.meta(), &self.query))
            .collect::<Vec<_>>();
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
        let rows = if tools.is_empty() {
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
            tools
                .into_iter()
                .map(|tool| self.render_tool_row(tool, cx).into_any_element())
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
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child("管理工具入口"),
                    )
                    .child(div().text_xs().text_color(muted).child(format!(
                        "首页 {home_count} 个 · 侧边栏 {sidebar_count} 个。开关只控制入口显示，工具和数据会保留。"
                    ))),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap(px(6.0))
                    .child(surface_tab(
                        ToolDragSurface::Home,
                        home_count,
                        self.surface == ToolDragSurface::Home,
                        cx,
                    ))
                    .child(surface_tab(
                        ToolDragSurface::ActivityBar,
                        sidebar_count,
                        self.surface == ToolDragSurface::ActivityBar,
                        cx,
                    )),
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
                    .child(v_flex().w_full().gap(px(5.0)).p(px(5.0)).children(rows)),
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

/// Attach the same searchable visibility manager to the surface's fixed control.
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

fn surface_tab(
    surface: ToolDragSurface,
    visible_count: usize,
    selected: bool,
    cx: &mut Context<ToolSurfaceManager>,
) -> impl IntoElement {
    let selector = format!("tool-manager-surface-{}", surface_key(surface));
    let label = format!("{} {visible_count}", surface_name(surface));
    crate::clickable_button(selector.clone())
        .debug_selector(move || selector.clone())
        .small()
        .flex_1()
        .label(label)
        .when(selected, |button| button.primary())
        .when(!selected, |button| button.outline())
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.select_surface(surface, cx);
        }))
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
