use super::*;

fn descriptor() -> Result<PluginDescriptor, PluginRegistrationError> {
    let plugin_id = PluginId::new("example.tool")?;
    Ok(PluginDescriptor::new(plugin_id, "Example", "example")
        .with_description("A built-in example tool")
        .with_capabilities([
            PluginCapability::new("ui.entry"),
            PluginCapability::new("task.scoped"),
        ])
        .with_settings(vec![
            PluginSettingDefinition::new("mode", PluginSettingKind::Enum)
                .with_enum_values(["safe", "full"])
                .with_default(PluginSettingValue::String("safe".into())),
        ]))
}

#[test]
fn valid_descriptor_passes_all_registration_checks() {
    let Ok(descriptor) = descriptor() else {
        return;
    };
    assert!(descriptor.validate().is_ok());
    assert!(PluginApiVersion::new(1, 0).is_supported_by(CURRENT_PLUGIN_API_VERSION));
}

#[test]
fn identifiers_reject_uppercase_and_path_characters() {
    assert!(matches!(
        PluginId::new("Example.Tool"),
        Err(PluginRegistrationError::InvalidIdentifier { .. })
    ));
    assert!(matches!(
        PluginId::new("../tool"),
        Err(PluginRegistrationError::InvalidIdentifier { .. })
    ));
}

#[test]
fn unsupported_api_version_is_rejected() {
    let Ok(descriptor) = descriptor() else {
        return;
    };
    let descriptor = descriptor.with_api_version(PluginApiVersion::new(2, 0));
    assert!(matches!(
        descriptor.validate(),
        Err(PluginRegistrationError::UnsupportedApiVersion { .. })
    ));
}

#[test]
fn unknown_and_duplicate_capabilities_are_rejected() {
    let Ok(descriptor) = descriptor() else {
        return;
    };
    let unknown = descriptor
        .clone()
        .with_capabilities([PluginCapability::new("network.any")]);
    assert!(matches!(
        unknown.validate(),
        Err(PluginRegistrationError::UnknownCapability { .. })
    ));

    let duplicate = descriptor.with_capabilities([
        PluginCapability::new("ui.entry"),
        PluginCapability::new("ui.entry"),
    ]);
    assert!(matches!(
        duplicate.validate(),
        Err(PluginRegistrationError::DuplicateCapability { .. })
    ));
}

#[test]
fn setting_schema_rejects_type_and_enum_default_mismatches() {
    let Ok(descriptor) = descriptor() else {
        return;
    };
    let wrong_type = descriptor.clone().with_settings(vec![
        PluginSettingDefinition::new("enabled", PluginSettingKind::Boolean)
            .with_default(PluginSettingValue::String("yes".into())),
    ]);
    assert!(matches!(
        wrong_type.validate(),
        Err(PluginRegistrationError::InvalidSetting { .. })
    ));

    let wrong_enum = descriptor.with_settings(vec![
        PluginSettingDefinition::new("mode", PluginSettingKind::Enum)
            .with_enum_values(["safe", "full"])
            .with_default(PluginSettingValue::String("unknown".into())),
    ]);
    assert!(matches!(
        wrong_enum.validate(),
        Err(PluginRegistrationError::InvalidSetting { .. })
    ));
}

#[test]
fn deserialized_invalid_descriptor_is_caught_before_registration() {
    let Ok(descriptor) = descriptor() else {
        return;
    };
    let serialized = serde_json::to_value(descriptor);
    assert!(serialized.is_ok());
    let Some(mut value) = serialized.ok() else {
        return;
    };
    value["id"] = serde_json::json!("Bad.Plugin");
    let deserialized: Result<PluginDescriptor, _> = serde_json::from_value(value);
    assert!(deserialized.is_ok());
    let Some(descriptor) = deserialized.ok() else {
        return;
    };
    assert!(matches!(
        descriptor.validate(),
        Err(PluginRegistrationError::InvalidIdentifier { .. })
    ));
}

#[test]
fn multi_entry_descriptor_validates_data_specs_and_keeps_primary_entry() {
    let primary = PluginEntryDescriptor::new("example", "Example")
        .with_input(PluginEntryDataSpec::json(4096))
        .with_output(PluginEntryDataSpec::text(2048));
    let secondary = PluginEntryDescriptor::new("example.preview", "Preview")
        .with_description("Preview output")
        .with_input(PluginEntryDataSpec::text(1024))
        .with_output(PluginEntryDataSpec::json(8192));
    let Ok(descriptor) = descriptor() else {
        return;
    };
    let descriptor = descriptor.with_entries(vec![primary, secondary]);

    assert!(descriptor.validate().is_ok());
    assert_eq!(descriptor.entry_descriptors().len(), 2);
    assert_eq!(
        descriptor.entry_descriptors()[1].output.kind,
        PluginEntryDataKind::Json
    );
}

#[test]
fn invalid_multi_entry_descriptor_is_rejected_without_unbounded_payload() {
    let entries = vec![PluginEntryDescriptor::new("example", "Example").with_input(
        PluginEntryDataSpec::text(MAX_PLUGIN_ENTRY_PAYLOAD_BYTES + 1),
    )];
    let Ok(descriptor) = descriptor() else {
        return;
    };
    let descriptor = descriptor.with_entries(entries);

    assert!(matches!(
        descriptor.validate(),
        Err(PluginRegistrationError::InvalidEntry { .. })
    ));
}

#[test]
fn multi_entry_descriptor_rejects_duplicate_ids_and_missing_primary() {
    let duplicate_entries = vec![
        PluginEntryDescriptor::new("example", "Example"),
        PluginEntryDescriptor::new("example", "Duplicate"),
    ];
    let Ok(descriptor) = descriptor() else {
        return;
    };
    assert!(matches!(
        descriptor
            .clone()
            .with_entries(duplicate_entries)
            .validate(),
        Err(PluginRegistrationError::DuplicateEntryDescriptor { .. })
    ));

    let missing_primary =
        descriptor.with_entries(vec![PluginEntryDescriptor::new("example.other", "Other")]);
    assert!(matches!(
        missing_primary.validate(),
        Err(PluginRegistrationError::MissingPrimaryEntry { .. })
    ));
}
