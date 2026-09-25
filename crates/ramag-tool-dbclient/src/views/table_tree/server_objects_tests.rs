use std::collections::{HashMap, HashSet};

use ramag_domain::entities::{ServerObject, ServerObjectGroup, VirtualView};

use super::TableTreeNavigation;
use super::navigation::TableTreeFilter;
use super::row::TreeRow;
use super::rows::build_tree_rows_with_navigation;
use super::server_objects::{ExplorerRowKind, ServerObjectsState};
use super::virtual_views::VirtualViewsState;

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
        matches!(row, TreeRow::ServerObject { kind: ExplorerRowKind::Root, label, .. } if label == "Server Objects")
    }));
    assert!(view.rows.iter().any(|row| {
        matches!(row, TreeRow::ServerObject { kind: ExplorerRowKind::Group, label, count: 1, is_expanded: true, .. } if label == "collations")
    }));
    assert!(view.rows.iter().any(|row| {
        matches!(row, TreeRow::ServerObject { kind: ExplorerRowKind::Item, label, detail: Some(detail), .. } if label == "utf8mb4_0900_ai_ci" && detail == "utf8mb4")
    }));
    assert!(view.rows.iter().any(|row| {
        matches!(row, TreeRow::ServerObject { kind: ExplorerRowKind::Group, label, is_expanded: false, .. } if label == "users")
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

#[test]
fn virtual_views_keep_read_only_sessions_visible_and_filterable() {
    let state = VirtualViewsState {
        is_expanded: true,
        views: vec![VirtualView {
            name: "sessions".into(),
            detail: Some("只读会话快照".into()),
            read_only: true,
        }],
        ..Default::default()
    };
    let view = build_with_virtual_views(&state, "");
    assert!(view.rows.iter().any(|row| {
        matches!(
            row,
            TreeRow::ServerObject {
                kind: ExplorerRowKind::VirtualRoot,
                label,
                count: 1,
                ..
            } if label == "Virtual views"
        )
    }));
    assert!(view.rows.iter().any(|row| {
        matches!(
            row,
            TreeRow::ServerObject {
                kind: ExplorerRowKind::VirtualItem,
                label,
                detail: Some(detail),
                ..
            } if label == "sessions" && detail == "只读会话快照"
        )
    }));
    assert!(
        build_with_virtual_views(&state, "sessions")
            .rows
            .iter()
            .any(|row| {
                matches!(
                    row,
                    TreeRow::ServerObject {
                        kind: ExplorerRowKind::VirtualItem,
                        ..
                    }
                )
            })
    );
}

#[test]
fn virtual_views_keep_refresh_error_distinct_from_empty_state() {
    let state = VirtualViewsState {
        is_expanded: true,
        error: Some("权限不足".into()),
        ..Default::default()
    };
    let view = build_with_virtual_views(&state, "");
    assert!(view.rows.iter().any(|row| {
        matches!(row, TreeRow::SchemaPlaceholder { text, is_error: true } if text.contains("权限不足"))
    }));
    assert!(!view.rows.iter().any(|row| {
        matches!(row, TreeRow::SchemaPlaceholder { text, is_error: false } if text == "（无会话）")
    }));
}

fn build(state: &ServerObjectsState) -> super::row::TreeRowsView {
    build_with_virtual_views_inner(Some(state), None, "")
}

fn build_with_virtual_views(state: &VirtualViewsState, filter: &str) -> super::row::TreeRowsView {
    build_with_virtual_views_inner(None, Some(state), filter)
}

fn build_with_virtual_views_inner(
    server_objects: Option<&ServerObjectsState>,
    virtual_views: Option<&VirtualViewsState>,
    filter: &str,
) -> super::row::TreeRowsView {
    build_tree_rows_with_navigation(
        &[],
        &HashMap::new(),
        &HashSet::new(),
        &HashMap::new(),
        false,
        filter,
        TableTreeNavigation {
            table_filter: TableTreeFilter::All,
            connection_id: None,
            navigation_favorites: &HashSet::new(),
            recent_tables: &[],
            collapsed_table_groups: &HashSet::new(),
            server_objects,
            virtual_views,
        },
    )
}
