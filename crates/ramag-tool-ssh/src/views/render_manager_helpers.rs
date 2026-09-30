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
#[derive(Clone, Copy)]
pub(super) struct EnvironmentBadgePalette {
    pub(super) dev: gpui_kit::Hsla,
    pub(super) test: gpui_kit::Hsla,
    pub(super) prod: gpui_kit::Hsla,
    pub(super) fallback: gpui_kit::Hsla,
}

impl EnvironmentBadgePalette {
    pub(super) fn new(
        dev: gpui_kit::Hsla,
        test: gpui_kit::Hsla,
        prod: gpui_kit::Hsla,
        fallback: gpui_kit::Hsla,
    ) -> Self {
        Self {
            dev,
            test,
            prod,
            fallback,
        }
    }
}

pub(super) fn environment_badge_colors(
    environment: &str,
    palette: EnvironmentBadgePalette,
) -> (gpui_kit::Hsla, gpui_kit::Hsla) {
    let foreground = match environment.trim().to_ascii_lowercase().as_str() {
        "dev" => palette.dev,
        "test" => palette.test,
        "prod" => palette.prod,
        _ => palette.fallback,
    };
    let mut background = foreground;
    background.a = 0.12;
    (foreground, background)
}

pub(super) fn workspace_tab_dot_color(
    loading: bool,
    has_error: bool,
    production: bool,
    environment: Option<&str>,
    loading_color: gpui_kit::Hsla,
    danger: gpui_kit::Hsla,
    palette: EnvironmentBadgePalette,
) -> gpui_kit::Hsla {
    if loading {
        loading_color
    } else if has_error || production {
        danger
    } else {
        environment
            .map(|value| environment_badge_colors(value, palette).0)
            .unwrap_or(palette.fallback)
    }
}
