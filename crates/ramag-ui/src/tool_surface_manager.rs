//! Searchable management panel for Home and sidebar tool entries.

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _, WindowExt as _,
    button::ButtonVariants as _,
    h_flex,
    input::{Input, InputState},
    v_flex,
};
use gpui_kit::{
    App, AppContext as _, ClickEvent, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, ScrollHandle, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, prelude::FluentBuilder as _, px,
};
use ramag_app::{ToolRegistry, ToolVisibility};
use ramag_domain::Tool;

use crate::activity_bar::ActivityBar;
use crate::tool_layout::{ToolDragSurface, notify_tool_layout_changed, persist_tool_visibility};

const MAX_DIALOG_HEIGHT: f32 = 660.0;
const MIN_DIALOG_BODY_HEIGHT: f32 = 250.0;
const TOOL_ROW_HEIGHT: f32 = 72.0;

pub(crate) struct ToolSurfaceManager {
    registry: Arc<ToolRegistry>,
    surface: ToolDragSurface,
    search: Entity<InputState>,
    query: String,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl ToolSurfaceManager {
    fn new(
        registry: Arc<ToolRegistry>,
        surface: ToolDragSurface,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| {
            crate::bounded_search_input(window, cx).placeholder("搜索工具名称、说明或 ID")
        });
        let subscriptions = vec![cx.observe(&search, |this, _, cx| {
            this.query = this.search.read(cx).value().trim().to_lowercase();
            cx.notify();
        })];
        Self {
            registry,
            surface,
            search,
            query: String::new(),
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Change only this surface, then persist and redraw both surfaces from the registry.
    fn toggle_visibility(&mut self, tool_id: &str, cx: &mut Context<Self>) {
        let Some(visibility) = self.registry.visibility(tool_id) else {
            return;
        };
        let changed = match self.surface {
            ToolDragSurface::Home => self.registry.set_home_visible(tool_id, !visibility.home),
            ToolDragSurface::ActivityBar => self
                .registry
                .set_sidebar_visible(tool_id, !visibility.sidebar),
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

    fn surface_name(&self) -> &'static str {
        match self.surface {
            ToolDragSurface::Home => "首页",
            ToolDragSurface::ActivityBar => "侧边栏",
        }
    }

    fn render_group(
        &self,
        title: &'static str,
        tools: Vec<Arc<dyn Tool>>,
        visible: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + 'static {
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let mut rows = Vec::with_capacity(tools.len());
        for tool in tools {
            rows.push(self.render_tool_row(tool, visible, cx));
        }
        let count = rows.len();
        let mut list = v_flex()
            .id(if visible {
                "tool-manager-current-list"
            } else {
                "tool-manager-available-list"
            })
            .debug_selector(move || {
                if visible {
                    "tool-manager-current-list".into()
                } else {
                    "tool-manager-available-list".into()
                }
            })
            .w_full()
            .border_1()
            .border_color(border.opacity(0.82))
            .rounded(px(8.0))
            .overflow_hidden();
        if rows.is_empty() {
            list = list.child(
                div()
                    .px(px(14.0))
                    .py(px(16.0))
                    .text_sm()
                    .text_color(muted)
                    .child(if self.query.is_empty() {
                        if visible {
                            "此处没有显示的工具"
                        } else {
                            "没有可添加的工具"
                        }
                    } else {
                        "没有匹配的工具"
                    }),
            );
        } else {
            list = list.children(rows);
        }

        v_flex()
            .id(if visible {
                "tool-manager-group-current"
            } else {
                "tool-manager-group-available"
            })
            .debug_selector(move || {
                if visible {
                    "tool-manager-group-current".into()
                } else {
                    "tool-manager-group-available".into()
                }
            })
            .w_full()
            .gap(px(7.0))
            .child(
                h_flex()
                    .items_center()
                    .gap(px(7.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("{count} 个")),
                    ),
            )
            .child(list)
    }

    fn render_tool_row(
        &self,
        tool: Arc<dyn Tool>,
        visible: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + 'static {
        let meta = tool.meta();
        let id = meta.id.clone();
        let selector = format!("tool-manager-row-{}-{id}", surface_key(self.surface));
        let action_selector = format!("tool-manager-action-{}-{id}", surface_key(self.surface));
        let action_label = if visible {
            format!("从{}移除", self.surface_name())
        } else {
            format!("添加到{}", self.surface_name())
        };
        let name = meta.name.clone();
        let description = meta.description.clone();
        let icon = ActivityBar::icon_for_meta(meta);
        let muted = cx.theme().muted_foreground;
        let accent = cx.theme().accent;
        let action = crate::clickable_button(action_selector.clone())
            .debug_selector(move || action_selector.clone())
            .small()
            .flex_none()
            .label(action_label)
            .when(visible, |button| button.outline())
            .when(!visible, |button| button.primary())
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.toggle_visibility(&id, cx);
            }));

        h_flex()
            .id(selector.clone())
            .debug_selector(move || selector.clone())
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_center()
            .gap(px(10.0))
            .px(px(12.0))
            .py(px(9.0))
            .child(icon.size(px(18.0)).text_color(accent))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(3.0))
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
            .child(action)
    }
}

impl Render for ToolSurfaceManager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut current = Vec::new();
        let mut available = Vec::new();
        for tool in self.registry.list() {
            let Some(visibility) = self.registry.visibility(&tool.meta().id) else {
                continue;
            };
            if !tool_matches(tool.meta(), &self.query) {
                continue;
            }
            if self.is_visible(visibility) {
                current.push(tool);
            } else {
                available.push(tool);
            }
        }

        let layout = crate::dialog_layout::DialogLayout::new(window, 620.0);
        let matching_tool_count = current.len() + available.len();
        let desired_height = px(
            ((matching_tool_count.min(7) as f32 * TOOL_ROW_HEIGHT) + 200.0)
                .max(MIN_DIALOG_BODY_HEIGHT),
        );
        let body_height = desired_height
            .min(layout.body_height)
            .min(px(MAX_DIALOG_HEIGHT));
        let muted = cx.theme().muted_foreground;
        let surface_name = self.surface_name();

        v_flex()
            .id("tool-manager-body")
            .debug_selector(|| "tool-manager-body".into())
            .w_full()
            .h(body_height)
            .min_h_0()
            .gap(px(11.0))
            .child(div().text_xs().text_color(muted).child(format!(
                "只修改{surface_name}入口。工具和另一处入口会保留。"
            )))
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
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        v_flex()
                            .w_full()
                            .gap(px(15.0))
                            .child(self.render_group("当前显示", current, true, cx))
                            .child(self.render_group("可添加", available, false, cx)),
                    ),
            )
    }
}

/// Open the same searchable manager from either tool surface.
pub(crate) fn open_tool_surface_manager(
    registry: Arc<ToolRegistry>,
    surface: ToolDragSurface,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ToolSurfaceManager> {
    let panel = cx.new(|cx| ToolSurfaceManager::new(registry, surface, window, cx));
    let title = format!(
        "管理{}工具",
        match surface {
            ToolDragSurface::Home => "首页",
            ToolDragSurface::ActivityBar => "侧边栏",
        }
    );
    window.open_dialog(cx, {
        let panel = panel.clone();
        move |dialog, window, _| {
            let panel = panel.clone();
            let layout = crate::dialog_layout::DialogLayout::new(window, 620.0);
            let done = crate::clickable_button("tool-manager-done")
                .debug_selector(|| "tool-manager-done".into())
                .small()
                .primary()
                .label("完成")
                .on_click(|_: &ClickEvent, window, cx| window.close_dialog(cx));
            dialog
                .title(crate::closable_dialog_title(
                    "tool-manager-close",
                    title.clone(),
                    |_, _| {},
                ))
                .close_button(false)
                .w(layout.width)
                .max_h(layout.height)
                .margin_top(layout.top)
                .content(move |content, _, _| content.child(panel.clone()))
                .footer(done)
        }
    });
    panel
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
