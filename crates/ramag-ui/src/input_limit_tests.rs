use super::{
    byte_prefix, clickable_button, clickable_checkbox, clickable_switch, menu_item_with_disabled,
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
