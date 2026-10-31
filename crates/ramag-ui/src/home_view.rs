use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName,
    animation::{EffectTransition, ease_in_out_cubic},
    button::ButtonVariants as _,
    h_flex,
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::{
    AppContext as _, BorrowAppContext as _, ClickEvent, Context, DragMoveEvent, Entity,
    EventEmitter, IntoElement, MouseButton, ParentElement, Render, ScrollHandle, SharedString,
    Styled, Subscription, Window, div, prelude::*, px,
};

use ramag_app::ToolRegistry;

use crate::GOLDEN_RATIO;
use crate::REORDER_TRANSITION_DURATION;
use crate::activity_bar::ActivityBar;
use crate::tool_layout::{
    DRAGGED_ITEM_OPACITY, HomeDropLayout, ToolDrag, ToolDragGlobal, ToolDragPreview,
    ToolDragSurface, ToolDropSide, ToolLayoutGlobal, begin_tool_drag, clear_tool_drag,
    dragged_item_background, home_drop_indicator, home_drop_target_from_position,
    notify_tool_layout_changed, persist_tool_order, tool_drag_display_slots, tool_drag_handle,
    tool_drag_state, tool_drop_index, update_tool_drop_target,
};
use crate::tool_surface_manager::{
    ToolSurfaceManager, create_tool_surface_manager, tool_surface_manager_popover,
};

#[derive(Debug, Clone)]
pub enum HomeEvent {
    OpenTool(String),
}

const TOOL_CARD_WIDTH: f32 = 220.0;
const TOOL_CARD_GAP: f32 = 16.0;
const MAX_GRID_COLUMNS: f32 = 8.0;
const TOOL_GRID_WIDTH: f32 =
    TOOL_CARD_WIDTH * MAX_GRID_COLUMNS + TOOL_CARD_GAP * (MAX_GRID_COLUMNS - 1.0);

pub struct HomeView {
    registry: Arc<ToolRegistry>,
    scroll: ScrollHandle,
    tool_manager: Option<Entity<ToolSurfaceManager>>,
    last_rendered_slots: Vec<Option<String>>,
    _tool_layout_subscription: Subscription,
    _tool_drag_subscription: Subscription,
}

impl EventEmitter<HomeEvent> for HomeView {}

impl HomeView {
    pub fn new(registry: Arc<ToolRegistry>, cx: &mut Context<Self>) -> Self {
        cx.update_default_global::<ToolLayoutGlobal, _>(|_, _| {});
        cx.update_default_global::<ToolDragGlobal, _>(|_, _| {});
        let tool_layout_subscription = cx.observe_global::<ToolLayoutGlobal>(|_, cx| cx.notify());
        let tool_drag_subscription = cx.observe_global::<ToolDragGlobal>(|_, cx| cx.notify());
        let last_rendered_slots = registry
            .home_list()
            .into_iter()
            .map(|tool| Some(tool.meta().id.clone()))
            .collect();
        Self {
            registry,
            scroll: ScrollHandle::new(),
            tool_manager: None,
            last_rendered_slots,
            _tool_layout_subscription: tool_layout_subscription,
            _tool_drag_subscription: tool_drag_subscription,
        }
    }

    /// 在释放时一次性提交最终槽位，保存偏好并通知首页与侧栏同步重绘。
    fn complete_drop(&mut self, dragged_id: &str, fallback_index: usize, cx: &mut Context<Self>) {
        let target_index = tool_drop_index(ToolDragSurface::Home, fallback_index, cx);
        if self
            .registry
            .reorder_home_to_index(dragged_id, target_index)
        {
            persist_tool_order(&self.registry, cx);
            notify_tool_layout_changed(cx);
        }
        clear_tool_drag(cx);
    }
}

impl Render for HomeView {
    /// 将持久化工具顺序绘制为首页网格，并同步拖拽占位线与来源卡片反馈。
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tool_manager = match self.tool_manager.clone() {
            Some(manager) => manager,
            None => {
                let manager = create_tool_surface_manager(
                    self.registry.clone(),
                    ToolDragSurface::Home,
                    window,
                    cx,
                );
                self.tool_manager = Some(manager.clone());
                manager
            }
        };
        let theme = cx.theme();
        let muted_fg = theme.muted_foreground;
        let accent = theme.accent;
        let border = theme.border;
        let fg = theme.foreground;
        let palette = crate::pulse_ui::pulse_palette(cx);
        let bg = palette.background;
        let card_bg = palette.surface;
        let card_hover_bg = palette.raised;
        let window_width = f32::from(window.bounds().size.width);
        let compact = window_width < 640.0;
        let card_width = home_card_width(window_width);
        let card_height = home_card_height(card_width);

        let mut accent_border = accent;
        accent_border.a = 0.55;

        let tools = self.registry.home_list();
        let has_tools = !tools.is_empty();
        let current_order = tools
            .iter()
            .map(|tool| tool.meta().id.clone())
            .collect::<Vec<_>>();
        let drag_state = tool_drag_state(cx);
        let display_slots =
            tool_drag_display_slots(&current_order, ToolDragSurface::Home, &drag_state);
        let previous_slots =
            std::mem::replace(&mut self.last_rendered_slots, display_slots.clone());
        let layout_revision = cx.read_global::<ToolLayoutGlobal, _>(|state, _| state.revision);
        let columns = grid_columns(window_width);
        let grid_width = home_grid_width(columns, card_width);
        let item_count = current_order.len();
        let home_layout = HomeDropLayout {
            width: card_width,
            height: card_height,
            item_count,
            columns,
            gap: TOOL_CARD_GAP,
        };
        let target = drag_state
            .target
            .as_ref()
            .filter(|target| target.surface == ToolDragSurface::Home)
            .map(|target| (target.index, target.side));
        let mut cards = Vec::with_capacity(display_slots.len());
        for slot in &display_slots {
            let Some(id) = slot else {
                continue;
            };

            let Some(tool) = tools.iter().find(|tool| tool.meta().id == *id) else {
                continue;
            };
            let Some(source_index) = current_order.iter().position(|item| item == id) else {
                continue;
            };
            let card_id = SharedString::from(format!("home-tool-{id}"));
            let debug_card_id = card_id.clone();
            let id_for_click = id.clone();
            let name = tool.meta().name.clone();
            let description = tool.meta().description.clone();
            let title_selector = format!("home-tool-title-{id}");
            let description_selector = format!("home-tool-description-{id}");
            let tooltip = format!("{name} — {description}");
            let icon = ActivityBar::icon_for_meta(tool.meta());
            let preview_icon = icon.clone();
            let preview_name = name.clone();
            let preview_description = description.clone();
            let drag = ToolDrag { id: id.clone() };
            let is_dragged = drag_state.dragged_id.as_deref() == Some(id.as_str());
            let card_background = if is_dragged {
                dragged_item_background(card_bg)
            } else {
                card_bg
            };
            let card_border = if is_dragged {
                accent.opacity(0.78)
            } else {
                border.opacity(0.65)
            };

            let mut card = v_flex()
                .id(card_id)
                .debug_selector(move || debug_card_id.to_string())
                .w(px(card_width))
                .h(px(card_height))
                .p(px(12.0))
                .gap(px(8.0))
                .bg(card_background)
                .border_1()
                .border_color(card_border)
                .rounded(px(6.0))
                .shadow_sm()
                .relative()
                .cursor_pointer()
                .tooltip(move |window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(tooltip.clone()).build(window, cx)
                })
                .hover(move |this| {
                    this.border_color(accent_border)
                        .bg(card_hover_bg)
                        .shadow_md()
                })
                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                    cx.emit(HomeEvent::OpenTool(id_for_click.clone()));
                }))
                .on_drag(drag, move |drag, position, _, cx| {
                    begin_tool_drag(&drag.id, ToolDragSurface::Home, source_index, cx);
                    cx.new(|_| {
                        ToolDragPreview::new(
                            ToolDragSurface::Home,
                            preview_icon.clone(),
                            preview_name.clone(),
                            preview_description.clone(),
                        )
                        .position(position)
                    })
                })
                .on_drop(cx.listener(move |this, drag: &ToolDrag, _, cx| {
                    this.complete_drop(&drag.id, source_index, cx);
                }))
                .child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .pr(px(14.0))
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            h_flex()
                                .size(px(24.0))
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .text_color(accent)
                                .child(icon),
                        )
                        .child(
                            div()
                                .debug_selector(move || title_selector.clone())
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(fg)
                                .child(name),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(8.0))
                        .right(px(8.0))
                        .child(tool_drag_handle(accent.opacity(0.65))),
                )
                .child(
                    div()
                        .debug_selector(move || description_selector.clone())
                        .w_full()
                        .min_w_0()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .whitespace_normal()
                        .text_xs()
                        .line_height(gpui_kit::rems(1.35))
                        .text_color(muted_fg)
                        .child(description),
                );
            if is_dragged {
                card = card.opacity(DRAGGED_ITEM_OPACITY);
            }
            let (from_x, from_y) = reorder_animation_offset_for_width(
                &previous_slots,
                &display_slots,
                id,
                columns,
                card_width,
            );
            let card = if from_x == px(0.0) && from_y == px(0.0) {
                card.into_any_element()
            } else {
                EffectTransition::new(REORDER_TRANSITION_DURATION)
                    .ease(ease_in_out_cubic)
                    .slide_x(from_x, px(0.0))
                    .slide_y(from_y, px(0.0))
                    .apply(card, format!("home-tool-reorder-{id}-{layout_revision}"))
                    .into_any_element()
            };
            cards.push(card);
        }

        let mut tool_grid = div()
            .id("home-tool-grid")
            .debug_selector(|| "home-tool-grid".into())
            .w(px(grid_width))
            .max_w(px(TOOL_GRID_WIDTH))
            .flex()
            .flex_row()
            .flex_wrap()
            .justify_start()
            .gap(px(TOOL_CARD_GAP))
            .relative()
            .on_drag_move(
                cx.listener(move |_, event: &DragMoveEvent<ToolDrag>, _, cx| {
                    let local_x = f32::from(event.event.position.x - event.bounds.left());
                    let local_y = f32::from(event.event.position.y - event.bounds.top());
                    if let Some((target_index, side)) = home_drop_target_from_position(
                        local_x,
                        local_y,
                        drag_state.source_index,
                        home_layout,
                    ) {
                        update_tool_drop_target(ToolDragSurface::Home, target_index, side, cx);
                    }
                }),
            )
            .on_drop(cx.listener(move |this, drag: &ToolDrag, _, cx| {
                this.complete_drop(&drag.id, item_count, cx);
            }))
            .children(cards);
        if let Some((target_index, target_side)) = target
            && let Some(indicator) = home_drop_indicator(
                accent,
                drag_state.source_index,
                target_index,
                target_side,
                home_layout,
            )
        {
            tool_grid = tool_grid.child(
                indicator
                    .id("home-tool-drop-indicator")
                    .on_mouse_move(cx.listener(move |_, _, _, cx| {
                        update_tool_drop_target(
                            ToolDragSurface::Home,
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
        if has_tools {
            let drop_end_index = item_count;
            let drop_end_target = drop_end_index - 1;
            tool_grid = tool_grid.child(
                div()
                    .id("home-tool-drop-end")
                    .w_full()
                    .h(px(10.0))
                    .flex_none()
                    .on_mouse_move(cx.listener(move |_, _, _, cx| {
                        update_tool_drop_target(
                            ToolDragSurface::Home,
                            drop_end_target,
                            ToolDropSide::Bottom,
                            cx,
                        );
                    }))
                    .on_drop(cx.listener(move |this, drag: &ToolDrag, _, cx| {
                        this.complete_drop(&drag.id, drop_end_index, cx);
                    })),
            );
        }

        v_flex()
            .id("home-view")
            .debug_selector(|| "home-view".into())
            .size_full()
            .bg(bg)
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .vertical_scrollbar(&self.scroll)
            .items_center()
            .on_mouse_up(MouseButton::Left, |_, _, cx| clear_tool_drag(cx))
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(TOOL_GRID_WIDTH + home_content_padding(window_width) * 2.0))
                    .p(px(home_content_padding(window_width)))
                    .gap(px(if compact { 16.0 } else { 24.0 }))
                    .items_center()
                    .child(render_home_header(item_count, tool_manager, grid_width, cx))
                    .child(tool_grid)
                    .when(!has_tools, |content| {
                        content.child(crate::pulse_ui::pulse_status_notice(
                            crate::pulse_ui::PulseStatus::Unavailable,
                            "首页没有显示工具。使用右上角的“管理工具”恢复入口，也可在设置的“工具显示”页管理。",
                            cx,
                        ))
                    }),
            )
    }
}

/// 根据窗口宽度计算首页网格列数，动画偏移必须与实际换行规则一致。
fn grid_columns(window_width: f32) -> usize {
    let content_width = home_content_width(window_width);
    ((content_width.max(TOOL_CARD_WIDTH) + TOOL_CARD_GAP) / (TOOL_CARD_WIDTH + TOOL_CARD_GAP))
        .floor()
        .clamp(1.0, MAX_GRID_COLUMNS) as usize
}

/// Keep the heading aligned with the actual card columns instead of an empty wide grid area.
fn home_grid_width(columns: usize, card_width: f32) -> f32 {
    card_width * columns as f32 + TOOL_CARD_GAP * columns.saturating_sub(1) as f32
}

/// Return the main-pane width available to the home grid after the activity bar and padding.
fn home_content_width(window_width: f32) -> f32 {
    (window_width - 48.0 - home_content_padding(window_width) * 2.0).max(0.0)
}

/// Use smaller edge padding on compact windows so one card still has usable width.
fn home_content_padding(window_width: f32) -> f32 {
    if window_width < 640.0 { 12.0 } else { 24.0 }
}

/// Shrink the single-column card only when the main pane cannot fit its desktop width.
fn home_card_width(window_width: f32) -> f32 {
    if grid_columns(window_width) == 1 {
        home_content_width(window_width).min(TOOL_CARD_WIDTH)
    } else {
        TOOL_CARD_WIDTH
    }
}

/// Keep the card's height proportional when a compact window shrinks its width.
fn home_card_height(card_width: f32) -> f32 {
    card_width / GOLDEN_RATIO
}

/// Calculate the animation start using the card dimensions currently rendered by the grid.
fn reorder_animation_offset_for_width(
    previous_slots: &[Option<String>],
    current_slots: &[Option<String>],
    id: &str,
    columns: usize,
    card_width: f32,
) -> (gpui_kit::Pixels, gpui_kit::Pixels) {
    let Some(previous_index) = previous_slots
        .iter()
        .position(|slot| slot.as_deref() == Some(id))
    else {
        return (px(0.0), px(0.0));
    };
    let Some(current_index) = current_slots
        .iter()
        .position(|slot| slot.as_deref() == Some(id))
    else {
        return (px(0.0), px(0.0));
    };
    if previous_index == current_index {
        return (px(0.0), px(0.0));
    }

    let previous_column = previous_index % columns;
    let previous_row = previous_index / columns;
    let current_column = current_index % columns;
    let current_row = current_index / columns;
    (
        px((previous_column as f32 - current_column as f32) * (card_width + TOOL_CARD_GAP)),
        px((previous_row as f32 - current_row as f32)
            * (home_card_height(card_width) + TOOL_CARD_GAP)),
    )
}

/// 标题与工具网格共用宽度；文字层级在窄窗口和较大字号下仍可自然换行。
fn render_home_header(
    tool_count: usize,
    tool_manager: Entity<ToolSurfaceManager>,
    grid_width: f32,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    h_flex()
        .id("home-logo")
        .debug_selector(|| "home-logo".into())
        .w(px(grid_width))
        .min_w_0()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .child(
            crate::pulse_ui::pulse_page_title("工具", Some(format!("{tool_count} 个可用工具")), cx)
                .flex_1()
                .min_w_0(),
        )
        .child(tool_surface_manager_popover(
            tool_manager,
            ToolDragSurface::Home,
            crate::clickable_button("home-tool-manager")
                .debug_selector(|| "home-tool-manager".into())
                .ghost()
                .icon(Icon::new(IconName::Plus))
                .label("管理工具")
                .tooltip("管理首页和侧边栏工具的显示位置"),
        ))
}

#[cfg(test)]
#[path = "home_view_visual_tests.rs"]
mod visual_tests;

#[cfg(test)]
mod tests {
    use super::{
        GOLDEN_RATIO, TOOL_CARD_GAP, TOOL_CARD_WIDTH, grid_columns, home_card_height,
        reorder_animation_offset_for_width,
    };

    #[test]
    fn tool_cards_follow_the_golden_ratio() {
        assert!((TOOL_CARD_WIDTH / home_card_height(TOOL_CARD_WIDTH) - GOLDEN_RATIO).abs() < 0.001);
        assert!((240.0 / home_card_height(240.0) - GOLDEN_RATIO).abs() < 0.001);
    }

    #[test]
    fn grid_columns_expand_at_expected_window_widths() {
        for (window_width, expected_columns) in [
            (360.0, 1),
            (600.0, 2),
            (900.0, 3),
            (1024.0, 4),
            (1200.0, 4),
            (1440.0, 5),
            (1689.0, 6),
            (2560.0, 8),
        ] {
            assert_eq!(
                grid_columns(window_width),
                expected_columns,
                "window width {window_width}"
            );
        }
    }

    #[test]
    fn reorder_animation_starts_at_the_previous_grid_slot() {
        let previous = vec![
            Some("left".to_owned()),
            Some("right".to_owned()),
            Some("bottom".to_owned()),
        ];
        let current = vec![
            Some("right".to_owned()),
            Some("left".to_owned()),
            Some("bottom".to_owned()),
        ];

        let right_offset =
            reorder_animation_offset_for_width(&previous, &current, "right", 2, TOOL_CARD_WIDTH);
        assert_eq!(f32::from(right_offset.0), TOOL_CARD_WIDTH + TOOL_CARD_GAP);
        assert_eq!(f32::from(right_offset.1), 0.0);

        let left_offset =
            reorder_animation_offset_for_width(&previous, &current, "left", 2, TOOL_CARD_WIDTH);
        assert_eq!(f32::from(left_offset.0), -(TOOL_CARD_WIDTH + TOOL_CARD_GAP));
        assert_eq!(f32::from(left_offset.1), 0.0);
    }
}
