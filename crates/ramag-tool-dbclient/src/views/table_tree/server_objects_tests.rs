use std::collections::{HashMap, HashSet};

use ramag_domain::entities::{ServerObject, ServerObjectGroup};

use super::TableTreeNavigation;
use super::navigation::TableTreeFilter;
use super::row::TreeRow;
use super::rows::build_tree_rows_with_navigation;
use super::server_objects::ServerObjectsState;

#[test]
fn rows_keep_datagrip_group_counts_and_children() {
    let mut state = ServerObjectsState {
        is_expanded: true,
        open_groups: HashSet::from(["collations".to_string()]),
        groups: vec![
            ServerObjectGroup {
                name: "collations".into(),
                items: vec![ServerObject {
                    name: "utf8mb4_0900_ai_ci".into(),
                    detail: Some("utf8mb4".into()),
                }],
            },
            ServerObjectGroup {
                name: "users".into(),
                items: vec![ServerObject {
                    name: "root@localhost".into(),
                    detail: None,
                }],
            },
        ],
        ..Default::default()
    };
    let view = build(&state);
    assert!(view.rows.iter().any(|row| {
        matches!(row, TreeRow::ServerObject { kind: super::server_objects::ServerObjectRowKind::Root, label, .. } if label == "Server Objects")
    }));
    assert!(view.rows.iter().any(|row| {
        matches!(row, TreeRow::ServerObject { kind: super::server_objects::ServerObjectRowKind::Group, label, count: 1, is_expanded: true, .. } if label == "collations")
    }));
    assert!(view.rows.iter().any(|row| {
        matches!(row, TreeRow::ServerObject { kind: super::server_objects::ServerObjectRowKind::Item, label, detail: Some(detail), .. } if label == "utf8mb4_0900_ai_ci" && detail == "utf8mb4")
    }));
    assert!(view.rows.iter().any(|row| {
        matches!(row, TreeRow::ServerObject { kind: super::server_objects::ServerObjectRowKind::Group, label, is_expanded: false, .. } if label == "users")
    }));

    state.is_expanded = false;
    assert_eq!(build(&state).rows.len(), 1);
}

#[test]
fn error_is_not_rendered_as_empty_data() {
    let state = ServerObjectsState {
        is_expanded: true,
        error: Some("权限不足".into()),
        ..Default::default()
    };
    let view = build(&state);
    assert!(view.rows.iter().any(|row| {
        matches!(row, TreeRow::SchemaPlaceholder { text, is_error: true } if text.contains("权限不足"))
    }));
}

fn build(state: &ServerObjectsState) -> super::row::TreeRowsView {
    build_tree_rows_with_navigation(
        &[],
        &HashMap::new(),
        &HashSet::new(),
        &HashMap::new(),
        false,
        "",
        TableTreeNavigation {
            table_filter: TableTreeFilter::All,
            connection_id: None,
            navigation_favorites: &HashSet::new(),
            recent_tables: &[],
            collapsed_table_groups: &HashSet::new(),
            server_objects: Some(state),
        },
    )
}
