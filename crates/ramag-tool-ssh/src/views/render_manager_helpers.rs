use gpui_kit::component::v_flex;
use gpui_kit::{IntoElement, ParentElement as _, Styled as _};

/// Build the centered status element used by the manager's loading state.
pub(super) fn centered_message(message: &'static str, color: gpui_kit::Hsla) -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .child(gpui_kit::div().text_sm().text_color(color).child(message))
}

/// Map a profile environment to a readable badge color while preserving the
/// current theme's fallback for unknown values.
pub(super) fn environment_badge_colors(
    environment: &str,
    fallback: gpui_kit::Hsla,
) -> (gpui_kit::Hsla, gpui_kit::Hsla) {
    let foreground = match environment.trim().to_ascii_lowercase().as_str() {
        "dev" => gpui_kit::hsla(140.0 / 360.0, 0.55, 0.42, 1.0),
        "test" => gpui_kit::hsla(35.0 / 360.0, 0.80, 0.45, 1.0),
        "prod" => gpui_kit::hsla(0.0, 0.70, 0.55, 1.0),
        _ => fallback,
    };
    let mut background = foreground;
    background.a = 0.12;
    (foreground, background)
}
