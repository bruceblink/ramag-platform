use std::collections::HashMap;
use std::sync::Arc;

use gpui::{AppContext as _, TestAppContext, px, size};
use ramag_app::ConnectionService;
use ramag_domain::entities::{ConnectionConfig, ConnectionId, QueryRecord, QueryRecordId};
use ramag_domain::error::Result;
use ramag_domain::traits::Storage;

use super::SchemaDiffDialog;

#[derive(Default)]
struct NoopStorage;

#[async_trait::async_trait]
impl Storage for NoopStorage {
    async fn list_connections(&self) -> Result<Vec<ConnectionConfig>> {
        Ok(Vec::new())
    }

    async fn get_connection(&self, _id: &ConnectionId) -> Result<Option<ConnectionConfig>> {
        Ok(None)
    }

    async fn save_connection(&self, _config: &ConnectionConfig) -> Result<()> {
        Ok(())
    }

    async fn delete_connection(&self, _id: &ConnectionId) -> Result<()> {
        Ok(())
    }

    async fn append_history(&self, _record: &QueryRecord) -> Result<()> {
        Ok(())
    }

    async fn list_history(
        &self,
        _connection_id: Option<&ConnectionId>,
        _limit: usize,
    ) -> Result<Vec<QueryRecord>> {
        Ok(Vec::new())
    }

    async fn delete_history(&self, _id: &QueryRecordId) -> Result<()> {
        Ok(())
    }

    async fn clear_history(&self, _connection_id: Option<&ConnectionId>) -> Result<()> {
        Ok(())
    }

    async fn get_preference(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn set_preference(&self, _key: &str, _value: &str) -> Result<()> {
        Ok(())
    }
}

fn test_dialog(cx: &mut TestAppContext) -> &mut gpui::VisualTestContext {
    let service = Arc::new(ConnectionService::new(
        HashMap::new(),
        Arc::new(NoopStorage),
    ));
    let source_connection =
        ConnectionConfig::new_mysql("源连接-这是一个很长的连接名称", "127.0.0.1", 3306, "root");
    let target_connection = ConnectionConfig::new_mysql(
        "目标连接-这是另一个很长的连接名称",
        "127.0.0.2",
        3306,
        "root",
    );

    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let dialog = cx.new(|_| SchemaDiffDialog {
            service,
            source_connection,
            target_connection,
            source_schema: "source_schema_with_a_long_name".into(),
            source_table: "source_table".into(),
            target_schema: "target_schema_with_a_long_name".into(),
            target_table: "target_table".into(),
            source: None,
            target: None,
            loading: false,
            request_generation: 0,
            error: None,
            vertical_scroll: gpui::ScrollHandle::new(),
            horizontal_scroll: gpui::ScrollHandle::new(),
            migration_vertical_scroll: gpui::ScrollHandle::new(),
            migration_horizontal_scroll: gpui::ScrollHandle::new(),
            migration_visible: false,
            saving_migration: false,
            executing_migration: false,
            migration_execution_generation: 0,
            migration_approvals: Vec::new(),
            pending_notification: None,
        });
        gpui_component::Root::new(dialog, window, cx)
    });
    visual_cx
}

fn assert_inside(child: gpui::Bounds<gpui::Pixels>, parent: gpui::Bounds<gpui::Pixels>) {
    assert!(child.origin.x >= parent.origin.x);
    assert!(child.origin.y >= parent.origin.y);
    assert!(child.right() <= parent.right());
    assert!(child.bottom() <= parent.bottom());
}

#[gpui::test]
fn schema_diff_toolbar_keeps_context_and_actions_inside_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let cx = test_dialog(cx);

    for width in [360.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(320.0)));
        cx.run_until_parked();

        let toolbar = cx
            .debug_bounds("schema-diff-toolbar")
            .expect("结构对比工具栏应渲染");
        let title = cx
            .debug_bounds("schema-diff-title")
            .expect("结构对比标题应渲染");
        let context = cx
            .debug_bounds("schema-diff-context")
            .expect("结构对比连接上下文应渲染");

        assert_inside(title, toolbar);
        assert_inside(context, toolbar);
        for selector in [
            "schema-diff-migration",
            "schema-diff-copy",
            "schema-diff-refresh",
        ] {
            let action = cx.debug_bounds(selector).expect("结构对比操作按钮应渲染");
            assert_inside(action, toolbar);
        }
    }
}
