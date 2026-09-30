use ramag_domain::entities::{RemotePlatformPreference, SshAuthMode, SshProfile, SshProfileOrigin};

use super::{
    EnvironmentBadgePalette, environment_badge_colors, is_jumpserver_profile, platform_label,
    profile_matches_query,
};

#[test]
fn environment_badges_follow_the_active_theme_palette() {
    let palette = EnvironmentBadgePalette {
        dev: gpui_kit::hsla(0.1, 0.2, 0.3, 1.0),
        test: gpui_kit::hsla(0.2, 0.3, 0.4, 1.0),
        prod: gpui_kit::hsla(0.3, 0.4, 0.5, 1.0),
        fallback: gpui_kit::hsla(0.4, 0.5, 0.6, 1.0),
    };

    assert_eq!(environment_badge_colors("dev", palette).0, palette.dev);
    assert_eq!(environment_badge_colors("TEST", palette).0, palette.test);
    assert_eq!(environment_badge_colors(" prod ", palette).0, palette.prod);
    assert_eq!(
        environment_badge_colors("staging", palette).0,
        palette.fallback
    );

    for environment in ["dev", "test", "prod", "staging"] {
        let (_, background) = environment_badge_colors(environment, palette);
        assert_eq!(background.a, 0.12);
    }
}

#[test]
fn profile_search_matches_name_host_and_username() {
    let mut profile = SshProfile::new("Production", "SERVER.EXAMPLE");
    profile.username = "Alice".into();
    profile.environment = Some("staging".into());

    assert!(profile_matches_query(&profile, "production"));
    assert!(profile_matches_query(&profile, "server.example"));
    assert!(profile_matches_query(&profile, "alice"));
    assert!(profile_matches_query(&profile, "staging"));
    assert!(!profile_matches_query(&profile, "missing"));
}

#[test]
fn jumpserver_icon_supports_explicit_and_legacy_profiles() {
    let mut explicit = SshProfile::new("asset", "jump.example");
    explicit.origin = SshProfileOrigin::JumpServer;
    assert!(is_jumpserver_profile(&explicit));

    let mut legacy = SshProfile::new("asset", "jump.example");
    legacy.auth_mode = SshAuthMode::Password;
    legacy.username = "login#root#00000000-0000-0000-0000-000000000000".into();
    assert!(is_jumpserver_profile(&legacy));

    legacy.username = "ordinary-user".into();
    assert!(!is_jumpserver_profile(&legacy));
}

#[test]
fn platform_labels_are_stable_for_manager_badges() {
    assert_eq!(platform_label(RemotePlatformPreference::Windows), "Windows");
    assert_eq!(platform_label(RemotePlatformPreference::Linux), "Linux");
    assert_eq!(platform_label(RemotePlatformPreference::Auto), "自动");
}
