//! One searchable tool manager for adding and removing Home and sidebar entries.

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Selectable, Sizable as _,
    button::ButtonVariants as _,
    h_flex,
    input::{Input, InputState},
    popover::Popover,
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

const MANAGER_WIDTH: f32 = 340.0;
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

    /// Change only this surface's entry preference and refresh both surfaces.
    fn set_tool_visible(&mut self, id: &str, visible: bool, cx: &mut Context<Self>) {
        let changed = match self.surface {
            ToolDragSurface::Home => self.registry.set_home_visible(id, visible),
            ToolDragSurface::ActivityBar => self.registry.set_sidebar_visible(id, visible),
        };
        if changed {
            persist_tool_visibility(&self.registry, cx);
            notify_tool_layout_changed(cx);
            cx.notify();
        }
    }

    fn is_visible(&self, visibility: ToolVisibility) -> bool {
        match self.surface {
            ToolDragSurface::Home => visibility.home,
            ToolDragSurface::ActivityBar => visibility.sidebar,
        }
    }

    fn render_tool_row(&self, tool: Arc<dyn Tool>, cx: &mut Context<Self>) -> impl IntoElement {
        let meta = tool.meta();
        let id = meta.id.clone();
        let surface = self.surface;
        let selector = format!("tool-manager-row-{}-{id}", surface_key(surface));
        let action_selector = format!("tool-manager-action-{}-{id}", surface_key(surface));
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
            .is_some_and(|visibility| self.is_visible(visibility));
        let action_label = if is_visible { "移除" } else { "添加" };
        let action_tooltip = format!(
            "{}{}入口",
            if is_visible { "从" } else { "添加到" },
            surface_name(surface)
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
                crate::clickable_button(action_selector.clone())
                    .debug_selector(move || action_selector.clone())
                    .small()
                    .flex_none()
                    .label(action_label)
                    .tooltip(action_tooltip)
                    .when(is_visible, |button| button.outline())
                    .when(!is_visible, |button| button.primary())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_tool_visible(&id, !is_visible, cx);
                    })),
            )
    }
}

impl Render for ToolSurfaceManager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tools = self
            .registry
            .list()
            .into_iter()
            .filter(|tool| tool_matches(tool.meta(), &self.query))
            .collect::<Vec<_>>();
        let visible_count = self
            .registry
            .list()
            .iter()
            .filter(|tool| {
                self.registry
                    .visibility(&tool.meta().id)
                    .is_some_and(|visibility| self.is_visible(visibility))
            })
            .count();
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
                            .child(format!("管理{}工具", surface_name(self.surface))),
                    )
                    .child(div().text_xs().text_color(muted).child(format!(
                        "当前显示 {visible_count} 个。操作只影响此处入口，不会删除工具或数据。"
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
                    .max_h(px(MANAGER_MAX_HEIGHT - 108.0))
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

/// Attach the same searchable add/remove manager to the surface's fixed control.
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

fn tool_matches(meta: &ramag_domain::ToolMeta, query: &str) -> bool {
    query.is_empty()
        || [&meta.name, &meta.id, &meta.description]
            .iter()
            .any(|value| value.to_lowercase().contains(query))
}

#[cfg(test)]
#[path = "tool_surface_manager_tests.rs"]
mod tests;
