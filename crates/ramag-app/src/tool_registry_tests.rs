use super::*;
use ramag_domain::{PluginEntryDataSpec, PluginEntryDescriptor, ToolMeta};

struct DummyTool {
    meta: ToolMeta,
}

impl Tool for DummyTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

fn dummy(id: &str, name: &str) -> Arc<DummyTool> {
    Arc::new(DummyTool {
        meta: ToolMeta::new(id, name, ""),
    })
}

#[test]
fn register_and_list() {
    let reg = ToolRegistry::new();
    reg.register(dummy("a", "ToolA"));
    reg.register(dummy("b", "ToolB"));
    assert_eq!(reg.count(), 2);
    assert!(reg.find("a").is_some());
    assert!(reg.find("missing").is_none());
}

#[test]
fn duplicate_registration_ignored() {
    let reg = ToolRegistry::new();
    reg.register(dummy("dup", "Tool1"));
    reg.register(dummy("dup", "Tool2"));
    assert_eq!(reg.count(), 1);
}

#[test]
fn disabled_tool_hidden_from_list_and_find() {
    let reg = ToolRegistry::new();
    reg.register(dummy("a", "ToolA"));
    reg.register(dummy("b", "ToolB"));

    assert!(reg.set_enabled("a", false));
    assert_eq!(reg.count(), 1);
    assert!(reg.find("a").is_none());
    assert_eq!(reg.list().len(), 1);
    assert!(!reg.set_enabled("a", false));
    assert!(!reg.set_enabled("missing", true));

    assert!(reg.set_enabled("a", true));
    assert!(reg.find("a").is_some());
    assert_eq!(reg.count(), 2);
}

#[test]
fn reorder_updates_visible_tool_order() {
    let reg = ToolRegistry::new();
    reg.register(dummy("a", "ToolA"));
    reg.register(dummy("b", "ToolB"));
    reg.register(dummy("c", "ToolC"));

    assert!(reg.reorder("c", "a", true));
    assert_eq!(reg.order(), ["c", "a", "b"]);
    assert!(reg.reorder("c", "b", false));
    assert_eq!(reg.order(), ["a", "b", "c"]);
    assert!(!reg.reorder("a", "missing", true));
}

#[test]
fn reorder_to_target_moves_item_into_target_slot() {
    let reg = ToolRegistry::new();
    reg.register(dummy("a", "ToolA"));
    reg.register(dummy("b", "ToolB"));
    reg.register(dummy("c", "ToolC"));
    reg.register(dummy("d", "ToolD"));

    assert!(reg.reorder_to_target("a", "c"));
    assert_eq!(reg.order(), ["b", "c", "a", "d"]);
    assert!(reg.reorder_to_target("d", "b"));
    assert_eq!(reg.order(), ["d", "b", "c", "a"]);
    assert!(!reg.reorder_to_target("a", "a"));
}

#[test]
fn move_to_end_places_item_after_last_visible_tool() {
    let reg = ToolRegistry::new();
    reg.register(dummy("a", "ToolA"));
    reg.register(dummy("b", "ToolB"));
    reg.register(dummy("c", "ToolC"));

    assert!(reg.move_to_end("b"));
    assert_eq!(reg.order(), ["a", "c", "b"]);
    assert!(!reg.move_to_end("b"));
}

#[test]
fn reorder_to_index_moves_item_to_the_requested_visible_slot() {
    let reg = ToolRegistry::new();
    reg.register(dummy("a", "ToolA"));
    reg.register(dummy("b", "ToolB"));
    reg.register(dummy("c", "ToolC"));

    assert!(reg.reorder_to_index("c", 0));
    assert_eq!(reg.order(), ["c", "a", "b"]);
    assert!(reg.reorder_to_index("c", 3));
    assert_eq!(reg.order(), ["a", "b", "c"]);
    assert!(!reg.reorder_to_index("b", 1));
}

#[test]
fn reorder_to_index_keeps_disabled_tools_in_the_registry() {
    let reg = ToolRegistry::new();
    reg.register(dummy("a", "ToolA"));
    reg.register(dummy("hidden", "Hidden"));
    reg.register(dummy("b", "ToolB"));
    assert!(reg.set_enabled("hidden", false));

    assert!(reg.reorder_to_index("b", 0));
    assert_eq!(reg.order(), ["b", "a", "hidden"]);
    assert_eq!(reg.list().len(), 2);
}

#[test]
fn apply_order_keeps_new_tools_after_saved_tools() {
    let reg = ToolRegistry::new();
    reg.register(dummy("a", "ToolA"));
    reg.register(dummy("b", "ToolB"));
    reg.register(dummy("c", "ToolC"));

    assert!(reg.apply_order(&["b".into(), "a".into()]));
    assert_eq!(reg.order(), ["b", "a", "c"]);
    assert!(reg.apply_order_json(r#"["c","a"]"#).unwrap());
    assert_eq!(reg.order(), ["c", "a", "b"]);
    assert!(reg.apply_order_json("not-json").is_err());
}

#[test]
fn builtin_registration_keeps_tool_behavior_and_records_descriptor() {
    let reg = ToolRegistry::new();
    reg.register_builtin(dummy("example", "Example")).unwrap();

    assert_eq!(reg.order(), ["example"]);
    let plugins = reg.plugin_descriptors();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].id.as_str(), "example");
    assert_eq!(plugins[0].entry_id, "example");
}

#[test]
fn plugin_registration_rejects_duplicate_and_mismatched_entries() {
    let reg = ToolRegistry::new();
    reg.register_builtin(dummy("example", "Example")).unwrap();

    let duplicate = PluginDescriptor::new(
        PluginId::new("example").unwrap(),
        "Another Example",
        "other",
    );
    assert!(matches!(
        reg.register_plugin(duplicate, dummy("other", "Other")),
        Err(PluginRegistrationError::DuplicatePluginId { .. })
    ));

    let mismatch = PluginDescriptor::new(
        PluginId::new("different").unwrap(),
        "Different",
        "different",
    );
    assert!(matches!(
        reg.register_plugin(mismatch, dummy("another", "Another")),
        Err(PluginRegistrationError::EntryIdMismatch { .. })
    ));
    assert_eq!(reg.order(), ["example"]);
}

#[test]
fn multi_entry_plugin_registers_and_unregisters_as_one_plugin() {
    let reg = ToolRegistry::new();
    let descriptor = PluginDescriptor::new(
        PluginId::new("example.bundle").unwrap(),
        "Example bundle",
        "example.first",
    )
    .with_entries(vec![
        PluginEntryDescriptor::new("example.first", "First")
            .with_input(PluginEntryDataSpec::json(2048)),
        PluginEntryDescriptor::new("example.second", "Second"),
    ]);

    reg.register_plugin_entries(
        descriptor,
        vec![
            dummy("example.first", "First"),
            dummy("example.second", "Second"),
        ],
    )
    .unwrap();

    assert_eq!(reg.count(), 2);
    assert_eq!(reg.plugin_descriptors().len(), 1);
    assert!(reg.unregister_plugin("example.bundle"));
    assert_eq!(reg.count(), 0);
}

#[test]
fn multi_entry_registration_is_atomic_when_an_entry_is_missing() {
    let reg = ToolRegistry::new();
    let descriptor = PluginDescriptor::new(
        PluginId::new("example.bundle").unwrap(),
        "Example bundle",
        "example.first",
    )
    .with_entries(vec![
        PluginEntryDescriptor::new("example.first", "First"),
        PluginEntryDescriptor::new("example.second", "Second"),
    ]);

    let result = reg.register_plugin_entries(descriptor, vec![dummy("example.first", "First")]);
    assert!(matches!(
        result,
        Err(PluginRegistrationError::EntryCountMismatch { .. })
    ));
    assert_eq!(reg.count(), 0);
}
