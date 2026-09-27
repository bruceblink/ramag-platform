use super::*;

fn schema(name: &str) -> Schema {
    Schema {
        name: name.to_string(),
        charset: None,
        collation: None,
    }
}

#[test]
fn new_mysql_connection_opens_configured_database() {
    let connection = ConnectionConfig::new_mysql("local", "127.0.0.1", 13306, "ramag_ui_test");
    let schemas = vec![schema("ramag_ui_test"), schema("information_schema")];
    let mut active_schema = None;
    let mut open_schemas = HashSet::new();

    let opened =
        open_default_schema_if_needed(&connection, &schemas, &mut active_schema, &mut open_schemas);

    assert_eq!(opened.as_deref(), Some("ramag_ui_test"));
    assert_eq!(active_schema.as_deref(), Some("ramag_ui_test"));
    assert!(open_schemas.contains("ramag_ui_test"));
}

#[test]
fn existing_schema_selection_is_not_reopened_automatically() {
    let connection = ConnectionConfig::new_mysql("local", "127.0.0.1", 13306, "ramag_ui_test");
    let schemas = vec![schema("ramag_ui_test")];
    let mut active_schema = Some("archive".to_string());
    let mut open_schemas = HashSet::new();

    assert_eq!(
        open_default_schema_if_needed(&connection, &schemas, &mut active_schema, &mut open_schemas,),
        None
    );
    assert!(open_schemas.is_empty());
    assert_eq!(active_schema.as_deref(), Some("archive"));
}

#[test]
fn only_initial_schema_load_uses_fullscreen_state() {
    assert!(show_fullscreen_schema_loading(&[], true));
    assert!(!show_fullscreen_schema_loading(&[schema("public")], true));
    assert!(show_fullscreen_schema_error(&[], Some("offline")));
    assert!(!show_fullscreen_schema_error(
        &[schema("public")],
        Some("offline")
    ));
}
