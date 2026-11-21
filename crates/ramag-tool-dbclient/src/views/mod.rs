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

/// SQLite 把文件路径存放在 `host`，只有网络连接才显示端口。
fn connection_address(connection: &ramag_domain::entities::ConnectionConfig) -> String {
    if connection.driver == ramag_domain::entities::DriverKind::Sqlite {
        connection.host.clone()
    } else {
        format!("{}:{}", connection.host, connection.port)
    }
}

/// 侧栏与对象树同时在 760px 及以下窗口退出横向布局。
pub(super) fn is_compact_session_width(width: f32) -> bool {
    !width.is_finite() || width <= ramag_ui::shell::SHELL_NARROW_BREAKPOINT
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
    use super::{connection_address, inline_text_preview};
    use ramag_domain::entities::ConnectionConfig;

    #[test]
    fn inline_preview_is_single_line_and_unicode_safe() {
        assert_eq!(inline_text_preview("你好世界", 2), "你好…");
        assert_eq!(inline_text_preview("a\nb\0c", 20), "a b�c");
    }

    #[test]
    fn connection_address_distinguishes_sqlite_files_and_network_endpoints() {
        let path = r"C:\Test Data\preview.sqlite";
        let sqlite = ConnectionConfig::new_sqlite("local", path);
        let mysql = ConnectionConfig::new_mysql("remote", "mysql.local", 3306, "user");
        assert_eq!(connection_address(&sqlite), path);
        assert_eq!(connection_address(&mysql), "mysql.local:3306");
    }
}

#[cfg(test)]
mod session_layout_tests {
    use super::is_compact_session_width;

    #[test]
    fn compact_session_breakpoint_preserves_a_usable_query_area() {
        assert!(is_compact_session_width(759.0));
        assert!(is_compact_session_width(760.0));
        assert!(!is_compact_session_width(761.0));
    }
}
