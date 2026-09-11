//! DB Client 视图集合：DbClientView 根视图 + 连接 / 表单 / 表树等子面板

pub mod cell_edit_dialog;
pub mod connection_form;
pub mod connection_list;
pub mod connection_session;
mod connection_target;
pub mod data_sync_dialog;
pub mod dbclient_view;
pub mod history_dialog;
pub mod query_panel;
pub mod query_tab;
mod recent_connections_dialog;
mod result_diff;
mod result_diff_dialog;
pub mod result_panel;
pub mod result_table;
mod result_value;
mod result_value_dialog;
mod schema_diagram;
mod schema_diff;
mod schema_diff_dialog;
mod schema_migration;
mod table_designer;
mod table_properties;
pub mod table_tree;
pub mod tree_helpers;

pub use dbclient_view::DbClientView;

pub(super) const COMPACT_SESSION_BREAKPOINT: f32 = 720.0;

/// Keeps the database session responsive before a fixed tree would starve the query area.
pub(super) fn is_compact_session_width(width: f32) -> bool {
    width < COMPACT_SESSION_BREAKPOINT
}

pub(super) fn inline_text_preview(text: &str, max_chars: usize) -> String {
    let mut output = String::with_capacity(text.len().min(max_chars.saturating_mul(4)));
    let mut chars = text.chars();
    for character in chars.by_ref().take(max_chars) {
        if matches!(character, '\n' | '\r' | '\t') {
            output.push(' ');
        } else if character.is_control() {
            output.push('\u{fffd}');
        } else {
            output.push(character);
        }
    }
    if chars.next().is_some() {
        output.push('…');
    }
    output
}

#[cfg(test)]
mod inline_text_tests {
    use super::inline_text_preview;

    #[test]
    fn inline_preview_is_single_line_and_unicode_safe() {
        assert_eq!(inline_text_preview("你好世界", 2), "你好…");
        assert_eq!(inline_text_preview("a\nb\0c", 20), "a b�c");
    }
}

#[cfg(test)]
mod session_layout_tests {
    use super::{COMPACT_SESSION_BREAKPOINT, is_compact_session_width};

    #[test]
    fn compact_session_breakpoint_preserves_a_usable_query_area() {
        assert!(is_compact_session_width(COMPACT_SESSION_BREAKPOINT - 1.0));
        assert!(!is_compact_session_width(COMPACT_SESSION_BREAKPOINT));
        assert!(!is_compact_session_width(COMPACT_SESSION_BREAKPOINT + 1.0));
    }
}
