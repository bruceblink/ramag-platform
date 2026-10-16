//! Compact add picker and per-entry removal actions for Home and the sidebar.

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Selectable, Sizable as _,
    button::ButtonVariants as _,
    h_flex,
    input::{Input, InputState},
    menu::PopupMenuItem,
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

use crate::PointerDropdownMenu as _;
use crate::tool_layout::{
    ToolDragSurface, ToolLayoutGlobal, notify_tool_layout_changed, persist_tool_visibility,
};

const PICKER_WIDTH: f32 = 320.0;
const PICKER_MAX_HEIGHT: f32 = 420.0;

pub(crate) struct ToolSurfacePicker {
    registry: Arc<ToolRegistry>,
    surface: ToolDragSurface,
    search: Entity<InputState>,
    query: String,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl ToolSurfacePicker {
    pub(crate) fn new(
        registry: Arc<ToolRegistry>,
        surface: ToolDragSurface,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.update_default_global::<ToolLayoutGlobal, _>(|_, _| {});
        let search =
            cx.new(|cx| crate::bounded_search_input(window, cx).placeholder("搜索要添加的工具"));
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

    /// Add only the current surface entry, then persist and redraw both surfaces.
    fn add_tool(&mut self, id: &str, cx: &mut Context<Self>) {
        let changed = match self.surface {
            ToolDragSurface::Home => self.registry.set_home_visible(id, true),
            ToolDragSurface::ActivityBar => self.registry.set_sidebar_visible(id, true),
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
        let selector = format!("tool-picker-row-{}-{id}", surface_key(self.surface));
        let name = meta.name.clone();
        let description = meta.description.clone();
        let icon = crate::activity_bar::ActivityBar::icon_for_meta(meta);
        let accent = cx.theme().accent;
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let hover = cx.theme().muted;
        let add_icon = Icon::new(IconName::Plus).small().text_color(muted);

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
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.add_tool(&id, cx)))
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
            .child(add_icon)
    }
}

impl Render for ToolSurfacePicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tools = self
            .registry
            .list()
            .into_iter()
            .filter(|tool| {
                self.registry
                    .visibility(&tool.meta().id)
                    .is_some_and(|visibility| !self.is_visible(visibility))
            })
            .filter(|tool| tool_matches(tool.meta(), &self.query))
            .collect::<Vec<_>>();
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let empty_message = if self.query.is_empty() {
            "所有工具都已添加"
        } else {
            "没有匹配的工具"
        };
        let rows = if tools.is_empty() {
            vec![
                div()
                    .w_full()
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
            .id("tool-picker-body")
            .debug_selector(|| "tool-picker-body".into())
            .w(px(PICKER_WIDTH))
            .max_h(px(PICKER_MAX_HEIGHT))
            .gap(px(8.0))
            .child(
                div()
                    .px(px(2.0))
                    .text_xs()
                    .text_color(muted)
                    .child("添加到当前界面"),
            )
            .child(
                div()
                    .id("tool-picker-search")
                    .debug_selector(|| "tool-picker-search".into())
                    .w_full()
                    .child(
                        Input::new(&self.search)
                            .small()
                            .prefix(Icon::new(IconName::Search).small().text_color(muted)),
                    ),
            )
            .child(
                div()
                    .id("tool-picker-scroll")
                    .debug_selector(|| "tool-picker-scroll".into())
                    .w_full()
                    .max_h(px(PICKER_MAX_HEIGHT - 90.0))
                    .border_1()
                    .border_color(border.opacity(0.65))
                    .rounded(px(8.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(v_flex().w_full().gap(px(5.0)).p(px(5.0)).children(rows)),
            )
    }
}

pub(crate) fn create_tool_surface_picker(
    registry: Arc<ToolRegistry>,
    surface: ToolDragSurface,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ToolSurfacePicker> {
    cx.new(|cx| ToolSurfacePicker::new(registry, surface, window, cx))
}

/// Attach a searchable, anchored add picker to the surface's fixed plus button.
pub(crate) fn tool_picker_popover(
    picker: Entity<ToolSurfacePicker>,
    surface: ToolDragSurface,
    trigger: impl Selectable + IntoElement + 'static,
) -> impl IntoElement {
    let anchor = match surface {
        ToolDragSurface::Home => Anchor::BottomRight,
        ToolDragSurface::ActivityBar => Anchor::RightCenter,
    };
    Popover::new(format!("tool-picker-{}", surface_key(surface)))
        .anchor(anchor)
        .trigger(trigger)
        .content(move |_, _, _| picker.clone())
}

/// Remove one surface entry without changing the tool registration or its other entry.
fn remove_surface_entry(registry: &ToolRegistry, surface: ToolDragSurface, id: &str, cx: &mut App) {
    let changed = match surface {
        ToolDragSurface::Home => registry.set_home_visible(id, false),
        ToolDragSurface::ActivityBar => registry.set_sidebar_visible(id, false),
    };
    if changed {
        persist_tool_visibility(registry, cx);
        notify_tool_layout_changed(cx);
    }
}

/// Show a discoverable per-tool menu with the single entry-removal action.
pub(crate) fn tool_entry_actions(
    registry: Arc<ToolRegistry>,
    surface: ToolDragSurface,
    id: String,
) -> impl IntoElement + 'static {
    let selector = format!("tool-entry-actions-{}-{id}", surface_key(surface));
    let debug_selector = selector.clone();
    let label = format!(
        "从{}移除",
        match surface {
            ToolDragSurface::Home => "首页",
            ToolDragSurface::ActivityBar => "侧边栏",
        }
    );
    let menu_item_selector = format!("tool-entry-remove-{}-{id}", surface_key(surface));
    let menu_item_debug_selector = menu_item_selector.clone();
    crate::clickable_button(selector)
        .debug_selector(move || debug_selector.clone())
        .small()
        .p_0()
        .size(px(16.0))
        .ghost()
        .icon(Icon::new(IconName::Ellipsis).size(px(13.0)))
        .tooltip(format!("更多操作：{label}"))
        .on_click(|_, _, cx| cx.stop_propagation())
        .pointer_dropdown_menu(move |menu, _, _| {
            let registry = registry.clone();
            let id = id.clone();
            let label = label.clone();
            let selector = menu_item_selector.clone();
            let debug_selector = menu_item_debug_selector.clone();
            menu.item(
                PopupMenuItem::element(move |_, _| {
                    let debug_selector = debug_selector.clone();
                    div()
                        .id(selector.clone())
                        .debug_selector(move || debug_selector.clone())
                        .w_full()
                        .child(label.clone())
                })
                .on_click(move |_: &ClickEvent, _, cx| {
                    remove_surface_entry(&registry, surface, &id, cx);
                }),
            )
        })
}

fn surface_key(surface: ToolDragSurface) -> &'static str {
    match surface {
        ToolDragSurface::Home => "home",
        ToolDragSurface::ActivityBar => "sidebar",
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
