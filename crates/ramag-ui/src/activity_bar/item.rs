//! 活动栏入口的显示配置、图标映射和按钮交互。

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _, badge::Badge, button::ButtonVariants as _, h_flex,
    v_flex,
};
use gpui_kit::{
    ClickEvent, Context, ParentElement, SharedString, Styled, Window, div, hsla, prelude::*, px,
};

use crate::tool_layout::{ToolDrag, ToolDragPreview, ToolDragSurface, begin_tool_drag};

use super::ActivityBar;

pub(super) struct ActivityItemDecoration {
    label: SharedString,
    tooltip: SharedString,
    show_badge: bool,
}

impl ActivityItemDecoration {
    pub(super) fn new(
        label: impl Into<SharedString>,
        tooltip: impl Into<SharedString>,
        show_badge: bool,
    ) -> Self {
        Self {
            label: label.into(),
            tooltip: tooltip.into(),
            show_badge,
        }
    }
}

pub(super) type ActivityClickHandler =
    Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui_kit::App) + 'static>;

pub(super) struct ActivityItemConfig {
    pub(super) id: SharedString,
    pub(super) icon: Icon,
    pub(super) is_selected: bool,
    pub(super) height: f32,
    pub(super) item_width: f32,
    pub(super) label_visible: bool,
    pub(super) accent: gpui_kit::Hsla,
    pub(super) decoration: ActivityItemDecoration,
    pub(super) on_click: ActivityClickHandler,
    pub(super) tool_drag: Option<ToolDrag>,
    pub(super) source_index: Option<usize>,
    pub(super) source_background: Option<gpui_kit::Hsla>,
}

/// 将工具元数据映射到内嵌 SVG，避免上游字体图标在不同平台回退成相同外观。
/// `declared_icon` 只允许使用本地白名单，未知插件图标统一回退到工具箱图标。
pub(super) fn tool_icon_path(tool_id: &str, declared_icon: Option<&str>) -> &'static str {
    match declared_icon.unwrap_or(tool_id) {
        "database" | "dbclient" => "icons/database.svg",
        "git_branch" | "vcs" => "icons/git-branch.svg",
        "clipboard" => "icons/clipboard.svg",
        "terminal" | "ssh" => "icons/terminal.svg",
        "gauge" | "system" => "icons/gauge.svg",
        "box" | "container" => "icons/container.svg",
        "server" | "kafka" => "icons/kafka.svg",
        "mqtt" => "icons/mqtt.svg",
        "api" => "icons/api.svg",
        "braces" | "jsonfmt" => "icons/json.svg",
        "cloud" | "object-storage" | "object_storage" => "icons/cloud.svg",
        "users" => "icons/users.svg",
        "plugin" | "plugins" => "icons/plugin.svg",
        "url" | "globe" => "icons/globe.svg",
        "hash" => "icons/hash.svg",
        _ => "icons/toolbox.svg",
    }
}

/// 为活动栏按钮创建共享的左对齐图标与标签内容；无标签入口保持图标居中。
pub(super) fn activity_button_content(
    id: &str,
    icon_content: gpui_kit::AnyElement,
    label: SharedString,
    label_visible: bool,
) -> gpui_kit::AnyElement {
    let icon_selector = format!("activity-entry-icon-{id}");
    let label_selector = format!("activity-entry-label-{id}");
    let label_selector_for_debug = label_selector.clone();
    let icon_wrapper = div()
        .id(icon_selector.clone())
        .debug_selector(move || icon_selector.clone())
        .size(px(16.0))
        .flex_none()
        .items_center()
        .justify_center()
        .child(icon_content);

    if label_visible {
        h_flex()
            .id(format!("activity-entry-content-{id}"))
            .w_full()
            .h_full()
            .min_w_0()
            .items_center()
            .justify_start()
            .gap(px(10.0))
            .child(icon_wrapper)
            .child(
                div()
                    .id(label_selector)
                    .debug_selector(move || label_selector_for_debug.clone())
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(13.0))
                    .child(label),
            )
            .into_any_element()
    } else {
        icon_wrapper.into_any_element()
    }
}

/// 创建打开工具管理弹窗的活动栏按钮，并复用导航入口的内容对齐方式。
pub(super) fn manager_button(
    item_width: f32,
    label_visible: bool,
    short_viewport: bool,
) -> gpui_kit::component::button::Button {
    let button_label_visible = label_visible && !short_viewport;
    let accessibility_label = if label_visible {
        "管理工具"
    } else {
        "管理首页和侧边栏工具"
    };
    let content = activity_button_content(
        "tool-manager",
        Icon::new(IconName::Plus).size(px(16.0)).into_any_element(),
        "管理工具".into(),
        button_label_visible,
    );
    crate::clickable_button("activity-tool-manager")
        .debug_selector(|| "activity-tool-manager".into())
        .ghost()
        .child(content)
        .w(px(if button_label_visible {
            item_width
        } else {
            32.0
        }))
        .h(px(command_item_height(label_visible, short_viewport)))
        .when(button_label_visible, |button| button.px(px(10.0)))
        .when(!button_label_visible, |button| button.size(px(32.0)).p_0())
        .small()
        .tooltip("管理首页和侧边栏工具的显示位置")
        .accessibility_label(accessibility_label)
}

/// 按侧栏模式调整固定命令入口高度，避免短窗口和紧凑图标栏挤压内容。
pub(super) fn command_item_height(label_visible: bool, short_viewport: bool) -> f32 {
    if label_visible && !short_viewport {
        super::ACTIVITY_ITEM_HEIGHT
    } else {
        32.0
    }
}

/// 短窗口固定入口使用图标尺寸，其余模式沿用侧栏内容宽度。
pub(super) fn command_item_width(item_width: f32, short_viewport: bool) -> f32 {
    if short_viewport { 32.0 } else { item_width }
}

/// 短窗口隐藏固定入口的可见文字，同时保留入口本身的辅助名称。
pub(super) fn command_label_visible(label_visible: bool, short_viewport: bool) -> bool {
    label_visible && !short_viewport
}

/// 短窗口将固定命令排成居中图标行，其他模式保持纵向导航布局。
pub(super) fn command_section(short_viewport: bool, inline_compact: bool) -> gpui_kit::Div {
    if short_viewport {
        h_flex().justify_center()
    } else if inline_compact {
        v_flex().items_center()
    } else {
        v_flex()
    }
}

/// 窄屏抽屉显示原型标题，桌面与图标栏继续使用产品品牌行。
pub(super) fn activity_brand_row(
    narrow_drawer: bool,
    inline_compact: bool,
    foreground: gpui_kit::Hsla,
    border: gpui_kit::Hsla,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let mut row = h_flex()
        .id("activity-brand")
        .debug_selector(|| "activity-brand".into())
        .w_full()
        .h(px(32.0))
        .items_center()
        .gap(px(10.0))
        .px(px(if inline_compact { 0.0 } else { 8.0 }))
        .when(inline_compact, |row| row.justify_center());
    if narrow_drawer {
        row = row.child(
            div()
                .id("activity-navigation-title")
                .debug_selector(|| "activity-navigation-title".into())
                .flex_1()
                .min_w_0()
                .text_lg()
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .text_color(foreground)
                .aria_label("全部工具")
                .child("全部工具"),
        );
    } else {
        row = row.child(
            div()
                .id("activity-brand-mark")
                .debug_selector(|| "activity-brand-mark".into())
                .flex()
                .items_center()
                .justify_center()
                .size(px(28.0))
                .flex_none()
                .border_1()
                .border_color(border)
                .rounded(px(6.0))
                .text_size(px(16.0))
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(foreground)
                .child("R"),
        );
        if !inline_compact {
            row = row.child(
                div()
                    .id("activity-brand-name")
                    .debug_selector(|| "activity-brand-name".into())
                    .flex_1()
                    .min_w_0()
                    .text_size(px(16.0))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(foreground)
                    .child("Ramag"),
            );
        }
    }
    row
}

pub(super) fn activity_item(
    config: ActivityItemConfig,
    cx: &mut Context<ActivityBar>,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let ActivityItemConfig {
        id,
        icon,
        is_selected,
        height,
        item_width,
        label_visible,
        accent,
        decoration,
        on_click,
        tool_drag,
        source_index,
        source_background,
    } = config;
    let ActivityItemDecoration {
        label,
        tooltip,
        show_badge,
    } = decoration;
    let transparent = hsla(0.0, 0.0, 0.0, 0.0);
    let mut selected_bg = accent;
    selected_bg.a = 0.14;
    let mut hover_bg = cx.theme().muted;
    hover_bg.a = 0.72;
    let active_bg = if is_selected {
        selected_bg
    } else {
        cx.theme().list_active
    };
    let foreground = if is_selected {
        accent
    } else {
        cx.theme().muted_foreground
    };
    let preview_icon = icon.clone();
    let preview_name = label.clone();
    let item_selector = format!("activity-{id}");
    let button_selector = if is_selected {
        format!("activity-selected-entry-button-{id}")
    } else {
        format!("activity-entry-button-{id}")
    };
    let icon = icon.size(px(16.0));
    let icon_content = if show_badge {
        div()
            .id("activity-settings-badge")
            .debug_selector(|| "activity-settings-badge".into())
            .child(Badge::new().dot().color(accent).child(icon))
            .into_any_element()
    } else {
        icon.into_any_element()
    };
    let button_content = activity_button_content(&id, icon_content, label.clone(), label_visible);
    let accessibility_label = if is_selected {
        format!("{label}，当前页面")
    } else {
        label.to_string()
    };
    let mut button = crate::clickable_button(id.clone())
        .debug_selector(move || button_selector.clone())
        .ghost()
        // 行容器统一绘制 hover/active 底色，避免 Ghost 单独留下白色 hover 字色。
        .bg(transparent)
        .text_color(foreground)
        .child(button_content);
    button = button
        .tooltip(tooltip)
        .accessibility_label(accessibility_label)
        .w(px(if label_visible { item_width } else { 32.0 }))
        .h(px(height))
        .when(label_visible, |button| button.px(px(10.0)))
        .when(!label_visible, |button| button.justify_center().p_0());
    let mut item = h_flex()
        .id(SharedString::from(item_selector.clone()))
        .debug_selector(move || item_selector.clone())
        .w(px(item_width))
        .h(px(height))
        .relative()
        .rounded(px(6.0))
        .bg(source_background.unwrap_or(transparent))
        .items_center()
        .gap(px(0.0))
        .when(!label_visible, |item| item.justify_center())
        .child(button.on_click(on_click));
    if is_selected {
        item = item.bg(selected_bg);
    }
    // 选中入口在悬停时仍保留强调底色，避免与普通入口混淆。
    item = item.hover(move |item| item.bg(if is_selected { selected_bg } else { hover_bg }));
    item = item.active(move |item| item.bg(active_bg));
    if let Some(drag) = tool_drag {
        let Some(source_index) = source_index else {
            return item;
        };
        item = item
            .cursor_move()
            .on_drag(drag, move |drag, position, _, cx| {
                begin_tool_drag(&drag.id, ToolDragSurface::ActivityBar, source_index, cx);
                cx.new(|_| {
                    ToolDragPreview::new(
                        ToolDragSurface::ActivityBar,
                        preview_icon.clone(),
                        preview_name.to_string(),
                        String::new(),
                    )
                    .position(position)
                })
            })
            .on_drop(cx.listener(move |this, drag: &ToolDrag, _, cx| {
                this.complete_drop(&drag.id, source_index, cx);
            }));
    }
    item
}

#[cfg(test)]
#[path = "item_tests.rs"]
mod tests;
