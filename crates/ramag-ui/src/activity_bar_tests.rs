use ramag_app::AvailableUpdate;
use ramag_domain::entities::ReleaseInfo;

use super::{UpdateCheckResult, indicator_value, tool_icon_path};

fn available_result() -> UpdateCheckResult {
    UpdateCheckResult::Available(AvailableUpdate {
        release: ReleaseInfo {
            version: "0.0.3".into(),
            tag_name: "v0.0.3".into(),
            release_url: "https://github.com/bruceblink/ramag-platform/releases/tag/v0.0.3".into(),
            notes: String::new(),
            published_at: None,
            assets: Vec::new(),
        },
        asset: None,
    })
}

#[test]
fn update_indicator_tracks_only_real_update_results() {
    assert!(!indicator_value(&UpdateCheckResult::UpToDate {
        current_version: "0.0.2".into(),
        latest_version: "0.0.2".into(),
    }));
    let available = available_result();
    assert!(indicator_value(&available));
    let UpdateCheckResult::Available(update) = available else {
        unreachable!();
    };
    assert!(!indicator_value(&UpdateCheckResult::UnsupportedPlatform(
        update
    )));
}

#[test]
fn builtin_tool_icons_are_unique_and_semantic() {
    let entries = [
        ("dbclient", Some("database")),
        ("vcs", Some("git_branch")),
        ("clipboard", Some("clipboard")),
        ("ssh", Some("terminal")),
        ("system", Some("gauge")),
        ("container", Some("box")),
        ("kafka", Some("server")),
        ("mqtt", Some("mqtt")),
        ("api", Some("api")),
        ("json-path-extractor", Some("braces")),
        ("object_storage", Some("cloud")),
        ("collaboration", Some("users")),
    ];
    let paths = entries
        .iter()
        .map(|(id, icon)| tool_icon_path(id, *icon))
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(paths.len(), entries.len());
    assert_eq!(
        tool_icon_path("ssh", Some("terminal")),
        "icons/terminal.svg"
    );
    assert_eq!(
        tool_icon_path("json-path-extractor", Some("braces")),
        "icons/json.svg"
    );
    assert_eq!(
        tool_icon_path("object_storage", Some("cloud")),
        "icons/cloud.svg"
    );
}

#[test]
fn unknown_plugin_icon_uses_local_toolbox_fallback() {
    assert_eq!(
        tool_icon_path("plugin.example", Some("external-icon")),
        "icons/toolbox.svg"
    );
}
