use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    animation::{EffectTransition, ease_in_out_cubic},
    button::{ButtonVariants as _, Toggle},
    h_flex,
    input::InputState,
    menu::PopupMenuItem,
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
use crate::PointerDropdownMenu as _;
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
/// 列表视图遵循原型的固定 64px 行高，不随全局卡片密度变化。
const HOME_LIST_ROW_HEIGHT: f32 = 64.0;
/// 原型标准和紧凑网格卡片的内边距。
const HOME_STANDARD_CARD_PADDING: f32 = 15.0;
const HOME_COMPACT_CARD_PADDING: f32 = 12.0;
/// 原型紧凑网格卡片的最小高度；大字号会按比例增加该值。
const HOME_COMPACT_CARD_HEIGHT: f32 = 93.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HomeToolCategory {
    All,
    Data,
    Message,
    Operations,
    Local,
    Other,
}

impl HomeToolCategory {
    const ALL: [Self; 6] = [
        Self::All,
        Self::Data,
        Self::Message,
        Self::Operations,
        Self::Local,
        Self::Other,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::All => "全部分类",
            Self::Data => "数据",
            Self::Message => "接口与消息",
            Self::Operations => "运维",
            Self::Local => "本机",
            Self::Other => "其他",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HomeLayout {
    Grid,
    List,
}

pub struct HomeView {
    registry: Arc<ToolRegistry>,
    scroll: ScrollHandle,
    tool_manager: Option<Entity<ToolSurfaceManager>>,
    search: Option<Entity<InputState>>,
    search_query: String,
    category: HomeToolCategory,
    layout: HomeLayout,
    last_rendered_slots: Vec<Option<String>>,
    _tool_layout_subscription: Subscription,
    _tool_drag_subscription: Subscription,
    /// 共享显示偏好变化后重绘首页并重新计算卡片几何。
    _system_settings_subscription: Subscription,
    _search_subscription: Option<Subscription>,
}

impl EventEmitter<HomeEvent> for HomeView {}

impl HomeView {
    pub fn new(registry: Arc<ToolRegistry>, cx: &mut Context<Self>) -> Self {
        cx.update_default_global::<ToolLayoutGlobal, _>(|_, _| {});
        cx.update_default_global::<ToolDragGlobal, _>(|_, _| {});
        let tool_layout_subscription = cx.observe_global::<ToolLayoutGlobal>(|_, cx| cx.notify());
        let tool_drag_subscription = cx.observe_global::<ToolDragGlobal>(|_, cx| cx.notify());
        let system_settings_subscription =
            cx.observe_global::<crate::SystemSettingsGlobal>(|_, cx| cx.notify());
        let last_rendered_slots = registry
            .home_list()
            .into_iter()
            .map(|tool| Some(tool.meta().id.clone()))
            .collect();
        Self {
            registry,
            scroll: ScrollHandle::new(),
            tool_manager: None,
            search: None,
            search_query: String::new(),
            category: HomeToolCategory::All,
            layout: HomeLayout::Grid,
            last_rendered_slots,
            _tool_layout_subscription: tool_layout_subscription,
            _tool_drag_subscription: tool_drag_subscription,
            _system_settings_subscription: system_settings_subscription,
            _search_subscription: None,
        }
    }

    /// 首次绘制时创建搜索输入，并在内容变化后同步首页筛选状态。
    fn ensure_search(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        if let Some(search) = self.search.as_ref() {
            return search.clone();
        }

        let search = cx.new(|cx| {
            crate::bounded_search_input(window, cx).placeholder("搜索工具名称、说明或 ID")
        });
        let search_for_event = search.clone();
        self._search_subscription = Some(cx.observe(&search, move |this, _, cx| {
            this.search_query = search_for_event.read(cx).value().trim().to_lowercase();
            this.last_rendered_slots.clear();
            cx.notify();
        }));
        self.search = Some(search.clone());
        search
    }

    /// 重置搜索与分类，使无结果状态可从同一处恢复全部首页工具。
    fn clear_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_query.clear();
        self.category = HomeToolCategory::All;
        self.last_rendered_slots.clear();
        if let Some(search) = self.search.as_ref() {
            search.update(cx, |state, cx| {
                state.set_value("", window, cx);
                cx.emit(gpui_kit::component::input::InputEvent::Change);
            });
        }
        cx.notify();
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

mod render;

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

/// 按当前窗口断点扣除实际侧栏宽度，避免首页网格超出主内容区。
fn home_content_width(window_width: f32) -> f32 {
    let sidebar_width = if window_width <= crate::shell::SHELL_NARROW_BREAKPOINT {
        0.0
    } else {
        crate::activity_bar::BAR_WIDTH
    };
    (window_width - sidebar_width - home_content_padding(window_width) * 2.0).max(0.0)
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

/// 紧凑卡片沿用原型 93px 基准；较大界面字号按比例增加高度，避免标题挤压正文。
fn home_card_height_for_density(
    card_width: f32,
    density: crate::ContentDensity,
    font_size: f32,
) -> f32 {
    let compact_height = HOME_COMPACT_CARD_HEIGHT * (font_size / 16.0).max(1.0);
    density.row_height(home_card_height(card_width), compact_height)
}

/// 密度档同时控制首页卡片内边距，与原型的标准和紧凑值保持一致。
fn home_card_padding_for_density(density: crate::ContentDensity) -> f32 {
    density.row_height(HOME_STANDARD_CARD_PADDING, HOME_COMPACT_CARD_PADDING)
}

/// 按当前卡片尺寸和间距计算重排动画起点，确保列表行与网格卡片对齐。
fn reorder_animation_offset(
    previous_slots: &[Option<String>],
    current_slots: &[Option<String>],
    id: &str,
    columns: usize,
    item_width: f32,
    item_height: f32,
    gap: f32,
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
        px((previous_column as f32 - current_column as f32) * (item_width + gap)),
        px((previous_row as f32 - current_row as f32) * (item_height + gap)),
    )
}

/// 列表按行中心决定插入前后，避免宽行中横向位置改变排序结果。
fn home_list_drop_target_from_position(
    y: f32,
    layout: HomeDropLayout,
) -> Option<(usize, ToolDropSide)> {
    if layout.item_count == 0 || y < 0.0 {
        return None;
    }

    let cell_height = layout.height + layout.gap;
    let target_index = (y / cell_height.max(1.0)).floor() as usize;
    if target_index >= layout.item_count {
        return Some((layout.item_count - 1, ToolDropSide::Bottom));
    }

    let local_y = y - target_index as f32 * cell_height;
    let side = if local_y < layout.height / 2.0 {
        ToolDropSide::Top
    } else {
        ToolDropSide::Bottom
    };
    Some((target_index, side))
}

/// 内置工具使用明确分类；插件和未来未知工具统一归到“其他”。
fn home_tool_category(id: &str) -> HomeToolCategory {
    match id {
        "dbclient" | "mongodb" | "redis" => HomeToolCategory::Data,
        "api" | "kafka" | "mqtt" => HomeToolCategory::Message,
        "ssh" | "object_storage" | "container" | "vcs" => HomeToolCategory::Operations,
        "clipboard" | "system" => HomeToolCategory::Local,
        _ => HomeToolCategory::Other,
    }
}

/// 名称、说明和 ID 共用一次小写匹配，类别筛选始终使用当前首页可见工具。
fn tool_matches_home_filter(
    meta: &ramag_domain::ToolMeta,
    query: &str,
    category: HomeToolCategory,
) -> bool {
    (category == HomeToolCategory::All || home_tool_category(&meta.id) == category)
        && (query.is_empty()
            || meta.name.to_lowercase().contains(query)
            || meta.description.to_lowercase().contains(query)
            || meta.id.to_lowercase().contains(query))
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
#[path = "home_view_accessibility_tests.rs"]
mod accessibility_tests;

#[cfg(test)]
mod tests {
    use super::{
        GOLDEN_RATIO, HomeDropLayout, HomeToolCategory, TOOL_CARD_GAP, TOOL_CARD_WIDTH,
        grid_columns, home_card_height, home_card_height_for_density,
        home_card_padding_for_density, home_content_width, home_list_drop_target_from_position,
        home_tool_category, reorder_animation_offset, tool_matches_home_filter,
    };
    use ramag_domain::ToolMeta;

    #[test]
    fn tool_cards_follow_the_golden_ratio() {
        assert!((TOOL_CARD_WIDTH / home_card_height(TOOL_CARD_WIDTH) - GOLDEN_RATIO).abs() < 0.001);
        assert!((240.0 / home_card_height(240.0) - GOLDEN_RATIO).abs() < 0.001);
    }

    #[test]
    fn compact_home_cards_follow_density_and_scale_for_large_text() {
        assert_eq!(
            home_card_height_for_density(TOOL_CARD_WIDTH, crate::ContentDensity::Standard, 18.0,),
            home_card_height(TOOL_CARD_WIDTH)
        );
        for font_size in [14.0, 16.0] {
            assert_eq!(
                home_card_height_for_density(
                    TOOL_CARD_WIDTH,
                    crate::ContentDensity::Compact,
                    font_size,
                ),
                93.0
            );
        }
        assert_eq!(
            home_card_height_for_density(TOOL_CARD_WIDTH, crate::ContentDensity::Compact, 18.0,),
            93.0 * 18.0 / 16.0
        );
        assert_eq!(
            home_card_padding_for_density(crate::ContentDensity::Standard),
            15.0
        );
        assert_eq!(
            home_card_padding_for_density(crate::ContentDensity::Compact),
            12.0
        );
    }

    #[test]
    fn grid_columns_expand_at_expected_window_widths() {
        for (window_width, expected_columns) in [
            (360.0, 1),
            (600.0, 2),
            (720.0, 2),
            (760.0, 3),
            (761.0, 2),
            (900.0, 2),
            (1024.0, 3),
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
    fn home_grid_uses_the_shell_width_at_narrow_and_desktop_breakpoints() {
        assert_eq!(home_content_width(360.0), 336.0);
        assert_eq!(home_content_width(720.0), 672.0);
        assert_eq!(home_content_width(760.0), 712.0);
        assert_eq!(home_content_width(761.0), 517.0);
        assert_eq!(home_content_width(1440.0), 1196.0);
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

        let right_offset = reorder_animation_offset(
            &previous,
            &current,
            "right",
            2,
            TOOL_CARD_WIDTH,
            home_card_height(TOOL_CARD_WIDTH),
            TOOL_CARD_GAP,
        );
        assert_eq!(f32::from(right_offset.0), TOOL_CARD_WIDTH + TOOL_CARD_GAP);
        assert_eq!(f32::from(right_offset.1), 0.0);

        let left_offset = reorder_animation_offset(
            &previous,
            &current,
            "left",
            2,
            TOOL_CARD_WIDTH,
            home_card_height(TOOL_CARD_WIDTH),
            TOOL_CARD_GAP,
        );
        assert_eq!(f32::from(left_offset.0), -(TOOL_CARD_WIDTH + TOOL_CARD_GAP));
        assert_eq!(f32::from(left_offset.1), 0.0);
    }

    #[test]
    fn list_reorder_animation_uses_the_rendered_row_height() {
        let previous = vec![Some("first".to_owned()), Some("second".to_owned())];
        let current = vec![Some("second".to_owned()), Some("first".to_owned())];
        let offset = reorder_animation_offset(
            &previous,
            &current,
            "first",
            1,
            640.0,
            super::HOME_LIST_ROW_HEIGHT,
            0.0,
        );
        assert_eq!(f32::from(offset.0), 0.0);
        assert_eq!(f32::from(offset.1), -super::HOME_LIST_ROW_HEIGHT);
    }

    #[test]
    fn list_drop_position_uses_row_edges_and_ignores_horizontal_geometry() {
        let layout = HomeDropLayout {
            width: 900.0,
            height: super::HOME_LIST_ROW_HEIGHT,
            item_count: 3,
            columns: 1,
            gap: 0.0,
        };
        assert_eq!(
            home_list_drop_target_from_position(8.0, layout),
            Some((0, super::ToolDropSide::Top))
        );
        assert_eq!(
            home_list_drop_target_from_position(56.0, layout),
            Some((0, super::ToolDropSide::Bottom))
        );
        assert_eq!(
            home_list_drop_target_from_position(140.0, layout),
            Some((2, super::ToolDropSide::Top))
        );
        assert_eq!(
            home_list_drop_target_from_position(250.0, layout),
            Some((2, super::ToolDropSide::Bottom))
        );
    }

    #[test]
    fn built_in_categories_are_explicit_and_unknown_tools_remain_filterable() {
        for (id, expected) in [
            ("dbclient", HomeToolCategory::Data),
            ("api", HomeToolCategory::Message),
            ("kafka", HomeToolCategory::Message),
            ("ssh", HomeToolCategory::Operations),
            ("object_storage", HomeToolCategory::Operations),
            ("clipboard", HomeToolCategory::Local),
            ("system", HomeToolCategory::Local),
            ("plugin.customer-tool", HomeToolCategory::Other),
        ] {
            assert_eq!(home_tool_category(id), expected, "tool {id}");
        }

        let plugin = ToolMeta::new(
            "plugin.customer-tool",
            "Customer Plugin",
            "A searchable external entry",
        );
        assert!(tool_matches_home_filter(
            &plugin,
            "customer",
            HomeToolCategory::Other
        ));
        assert!(tool_matches_home_filter(
            &plugin,
            "external",
            HomeToolCategory::All
        ));
        assert!(tool_matches_home_filter(
            &plugin,
            "plugin.customer",
            HomeToolCategory::Other
        ));
        assert!(!tool_matches_home_filter(
            &plugin,
            "external",
            HomeToolCategory::Data
        ));
    }
}
