use ramag_domain::{
    MAX_PLUGIN_SETTING_LIST_ITEMS, MAX_PLUGIN_SETTING_VALUE_BYTES, PluginDescriptor, PluginId,
    PluginSettingDefinition, PluginSettingKind, PluginSettingValue,
};

use super::*;

fn descriptor() -> PluginDescriptor {
    PluginDescriptor::new(
        PluginId::new("example.tool").unwrap(),
        "Example Tool",
        "example.tool",
    )
    .with_settings(vec![
        PluginSettingDefinition::new("enabled", PluginSettingKind::Boolean)
            .with_default(PluginSettingValue::Boolean(true)),
        PluginSettingDefinition::new("mode", PluginSettingKind::Enum)
            .with_enum_values(["safe", "full"])
            .with_default(PluginSettingValue::String("safe".into())),
        PluginSettingDefinition::new("tags", PluginSettingKind::StringList),
    ])
}

#[test]
fn snapshot_is_namespaced_and_fills_declared_defaults() {
    let snapshot = PluginSettingsSnapshot::from_namespaced_values(
        &descriptor(),
        [(
            "plugin.example.tool.mode",
            PluginSettingValue::String("full".into()),
        )],
    )
    .unwrap();

    assert_eq!(snapshot.namespace(), "plugin.example.tool.");
    assert_eq!(
        snapshot.get("enabled"),
        Some(&PluginSettingValue::Boolean(true))
    );
    assert_eq!(
        snapshot.get_namespaced("plugin.example.tool.mode").unwrap(),
        Some(&PluginSettingValue::String("full".into()))
    );
    assert_eq!(
        snapshot.qualified_key("tags").unwrap(),
        "plugin.example.tool.tags"
    );
}

#[test]
fn foreign_namespace_is_rejected_before_values_are_stored() {
    let error = PluginSettingsSnapshot::from_namespaced_values(
        &descriptor(),
        [(
            "plugin.other.tool.enabled",
            PluginSettingValue::Boolean(true),
        )],
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PluginSettingsError::NamespaceMismatch { expected, key }
            if expected == "plugin.example.tool." && key == "plugin.other.tool.enabled"
    ));
}

#[test]
fn undeclared_and_duplicate_keys_are_rejected() {
    let unknown = PluginSettingsSnapshot::from_namespaced_values(
        &descriptor(),
        [(
            "plugin.example.tool.missing",
            PluginSettingValue::Boolean(true),
        )],
    )
    .unwrap_err();
    assert!(matches!(
        unknown,
        PluginSettingsError::UnknownSetting { .. }
    ));

    let duplicate = PluginSettingsSnapshot::from_namespaced_values(
        &descriptor(),
        [
            (
                "plugin.example.tool.enabled",
                PluginSettingValue::Boolean(true),
            ),
            (
                "plugin.example.tool.enabled",
                PluginSettingValue::Boolean(false),
            ),
        ],
    )
    .unwrap_err();
    assert!(matches!(
        duplicate,
        PluginSettingsError::DuplicateKey { .. }
    ));
}

#[test]
fn wrong_type_and_over_limit_values_are_rejected() {
    let snapshot = PluginSettingsSnapshot::from_namespaced_values(
        &descriptor(),
        std::iter::empty::<(String, PluginSettingValue)>(),
    )
    .unwrap();

    let wrong_type = snapshot.update("enabled", PluginSettingValue::String("yes".into()));
    assert!(matches!(
        wrong_type,
        Err(PluginSettingsError::InvalidValue { .. })
    ));

    let over_limit = snapshot.update(
        "tags",
        PluginSettingValue::StringList(
            (0..=MAX_PLUGIN_SETTING_LIST_ITEMS)
                .map(|index| format!("tag-{index}"))
                .collect(),
        ),
    );
    assert!(matches!(
        over_limit,
        Err(PluginSettingsError::InvalidValue { .. })
    ));

    let over_text_limit = snapshot.update(
        "mode",
        PluginSettingValue::String("x".repeat(MAX_PLUGIN_SETTING_VALUE_BYTES + 1)),
    );
    assert!(matches!(
        over_text_limit,
        Err(PluginSettingsError::InvalidValue { .. })
    ));
}

#[test]
fn update_returns_new_snapshot_without_mutating_previous_values() {
    let snapshot = PluginSettingsSnapshot::from_namespaced_values(
        &descriptor(),
        std::iter::empty::<(String, PluginSettingValue)>(),
    )
    .unwrap();
    let updated = snapshot
        .update("mode", PluginSettingValue::String("full".into()))
        .unwrap();

    assert_eq!(
        snapshot.get("mode"),
        Some(&PluginSettingValue::String("safe".into()))
    );
    assert_eq!(
        updated.get("mode"),
        Some(&PluginSettingValue::String("full".into()))
    );
}

#[test]
fn namespaced_reads_and_updates_reject_foreign_keys() {
    let snapshot = PluginSettingsSnapshot::from_namespaced_values(
        &descriptor(),
        std::iter::empty::<(String, PluginSettingValue)>(),
    )
    .unwrap();

    assert!(matches!(
        snapshot.get_namespaced("plugin.other.tool.enabled"),
        Err(PluginSettingsError::NamespaceMismatch { .. })
    ));
    assert!(matches!(
        snapshot.update_namespaced(
            "plugin.other.tool.enabled",
            PluginSettingValue::Boolean(false),
        ),
        Err(PluginSettingsError::NamespaceMismatch { .. })
    ));
}

#[test]
fn sensitive_settings_are_not_loaded_into_the_plain_snapshot() {
    let sensitive_descriptor = descriptor().with_settings(vec![
        PluginSettingDefinition::new("token", PluginSettingKind::String).sensitive(true),
    ]);

    let error = PluginSettingsSnapshot::from_namespaced_values(
        &sensitive_descriptor,
        [(
            "plugin.example.tool.token",
            PluginSettingValue::String("secret".into()),
        )],
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PluginSettingsError::SensitiveSettingRequiresSecretStorage { key } if key == "token"
    ));
}

#[test]
fn plain_snapshot_skips_sensitive_defaults_but_keeps_regular_settings() {
    let mixed = descriptor().with_settings(vec![
        PluginSettingDefinition::new("token", PluginSettingKind::String)
            .sensitive(true)
            .with_default(PluginSettingValue::String("secret".into())),
        PluginSettingDefinition::new("enabled", PluginSettingKind::Boolean)
            .with_default(PluginSettingValue::Boolean(true)),
    ]);
    let snapshot = PluginSettingsSnapshot::from_namespaced_values(
        &mixed,
        std::iter::empty::<(String, PluginSettingValue)>(),
    )
    .unwrap();

    assert_eq!(snapshot.get("token"), None);
    assert_eq!(
        snapshot.get("enabled"),
        Some(&PluginSettingValue::Boolean(true))
    );
}
