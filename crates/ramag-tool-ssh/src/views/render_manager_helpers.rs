use gpui_kit::component::v_flex;
use gpui_kit::{
    InteractiveElement as _, IntoElement, ParentElement as _, SharedString, Styled as _, div, px,
};
use ramag_domain::entities::{
    RemotePlatformPreference, SshAuthMode, SshProfile, SshProfileOrigin, contains_case_insensitive,
};

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

pub(super) fn profile_matches_query(profile: &SshProfile, query: &str) -> bool {
    contains_case_insensitive(&profile.name, query)
        || contains_case_insensitive(&profile.host, query)
        || contains_case_insensitive(&profile.username, query)
        || profile
            .environment
            .as_deref()
            .is_some_and(|environment| contains_case_insensitive(environment, query))
}

pub(super) fn is_jumpserver_profile(profile: &SshProfile) -> bool {
    profile.origin == SshProfileOrigin::JumpServer
        || (profile.auth_mode == SshAuthMode::Password
            && is_legacy_jumpserver_username(&profile.username))
}

fn is_legacy_jumpserver_username(username: &str) -> bool {
    let mut parts = username.split('#');
    let (Some(login), Some(account), Some(asset_id)) = (parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    !login.is_empty() && !account.is_empty() && parts.next().is_none() && looks_like_uuid(asset_id)
}

fn looks_like_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

pub(super) fn secondary_column(
    width: f32,
    text: String,
    color: gpui_kit::Hsla,
) -> impl IntoElement {
    div()
        .flex_none()
        .w(px(width))
        .text_xs()
        .text_color(color)
        .overflow_hidden()
        .text_ellipsis()
        .child(text)
}

pub(super) fn environment_badge(
    index: usize,
    environment: String,
    palette: EnvironmentBadgePalette,
) -> impl IntoElement {
    let slot = div()
        .debug_selector(move || format!("ssh-profile-environment-{index}"))
        .flex_none()
        .w(px(64.0))
        .flex()
        .justify_center();
    if environment.trim().is_empty() {
        slot
    } else {
        let (foreground, background) = environment_badge_colors(&environment, palette);
        slot.child(
            div()
                .px(px(6.0))
                .py(px(1.0))
                .rounded(px(4.0))
                .text_xs()
                .text_color(foreground)
                .bg(background)
                .max_w_full()
                .overflow_hidden()
                .text_ellipsis()
                .child(environment),
        )
    }
}

pub(super) fn platform_badge(
    index: usize,
    platform: RemotePlatformPreference,
    color: gpui_kit::Hsla,
) -> impl IntoElement {
    let mut background = color;
    background.a = 0.12;
    status_badge(
        format!("ssh-profile-platform-{index}"),
        76.0,
        platform_label(platform),
        color,
        Some(background),
    )
}

pub(super) fn platform_label(platform: RemotePlatformPreference) -> &'static str {
    match platform {
        RemotePlatformPreference::Auto => "自动",
        RemotePlatformPreference::Linux => "Linux",
        RemotePlatformPreference::Windows => "Windows",
    }
}

pub(super) fn status_badge(
    id: String,
    width: f32,
    label: &'static str,
    foreground: gpui_kit::Hsla,
    background: Option<gpui_kit::Hsla>,
) -> impl IntoElement {
    let debug_selector = id.clone();
    let mut slot = div()
        .id(SharedString::from(id))
        .debug_selector(move || debug_selector.clone())
        .flex_none()
        .w(px(width))
        .flex()
        .justify_center();
    if let Some(background) = background {
        slot = slot.child(
            div()
                .px(px(6.0))
                .py(px(1.0))
                .rounded(px(4.0))
                .text_xs()
                .text_color(foreground)
                .bg(background)
                .overflow_hidden()
                .text_ellipsis()
                .child(label),
        );
    } else {
        slot = slot.child(div().text_xs().text_color(foreground).child(label));
    }
    slot
}
