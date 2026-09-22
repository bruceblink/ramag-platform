//! 共享 UI：Shell（左 ActivityBar + 右 Tool 视图）+ 主题 + 通用组件

pub mod actions;
pub mod activity_bar;
pub mod assets;
pub mod axis_scroll;
pub mod confirm_dialog;
pub mod copy_support;
pub mod data_sync_overlay;
pub mod database_result_settings;
pub mod database_search;
mod dialog_layout;
pub mod editor_workspace;
pub mod home_view;
pub mod icons;
pub mod markdown;
pub mod mutation_gate;
pub mod platform;
mod plugin_diagnostics;
pub mod pointer_menu;
pub mod preferences;
pub mod prompt_dialog;
pub mod recent_items_dialog;
pub mod redis_tree_settings;
pub mod result_memory;
pub mod result_paging;
pub mod settings_view;
pub mod shell;
pub mod shortcuts_dialog;
pub mod system_settings;
pub mod theme;
pub(crate) mod tool_layout;
pub mod transfer_ui;

pub use actions::{CloseTab, OpenRecentItems};
pub use assets::RamagAssets;
pub use confirm_dialog::{open_confirm, open_confirm_with_cancel};
pub use copy_support::{
    SelectableText, copy_success_notification, copy_text, copy_text_with_notification,
    is_primary_modifier_double_click,
};
pub use data_sync_overlay::DataSyncOverlay;
pub use database_result_settings::{
    DATABASE_RESULT_SETTINGS_PREF_KEY, DatabaseResultSettings, DatabaseResultSettingsGlobal,
    database_result_settings, init_database_result_settings, set_database_result_settings,
};
pub use database_search::{
    DATABASE_SEARCH_SETTINGS_PREF_KEY, DatabaseSearchSettings, DatabaseSearchSettingsGlobal,
    database_search_settings, init_database_search_settings, set_database_search_settings,
    validate_id_converter_program,
};
pub use prompt_dialog::{
    open_bounded_masked_prompt, open_bounded_prompt, open_masked_prompt,
    open_optional_bounded_prompt, open_optional_prompt, open_prompt, open_reveal_masked_prompt,
};
pub use redis_tree_settings::{
    REDIS_TREE_SETTINGS_PREF_KEY, RedisTreeSettings, init_redis_tree_settings, redis_tree_settings,
    set_redis_tree_settings,
};

pub use activity_bar::{ActivityBar, NavEvent, NavTarget, sync_update_indicator};
pub use axis_scroll::{AxisScrollGesture, RestrictUniformListToAxisExt, handle_axis_scroll};
pub use editor_workspace::{
    EditorDraftPref, EditorWorkspacePref, MAX_EDITOR_DRAFT_BYTES, MAX_EDITOR_TABS,
    MAX_EDITOR_WORKSPACE_PREF_BYTES, MAX_EDITOR_WORKSPACE_TEXT_BYTES, can_open_editor_tab,
};
pub use home_view::{HomeEvent, HomeView};
pub use markdown::{markdown_preview, markdown_preview_at_path};
pub use mutation_gate::{AsyncMutationGate, MutationToken};
pub use pointer_menu::PointerDropdownMenu;
pub use result_memory::{
    GLOBAL_RESULT_WARNING_BYTES, MAX_GLOBAL_RESULT_BYTES, ResultMemoryBudget, ResultMemoryLease,
    ResultMemoryUpdate,
};
pub use result_paging::{
    DEFAULT_RESULT_PAGE_SIZE, MAX_RESULT_PAGE_SIZE, RESULT_PAGE_SIZE_PRESETS,
    parse_result_page_size, validate_result_page_size,
};
pub use settings_view::SettingsView;
pub use shell::{Shell, WindowBoundsPref};
pub use shortcuts_dialog::open_shortcuts;
pub use system_settings::{
    SYSTEM_SETTINGS_PREF_KEY, SystemSettings, SystemSettingsGlobal, init_system_settings,
    set_system_settings, system_settings,
};
pub use theme::{Mode, StorageGlobal, apply_theme, current_mode, init_theme};
pub use transfer_ui::{
    TransferState, open_import_options_dialog, progress_sink, spawn_transfer_ticker,
    transfer_notification, transfer_progress_row,
};

pub const FEEDBACK_ISSUE_URL: &str = "https://github.com/bruceblink/ramag-platform/issues/new";
pub const COMMUNITY_URL: &str =
    "https://github.com/bruceblink/ramag-platform/blob/main/README.md#社区--community";

/// 数据库、SSH 与云存储共用的生产保护文案，避免同一语义在各工具中漂移。
pub const PRODUCTION_MODE_LABEL: &str = "生产模式（只读保护）";
pub const PRODUCTION_BADGE_LABEL: &str = "生产";

/// 根据当前窗口宽度收缩对话框，保留 16px 两侧边距和宽窗口可读上限。
pub fn responsive_dialog_width(window: &gpui_kit::Window, preferred: f32) -> gpui_kit::Pixels {
    let available = f32::from(window.viewport_size().width);
    gpui_kit::px((available - 32.0).max(0.0).min(preferred))
}

/// 返回共享对话框应使用的顶部偏移，避免短窗口把内容推到视口之外。
pub fn responsive_dialog_top(window: &gpui_kit::Window) -> gpui_kit::Pixels {
    (window.viewport_size().height * 0.05).min(gpui_kit::px(42.0))
}

/// 返回对话框外框的最大高度；正文应在此高度内启用自己的滚动。
pub fn responsive_dialog_max_height(window: &gpui_kit::Window) -> gpui_kit::Pixels {
    (window.viewport_size().height - responsive_dialog_top(window) - gpui_kit::px(16.0))
        .max(gpui_kit::px(0.0))
}

/// 将通知宽度限制在当前窗口可用空间内，并保留宽窗口的可读上限。
pub fn responsive_notification(
    notification: gpui_kit::component::notification::Notification,
) -> gpui_kit::component::notification::Notification {
    use gpui_kit::Styled as _;

    notification.w_full().min_w_0().max_w(gpui_kit::px(320.0))
}

/// 推送统一宽度的通知，确保各工具的长状态和错误文案服从相同的窗口边界。
pub fn push_responsive_notification(
    window: &mut gpui_kit::Window,
    notification: gpui_kit::component::notification::Notification,
    cx: &mut gpui_kit::App,
) {
    use gpui_kit::Styled as _;
    use gpui_kit::component::WindowExt as _;

    let available_width = (f32::from(window.viewport_size().width) - 32.0).clamp(0.0, 320.0);
    let short_window = window.viewport_size().height < gpui_kit::px(160.0);
    let theme = gpui_kit::component::Theme::global_mut(cx);
    theme.notification.width = gpui_kit::px(available_width);
    theme.notification.margins.top = if short_window {
        gpui_kit::px(64.0)
    } else {
        gpui_kit::px(50.0)
    };
    let notification = responsive_notification(notification)
        .w(gpui_kit::px(available_width))
        .max_w(gpui_kit::px(available_width));
    window.push_notification(notification, cx);
}

/// 创建可在窄窗口换行的通用工具栏；调用方负责补充具体的对齐和内边距。
pub fn responsive_toolbar() -> gpui_kit::Div {
    use gpui_kit::component::h_flex;
    use gpui_kit::{Styled as _, px};

    h_flex()
        .w_full()
        .min_w_0()
        .flex_wrap()
        .items_center()
        .gap(px(8.0))
}

/// 创建带手型光标的按钮，统一可点击控件的悬浮反馈。
pub fn clickable_button(id: impl Into<gpui_kit::ElementId>) -> gpui_kit::component::button::Button {
    use gpui_kit::Styled as _;

    gpui_kit::component::button::Button::new(id).cursor_pointer()
}

/// 创建带手型光标的复选框。
pub fn clickable_checkbox(
    id: impl Into<gpui_kit::ElementId>,
) -> gpui_kit::component::checkbox::Checkbox {
    use gpui_kit::Styled as _;

    gpui_kit::component::checkbox::Checkbox::new(id).cursor_pointer()
}

/// 创建带手型光标的开关。
pub fn clickable_switch(id: impl Into<gpui_kit::ElementId>) -> gpui_kit::component::switch::Switch {
    use gpui_kit::Styled as _;

    gpui_kit::component::switch::Switch::new(id).cursor_pointer()
}

/// 创建带手型清除按钮的单行输入框。
pub fn cleanable_input(
    state: &gpui_kit::Entity<gpui_kit::component::input::InputState>,
    clear_id: impl Into<gpui_kit::SharedString>,
    disabled: bool,
    cx: &gpui_kit::App,
) -> gpui_kit::component::input::Input {
    use gpui_kit::component::{
        ActiveTheme as _, Icon, IconName, Sizable as _, button::ButtonVariants as _, input::Input,
    };
    use gpui_kit::{InteractiveElement as _, Styled as _};

    let input = Input::new(state).disabled(disabled).min_w_0();
    if disabled || state.read(cx).value().is_empty() {
        return input;
    }

    let state = state.clone();
    let clear_id = clear_id.into();
    let clear_selector = clear_id.to_string();
    input.suffix(
        clickable_button(clear_id)
            .debug_selector(move || clear_selector.clone())
            .icon(Icon::new(IconName::CircleX))
            .ghost()
            .xsmall()
            .flex_none()
            .tooltip("清空")
            .tab_stop(false)
            .text_color(cx.theme().muted_foreground)
            .on_click(move |_, window, cx| {
                state.update(cx, |state, cx| {
                    state.set_value("", window, cx);
                    // InputState::set_value 会主动抑制 Change；显式补发以同步调用方筛选状态。
                    cx.emit(gpui_kit::component::input::InputEvent::Change);
                    state.focus(window, cx);
                });
            }),
    )
}

/// 创建带清除按钮的代码编辑器；用于需要补全菜单的单行筛选框。
pub fn cleanable_editor(
    state: &gpui_kit::Entity<gpui_kit::component::input::EditorState>,
    clear_id: impl Into<gpui_kit::SharedString>,
    disabled: bool,
    cx: &gpui_kit::App,
) -> gpui_kit::Div {
    use gpui_kit::component::input::Editor;
    use gpui_kit::component::{
        ActiveTheme as _, Icon, IconName, Sizable as _, button::ButtonVariants as _, h_flex,
    };
    use gpui_kit::prelude::FluentBuilder as _;
    use gpui_kit::{InteractiveElement as _, ParentElement as _, Styled as _};

    let clear_id = clear_id.into();
    let clear_selector = clear_id.to_string();
    let state_for_clear = state.clone();
    h_flex()
        .w_full()
        .min_w_0()
        .child(
            Editor::new(state)
                .flex_1()
                .min_w_0()
                .disabled(disabled)
                .bordered(false),
        )
        .when(!disabled && !state.read(cx).value().is_empty(), |this| {
            this.child(
                clickable_button(clear_id)
                    .debug_selector(move || clear_selector.clone())
                    .icon(Icon::new(IconName::CircleX))
                    .ghost()
                    .xsmall()
                    .flex_none()
                    .tooltip("清空")
                    .tab_stop(false)
                    .text_color(cx.theme().muted_foreground)
                    .on_click(move |_, window, cx| {
                        state_for_clear.update(cx, |state, cx| {
                            state.set_value("", window, cx);
                            cx.emit(gpui_kit::component::input::InputEvent::Change);
                            state.focus(window, cx);
                        });
                    }),
            )
        })
}

/// 创建带手型关闭按钮的对话框标题；调用方仍可用 `on_close` 处理 Esc 等关闭路径。
pub fn closable_dialog_title(
    id: impl Into<gpui_kit::ElementId>,
    title: impl gpui_kit::IntoElement,
    on_close: impl Fn(&mut gpui_kit::Window, &mut gpui_kit::App) + 'static,
) -> impl gpui_kit::IntoElement {
    use gpui_kit::component::{
        IconName, Sizable as _, WindowExt as _, button::ButtonVariants as _, h_flex,
    };
    use gpui_kit::{InteractiveElement as _, ParentElement as _, Styled as _, div, px};

    h_flex()
        .debug_selector(|| "ramag-dialog-title".into())
        .w_full()
        .min_w_0()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .child(title),
        )
        .child(
            clickable_button(id)
                .ghost()
                .xsmall()
                .flex_none()
                .icon(IconName::Close)
                .debug_selector(|| "ramag-dialog-close".into())
                .tooltip("关闭")
                .on_click(move |_, window, cx| {
                    window.close_dialog(cx);
                    on_close(window, cx);
                }),
        )
}

/// 创建可在窄窗口换行的对话框双按钮操作区；第一个按钮是次要操作，第二个按钮是主要操作。
pub fn dialog_action_footer(
    secondary: impl gpui_kit::IntoElement,
    primary: impl gpui_kit::IntoElement,
) -> impl gpui_kit::IntoElement {
    use gpui_kit::component::h_flex;
    use gpui_kit::{InteractiveElement as _, ParentElement as _, Styled as _, px};

    h_flex()
        .debug_selector(|| "ramag-dialog-footer".into())
        .w_full()
        .flex_wrap()
        .items_center()
        .justify_end()
        .gap(px(8.0))
        .child(secondary)
        .child(primary)
}

/// 创建可在窄窗口内换行的居中状态提示，用于加载中、空列表和过滤无结果状态。
pub fn centered_status(
    message: impl Into<gpui_kit::SharedString>,
    color: gpui_kit::Hsla,
) -> gpui_kit::AnyElement {
    use gpui_kit::component::v_flex;
    use gpui_kit::{
        InteractiveElement as _, IntoElement as _, ParentElement as _, Styled as _, div, px,
    };

    v_flex()
        .size_full()
        .min_w_0()
        .items_center()
        .justify_center()
        .px(px(12.0))
        .child(
            div()
                .debug_selector(|| "ramag-centered-status-message".into())
                .w_full()
                .min_w_0()
                .text_center()
                .text_sm()
                .text_color(color)
                .child(message.into()),
        )
        .into_any_element()
}

/// 创建带手型光标的菜单项。
pub fn menu_item(
    label: impl Into<gpui_kit::SharedString>,
) -> gpui_kit::component::menu::PopupMenuItem {
    menu_item_with_disabled(label, false)
}

/// 创建可禁用菜单项；禁用时保持箭头。
pub fn menu_item_with_disabled(
    label: impl Into<gpui_kit::SharedString>,
    disabled: bool,
) -> gpui_kit::component::menu::PopupMenuItem {
    use gpui_kit::{ParentElement as _, Styled as _, div, prelude::FluentBuilder as _};

    let label = label.into();
    gpui_kit::component::menu::PopupMenuItem::element(move |_, _| {
        div()
            .w_full()
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .when(disabled, |this| this.cursor_default())
                    .when(!disabled, |this| this.cursor_pointer()),
            )
            .child(label.clone())
    })
    .disabled(disabled)
}

/// 即时搜索 / 过滤会在每次输入后重算，限制异常粘贴造成的重复分配与全表扫描成本。
pub const MAX_SEARCH_INPUT_BYTES: usize = 4 * 1024;

pub fn bounded_search_input(
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::Context<gpui_kit::component::input::InputState>,
) -> gpui_kit::component::input::InputState {
    gpui_kit::component::input::InputState::new(window, cx)
        .validate(|value, _| value.len() <= MAX_SEARCH_INPUT_BYTES)
}

/// GPUI Component 的 `validate` 目前仅检查单行输入；多行 / 代码编辑器需在变更后立即收口。
/// 超限时保留 UTF-8 安全前缀，并调用 `on_exceeded` 让业务层展示具体提示。
pub fn enforce_multiline_input_byte_limit<T: 'static>(
    input: &gpui_kit::Entity<gpui_kit::component::input::InputState>,
    max_bytes: usize,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::Context<T>,
    on_exceeded: impl Fn(&mut T, &mut gpui_kit::Window, &mut gpui_kit::Context<T>) + 'static,
) -> gpui_kit::Subscription {
    let input_for_event = input.clone();
    cx.subscribe_in(
        input,
        window,
        move |this, _, event: &gpui_kit::component::input::InputEvent, window, cx| {
            if !matches!(event, gpui_kit::component::input::InputEvent::Change) {
                return;
            }
            if !clamp_multiline_input_value(&input_for_event, max_bytes, window, cx) {
                return;
            }
            on_exceeded(this, window, cx);
        },
    )
}

/// 限制代码编辑器的输入大小；EditorState 与普通 InputState 是不同的状态类型。
pub fn enforce_editor_input_byte_limit<T: 'static>(
    input: &gpui_kit::Entity<gpui_kit::component::input::EditorState>,
    max_bytes: usize,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::Context<T>,
    on_exceeded: impl Fn(&mut T, &mut gpui_kit::Window, &mut gpui_kit::Context<T>) + 'static,
) -> gpui_kit::Subscription {
    let input_for_event = input.clone();
    cx.subscribe_in(
        input,
        window,
        move |this, _, event: &gpui_kit::component::input::InputEvent, window, cx| {
            if !matches!(event, gpui_kit::component::input::InputEvent::Change) {
                return;
            }
            if !clamp_editor_input_value(&input_for_event, max_bytes, window, cx) {
                return;
            }
            on_exceeded(this, window, cx);
        },
    )
}

/// 限制普通多行文本框的输入大小；TextareaState 与普通 InputState 是不同的状态类型。
pub fn enforce_textarea_input_byte_limit<T: 'static>(
    input: &gpui_kit::Entity<gpui_kit::component::input::TextareaState>,
    max_bytes: usize,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::Context<T>,
    on_exceeded: impl Fn(&mut T, &mut gpui_kit::Window, &mut gpui_kit::Context<T>) + 'static,
) -> gpui_kit::Subscription {
    let input_for_event = input.clone();
    cx.subscribe_in(
        input,
        window,
        move |this, _, event: &gpui_kit::component::input::InputEvent, window, cx| {
            if !matches!(event, gpui_kit::component::input::InputEvent::Change) {
                return;
            }
            if !clamp_textarea_input_value(&input_for_event, max_bytes, window, cx) {
                return;
            }
            on_exceeded(this, window, cx);
        },
    )
}

/// 将多行输入立即截到 UTF-8 安全字节边界；发生截断时返回 `true`。
pub fn clamp_multiline_input_value(
    input: &gpui_kit::Entity<gpui_kit::component::input::InputState>,
    max_bytes: usize,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::App,
) -> bool {
    let value = input.read(cx).value();
    if value.len() <= max_bytes {
        return false;
    }
    let bounded = byte_prefix(&value, max_bytes).to_string();
    input.update(cx, |state, cx| {
        state.set_value(bounded, window, cx);
    });
    true
}

pub fn clamp_editor_input_value(
    input: &gpui_kit::Entity<gpui_kit::component::input::EditorState>,
    max_bytes: usize,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::App,
) -> bool {
    let value = input.read(cx).value();
    if value.len() <= max_bytes {
        return false;
    }
    let bounded = byte_prefix(&value, max_bytes).to_string();
    input.update(cx, |state, cx| state.set_value(bounded, window, cx));
    true
}

pub fn clamp_textarea_input_value(
    input: &gpui_kit::Entity<gpui_kit::component::input::TextareaState>,
    max_bytes: usize,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::App,
) -> bool {
    let value = input.read(cx).value();
    if value.len() <= max_bytes {
        return false;
    }
    let bounded = byte_prefix(&value, max_bytes).to_string();
    input.update(cx, |state, cx| state.set_value(bounded, window, cx));
    true
}

fn byte_prefix(value: &str, max_bytes: usize) -> &str {
    let mut end = value.len().min(max_bytes);
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

#[cfg(test)]
mod copy_support_tests;

#[cfg(test)]
mod shared_ui_tests;

#[cfg(test)]
mod dialog_layout_tests;

#[cfg(test)]
mod input_limit_tests {
    use super::{
        byte_prefix, clickable_button, clickable_checkbox, clickable_switch,
        menu_item_with_disabled,
    };
    use gpui_kit::component::menu::PopupMenuItem;
    use gpui_kit::{CursorStyle, Styled as _};

    #[test]
    fn byte_prefix_preserves_utf8_boundaries() {
        assert_eq!(byte_prefix("你好世界", 7), "你好");
        assert_eq!(byte_prefix("abc", 99), "abc");
        assert_eq!(byte_prefix("abc", 0), "");
    }

    #[test]
    fn clickable_components_use_pointing_hand_cursor() {
        let mut button = clickable_button("cursor-test-button");
        let mut checkbox = clickable_checkbox("cursor-test-checkbox");
        let mut switch = clickable_switch("cursor-test-switch");

        assert_eq!(button.style().mouse_cursor, Some(CursorStyle::PointingHand));
        assert_eq!(
            checkbox.style().mouse_cursor,
            Some(CursorStyle::PointingHand)
        );
        assert_eq!(switch.style().mouse_cursor, Some(CursorStyle::PointingHand));
    }

    #[test]
    fn menu_item_preserves_disabled_state() {
        let enabled = menu_item_with_disabled("enabled", false);
        let disabled = menu_item_with_disabled("disabled", true);

        assert!(matches!(
            enabled,
            PopupMenuItem::ElementItem {
                disabled: false,
                ..
            }
        ));
        assert!(matches!(
            disabled,
            PopupMenuItem::ElementItem { disabled: true, .. }
        ));
    }
}
