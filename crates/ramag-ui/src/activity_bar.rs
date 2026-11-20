//! 活动栏：管理工具入口、选中状态与拖拽排序。

mod item;

use item::{
    ActivityItemConfig, ActivityItemDecoration, activity_item, command_item_height, manager_button,
    tool_icon_path,
};

use gpui_kit::component::{
    ActiveTheme, Icon,
    animation::{EffectTransition, ease_in_out_cubic},
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::{
    App, AppContext as _, BorrowAppContext as _, ClickEvent, Context, DragMoveEvent, Entity,
    EventEmitter, FocusHandle, Global, IntoElement, KeyDownEvent, MouseButton, ParentElement,
    Render, ScrollHandle, SharedString, Styled, Subscription, Window, div, prelude::*, px,
};
use ramag_app::{StaticPluginHost, ToolRegistry, UpdateCheckResult};
use ramag_domain::ToolMeta;
use std::sync::Arc;

use crate::REORDER_TRANSITION_DURATION;
use crate::icons;
use crate::tool_layout::{
    ACTIVITY_ITEM_GAP, ACTIVITY_ITEM_HEIGHT, DRAGGED_ITEM_OPACITY, ToolDrag, ToolDragGlobal,
    ToolDragSurface, ToolDropSide, ToolLayoutGlobal, activity_drop_indicator,
    activity_drop_target_from_position, activity_reorder_animation_offset, clear_tool_drag,
    dragged_item_background, notify_tool_layout_changed, persist_tool_order,
    tool_drag_display_slots, tool_drag_state, tool_drop_index, update_tool_drop_target,
};
use crate::tool_surface_manager::{
    ToolSurfaceManager, create_tool_surface_manager, tool_surface_manager_popover,
};

#[derive(Debug, Clone, PartialEq)]
pub enum NavTarget {
    Home,
    Tool(String),
    Settings,
}

#[derive(Debug, Clone)]
pub enum NavEvent {
    Navigate(NavTarget),
    CloseNavigation,
}

/// 桌面侧栏宽度；也用于侧栏工具管理弹层的可用宽度计算。
pub(crate) const BAR_WIDTH: f32 = 196.0;
const COMPACT_BAR_WIDTH: f32 = 62.0;
const COMPACT_CONTENT_WIDTH: f32 = 54.0;
const WORKBENCH_COMPACT_MAX_WIDTH: f32 = 1100.0;

pub struct ActivityBar {
    registry: Arc<ToolRegistry>,
    plugin_host: Arc<StaticPluginHost>,
    selected: NavTarget,
    tool_manager: Option<Entity<ToolSurfaceManager>>,
    focus_handle: FocusHandle,
    last_rendered_slots: Vec<Option<String>>,
    /// 保留工具列表滚动位置；固定的首页和设置入口不参与滚动。
    tool_scroll: ScrollHandle,
    _update_indicator_subscription: Subscription,
    _tool_layout_subscription: Subscription,
    _tool_drag_subscription: Subscription,
}

/// 应用内更新角标状态；新版本可用时显示 1，否则隐藏。
#[derive(Clone, Copy, Default)]
pub(crate) struct UpdateIndicatorGlobal {
    pub(crate) available: bool,
}

impl Global for UpdateIndicatorGlobal {}

/// 将更新检查结果同步到设置入口角标。
pub fn sync_update_indicator(result: &UpdateCheckResult, cx: &mut App) {
    let available = indicator_value(result);
    cx.set_global(UpdateIndicatorGlobal { available });
}

fn indicator_value(result: &UpdateCheckResult) -> bool {
    match result {
        UpdateCheckResult::UpToDate { .. } => false,
        UpdateCheckResult::Available(_) => true,
        UpdateCheckResult::UnsupportedPlatform(_) => true,
    }
}

impl EventEmitter<NavEvent> for ActivityBar {}

impl ActivityBar {
    pub fn new(
        registry: Arc<ToolRegistry>,
        plugin_host: Arc<StaticPluginHost>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.update_default_global::<UpdateIndicatorGlobal, _>(|_, _| {});
        cx.update_default_global::<ToolLayoutGlobal, _>(|_, _| {});
        cx.update_default_global::<ToolDragGlobal, _>(|_, _| {});
        let update_indicator_subscription =
            cx.observe_global::<UpdateIndicatorGlobal>(|_, cx| cx.notify());
        let tool_layout_subscription = cx.observe_global::<ToolLayoutGlobal>(|_, cx| cx.notify());
        let tool_drag_subscription = cx.observe_global::<ToolDragGlobal>(|_, cx| cx.notify());
        let last_rendered_slots = registry
            .sidebar_list()
            .into_iter()
            .map(|tool| Some(tool.meta().id.clone()))
            .collect();
        let focus_handle = cx.focus_handle();
        Self {
            registry,
            plugin_host,
            selected: NavTarget::Home,
            tool_manager: None,
            focus_handle,
            last_rendered_slots,
            tool_scroll: ScrollHandle::new(),
            _update_indicator_subscription: update_indicator_subscription,
            _tool_layout_subscription: tool_layout_subscription,
            _tool_drag_subscription: tool_drag_subscription,
        }
    }

    pub fn set_selected(&mut self, target: NavTarget, cx: &mut Context<Self>) {
        if self.selected != target {
            self.selected = target;
            cx.notify();
        }
    }

    /// 桌面侧栏重新出现时恢复键盘焦点。
    pub(crate) fn focus_navigation(&self, window: &mut Window, cx: &mut App) {
        self.focus_handle.focus(window, cx);
    }

    fn navigate(&mut self, target: NavTarget, cx: &mut Context<Self>) {
        cx.emit(NavEvent::Navigate(target));
    }

    /// 在释放时一次性提交最终槽位，保存偏好并通知首页与侧栏同步重绘。
    fn complete_drop(&mut self, dragged_id: &str, fallback_index: usize, cx: &mut Context<Self>) {
        let target_index = tool_drop_index(ToolDragSurface::ActivityBar, fallback_index, cx);
        if self
            .registry
            .reorder_sidebar_to_index(dragged_id, target_index)
        {
            persist_tool_order(&self.registry, cx);
            notify_tool_layout_changed(cx);
        }
        clear_tool_drag(cx);
    }

    /// 首页复用此映射，保证入口图标一致。
    pub(crate) fn icon_for_tool(tool_id: &str) -> Icon {
        Icon::default().path(tool_icon_path(tool_id, None))
    }

    /// 使用工具声明的图标键创建入口图标；未声明时按稳定工具 ID 选择内置图标。
    pub(crate) fn icon_for_meta(meta: &ToolMeta) -> Icon {
        Icon::default().path(tool_icon_path(&meta.id, meta.icon.as_deref()))
    }

    /// 共享导航沿用原型的 API 花括号图标；首页继续使用产品原有图标。
    fn navigation_icon_for_meta(meta: &ToolMeta) -> Icon {
        if meta.id == "api" {
            Icon::new(gpui_kit::assets::IconName::Braces)
        } else {
            Self::icon_for_meta(meta)
        }
    }
}

impl Render for ActivityBar {
    /// 将持久化工具顺序绘制为侧栏入口，并同步拖拽占位线与来源入口反馈。
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tool_manager = match self.tool_manager.clone() {
            Some(manager) => manager,
            None => {
                let manager = create_tool_surface_manager(
                    self.registry.clone(),
                    ToolDragSurface::ActivityBar,
                    window,
                    cx,
                );
                self.tool_manager = Some(manager.clone());
                manager
            }
        };
        let (accent, sidebar_bg, border, foreground, muted_foreground) = {
            let theme = cx.theme();
            (
                theme.accent,
                theme.sidebar,
                theme.border,
                theme.foreground,
                theme.muted_foreground,
            )
        };
        let tools = self.registry.sidebar_list();
        let selected = self.selected.clone();
        let viewport_width = f32::from(window.viewport_size().width);
        let navigation_label = if viewport_width <= crate::shell::SHELL_NARROW_BREAKPOINT {
            "移动端工具导航"
        } else {
            "主导航"
        };
        let inline_compact = viewport_width > crate::shell::SHELL_NARROW_BREAKPOINT
            && viewport_width <= WORKBENCH_COMPACT_MAX_WIDTH
            && matches!(&selected, NavTarget::Tool(id) if id == "api" || id == "dbclient");
        let short_viewport = f32::from(window.viewport_size().height) < 360.0;
        let sidebar_width = if viewport_width <= crate::shell::SHELL_NARROW_BREAKPOINT {
            crate::shell::navigation_drawer_width(viewport_width)
        } else if inline_compact {
            COMPACT_BAR_WIDTH
        } else {
            BAR_WIDTH
        };
        let sidebar_content_width = if inline_compact {
            COMPACT_CONTENT_WIDTH
        } else {
            (sidebar_width - 20.0).max(0.0)
        };
        let label_visible = !inline_compact;
        // 拖拽落点、动画和可见入口使用同一行高，切换密度后仍保持排序准确。
        let item_height = crate::content_density(cx).row_height(ACTIVITY_ITEM_HEIGHT, 32.0);
        let tool_order = tools
            .iter()
            .map(|tool| tool.meta().id.clone())
            .collect::<Vec<_>>();
        let drag_state = tool_drag_state(cx);
        let display_slots =
            tool_drag_display_slots(&tool_order, ToolDragSurface::ActivityBar, &drag_state);
        let previous_slots =
            std::mem::replace(&mut self.last_rendered_slots, display_slots.clone());
        let layout_revision = cx.read_global::<ToolLayoutGlobal, _>(|state, _| state.revision);

        let target = drag_state
            .target
            .as_ref()
            .filter(|target| target.surface == ToolDragSurface::ActivityBar)
            .map(|target| (target.index, target.side));
        let item_count = tool_order.len();
        let update_available =
            cx.read_global::<UpdateIndicatorGlobal, _>(|state, _| state.available);
        let plugin_errors = self
            .plugin_host
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.failure.is_some())
            .count();
        let settings_badge = update_available || plugin_errors > 0;
        let settings_tooltip = if plugin_errors > 0 {
            format!("设置（{plugin_errors} 个插件需要处理）")
        } else {
            "设置".to_string()
        };
        let mut tool_items = Vec::with_capacity(display_slots.len());
        for slot in &display_slots {
            let Some(id) = slot else {
                continue;
            };

            let Some(tool) = tools.iter().find(|tool| tool.meta().id == *id) else {
                continue;
            };
            let Some(source_index) = tool_order.iter().position(|item| item == id) else {
                continue;
            };
            let id_for_click = id.clone();
            let label = tool.meta().name.clone();
            let is_selected = matches!(&selected, NavTarget::Tool(s) if s == id);
            let is_dragged = drag_state.dragged_id.as_deref() == Some(id.as_str());
            let mut item = activity_item(
                ActivityItemConfig {
                    id: format!("tool-{id}").into(),
                    icon: Self::navigation_icon_for_meta(tool.meta()),
                    is_selected,
                    height: item_height,
                    item_width: sidebar_content_width,
                    label_visible,
                    accent,
                    decoration: ActivityItemDecoration::new(
                        SharedString::from(label.clone()),
                        SharedString::from(label),
                        false,
                    ),
                    on_click: Box::new(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.navigate(NavTarget::Tool(id_for_click.clone()), cx);
                    })),
                    tool_drag: Some(ToolDrag { id: id.clone() }),
                    source_index: Some(source_index),
                    source_background: is_dragged.then(|| dragged_item_background(sidebar_bg)),
                },
                cx,
            );
            if is_dragged {
                item = item.opacity(DRAGGED_ITEM_OPACITY);
            }
            let offset =
                activity_reorder_animation_offset(&previous_slots, &display_slots, id, item_height);
            let item = if offset == px(0.0) {
                item.into_any_element()
            } else {
                EffectTransition::new(REORDER_TRANSITION_DURATION)
                    .ease(ease_in_out_cubic)
                    .slide_y(offset, px(0.0))
                    .apply(
                        item,
                        format!("activity-tool-reorder-{id}-{layout_revision}"),
                    )
                    .into_any_element()
            };
            tool_items.push(
                div()
                    .relative()
                    .w(px(sidebar_content_width))
                    .h(px(item_height))
                    .child(item)
                    .into_any_element(),
            );
        }

        let drop_end_index = item_count;
        let drop_end_target = drop_end_index.saturating_sub(1);
        let mut tool_list = v_flex()
            .id("activity-tool-list")
            .w(px(sidebar_content_width))
            .flex_none()
            .gap(px(ACTIVITY_ITEM_GAP))
            .relative()
            .on_drag_move(
                cx.listener(move |_, event: &DragMoveEvent<ToolDrag>, _, cx| {
                    let local_y = f32::from(event.event.position.y - event.bounds.top());
                    if let Some((target_index, side)) =
                        activity_drop_target_from_position(local_y, item_count, item_height)
                    {
                        update_tool_drop_target(
                            ToolDragSurface::ActivityBar,
                            target_index,
                            side,
                            cx,
                        );
                    }
                }),
            )
            .on_drop(cx.listener(move |this, drag: &ToolDrag, _, cx| {
                this.complete_drop(&drag.id, item_count, cx);
            }))
            .children(tool_items)
            .child(
                div()
                    .id("activity-tool-drop-end")
                    .w(px(sidebar_content_width))
                    .h(px(8.0))
                    .flex_none()
                    .on_mouse_move(cx.listener(move |_, _, _, cx| {
                        update_tool_drop_target(
                            ToolDragSurface::ActivityBar,
                            drop_end_target,
                            ToolDropSide::Bottom,
                            cx,
                        );
                    }))
                    .on_drop(cx.listener(move |this, drag: &ToolDrag, _, cx| {
                        this.complete_drop(&drag.id, drop_end_index, cx);
                    })),
            );
        if let Some((target_index, target_side)) = target
            && let Some(indicator) = activity_drop_indicator(
                accent,
                drag_state.source_index,
                target_index,
                target_side,
                item_count,
                sidebar_content_width,
                item_height,
            )
        {
            tool_list = tool_list.child(
                indicator
                    .id("activity-tool-drop-indicator")
                    .debug_selector(|| "activity-tool-drop-indicator".into())
                    .on_mouse_move(cx.listener(move |_, _, _, cx| {
                        update_tool_drop_target(
                            ToolDragSurface::ActivityBar,
                            target_index,
                            target_side,
                            cx,
                        );
                    }))
                    .on_drop(cx.listener(move |this, drag: &ToolDrag, _, cx| {
                        this.complete_drop(&drag.id, target_index, cx);
                    })),
            );
        }

        let mut container = v_flex()
            .id("activity-bar")
            .debug_selector(|| "activity-bar".into())
            .role(gpui_kit::Role::Navigation)
            .aria_label(navigation_label)
            .w(px(sidebar_width))
            .h_full()
            .min_h_0()
            .flex_none()
            .bg(sidebar_bg)
            .border_r_1()
            .border_color(border)
            .pt(px(if short_viewport { 8.0 } else { 18.0 }))
            .pb(px(if short_viewport { 8.0 } else { 12.0 }))
            .px(px(if inline_compact { 4.0 } else { 10.0 }))
            .track_focus(&self.focus_handle)
            .when(inline_compact, |container| container.items_center())
            .when(!inline_compact, |container| container.items_stretch())
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, window, cx| {
                if f32::from(window.viewport_size().width) <= crate::shell::SHELL_NARROW_BREAKPOINT
                    && event.keystroke.key == "escape"
                {
                    cx.emit(NavEvent::CloseNavigation);
                    cx.stop_propagation();
                }
            }))
            .on_mouse_up(MouseButton::Left, |_, _, cx| clear_tool_drag(cx));

        let brand_row = item::activity_brand_row(
            viewport_width <= crate::shell::SHELL_NARROW_BREAKPOINT,
            inline_compact,
            foreground,
            border,
        );
        let mut brand_block = v_flex()
            .id("activity-brand-block")
            .debug_selector(|| "activity-brand-block".into())
            .w_full()
            .flex_none()
            .gap(px(if short_viewport { 0.0 } else { 18.0 }))
            .child(brand_row);
        if !inline_compact && !short_viewport {
            brand_block = brand_block.child(
                div()
                    .id("activity-workspace-label")
                    .debug_selector(|| "activity-workspace-label".into())
                    .w_full()
                    .h(px(12.0))
                    .px(px(8.0))
                    .text_xs()
                    .text_color(muted_foreground)
                    .child("本地工作区"),
            );
        }
        container = container.child(brand_block);

        let is_home_selected = matches!(selected, NavTarget::Home);
        let home_item = activity_item(
            ActivityItemConfig {
                id: "home".into(),
                icon: icons::home(),
                is_selected: is_home_selected,
                height: item_height,
                item_width: sidebar_content_width,
                label_visible,
                accent,
                decoration: ActivityItemDecoration::new("工具总览", "工具总览", false),
                on_click: Box::new(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.navigate(NavTarget::Home, cx);
                })),
                tool_drag: None,
                source_index: None,
                source_background: None,
            },
            cx,
        );
        container = container.child(
            div()
                .w_full()
                .flex_none()
                .mt(px(if short_viewport { 4.0 } else { 18.0 }))
                .child(home_item),
        );

        if !inline_compact && !short_viewport {
            container = container.child(
                div()
                    .id("activity-tools-heading")
                    .debug_selector(|| "activity-tools-heading".into())
                    .w_full()
                    .px(px(10.0))
                    .pt(px(5.0))
                    .pb(px(7.0))
                    .text_size(px(11.0))
                    .text_color(muted_foreground)
                    .child("工具"),
            );
        }
        container = container.child(
            v_flex()
                .id("activity-tool-scroll")
                .debug_selector(|| "activity-tool-scroll".into())
                .w(px(sidebar_content_width))
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.tool_scroll)
                .vertical_scrollbar(&self.tool_scroll)
                .child(tool_list),
        );
        let manager_trigger = manager_button(
            sidebar_content_width,
            label_visible,
            short_viewport,
            item_height,
        );
        let mut command_section = item::command_section(short_viewport, inline_compact)
            .id("activity-command-section")
            .debug_selector(|| "activity-command-section".into())
            .w_full()
            .flex_none()
            .pt(px(if short_viewport { 8.0 } else { 14.0 }))
            .border_t_1()
            .border_color(border)
            .gap(px(ACTIVITY_ITEM_GAP));
        command_section = command_section.child(tool_surface_manager_popover(
            tool_manager,
            ToolDragSurface::ActivityBar,
            manager_trigger,
        ));
        command_section = command_section.child(activity_item(
            ActivityItemConfig {
                id: "shortcuts".into(),
                icon: crate::shortcuts_dialog::shortcut_icon(),
                is_selected: false,
                height: command_item_height(label_visible, short_viewport, item_height),
                item_width: item::command_item_width(sidebar_content_width, short_viewport),
                label_visible: item::command_label_visible(label_visible, short_viewport),
                accent,
                decoration: ActivityItemDecoration::new("快捷键", "快捷键", false),
                on_click: Box::new(|_: &ClickEvent, window, app| {
                    crate::shortcuts_dialog::open_shortcuts(window, app)
                }),
                tool_drag: None,
                source_index: None,
                source_background: None,
            },
            cx,
        ));
        let settings_selected = matches!(selected, NavTarget::Settings);
        command_section = command_section.child(activity_item(
            ActivityItemConfig {
                id: "settings".into(),
                icon: icons::settings(),
                is_selected: settings_selected,
                height: command_item_height(label_visible, short_viewport, item_height),
                item_width: item::command_item_width(sidebar_content_width, short_viewport),
                label_visible: item::command_label_visible(label_visible, short_viewport),
                accent,
                decoration: ActivityItemDecoration::new("设置", settings_tooltip, settings_badge),
                on_click: Box::new(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.navigate(NavTarget::Settings, cx);
                })),
                tool_drag: None,
                source_index: None,
                source_background: None,
            },
            cx,
        ));
        container = container.child(command_section);
        // 原型的侧栏页脚只出现在完整文字导航中，不进入图标栏和窄屏抽屉。
        if viewport_width > crate::shell::SHELL_NARROW_BREAKPOINT && !inline_compact {
            container = container.child(
                div()
                    .id("activity-sidebar-footer")
                    .debug_selector(|| "activity-sidebar-footer".into())
                    .w_full()
                    .flex_none()
                    .pt(px(12.0))
                    .px(px(10.0))
                    .text_size(px(11.0))
                    .text_color(muted_foreground)
                    .child("开发工作台"),
            );
        }

        container
    }
}

#[cfg(test)]
#[path = "activity_bar_visual_tests.rs"]
mod activity_bar_visual_tests;

#[cfg(test)]
#[path = "activity_bar_tests.rs"]
mod activity_bar_tests;
