use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use gpui_kit::{AppContext as _, TestAppContext, px, size};
use ramag_app::{ConnectionService, MongoService, RedisService};
use ramag_domain::entities::{
    Column, ColumnKind, ColumnType, ConnectionConfig, ConnectionId, QueryRecord, QueryRecordId,
    Schema, Table, Trigger,
};
use ramag_domain::error::Result;
use ramag_domain::traits::Storage;

use crate::sql_completion::SchemaCache;
use crate::views::connection_list::ConnectionListPanel;
use crate::views::table_tree::{SchemaTables, TableColumns, TableTreePanel};

#[derive(Default)]
struct NoopStorage;

#[async_trait]
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

fn build_services() -> (Arc<ConnectionService>, Arc<RedisService>, Arc<MongoService>) {
    let storage: Arc<dyn Storage> = Arc::new(NoopStorage);
    (
        Arc::new(ConnectionService::new(HashMap::new(), storage.clone())),
        Arc::new(RedisService::new(
            Arc::new(ramag_infra_redis::RedisDriver::new()),
            storage.clone(),
        )),
        Arc::new(MongoService::new(
            Arc::new(ramag_infra_mongodb::MongoDriver::new()),
            storage,
        )),
    )
}

/// 真实渲染表树的一列和一个触发器，证明字段名选区与触发器列表共用用户可见的表节点。
#[gpui_kit::test]
fn column_name_is_selectable_and_trigger_stays_in_table_tree(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (service, redis_service, mongo_service) = build_services();
    let mut panel_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let connection_list = cx.new(|cx| {
            ConnectionListPanel::new(
                service.clone(),
                redis_service.clone(),
                mongo_service.clone(),
                window,
                cx,
            )
        });
        let panel = cx.new(|cx| {
            TableTreePanel::new(
                service,
                SchemaCache::new_shared(),
                connection_list,
                window,
                cx,
            )
        });
        panel_entity = Some(panel.clone());
        gpui_kit::component::Root::new(panel, window, cx)
    });
    let panel = panel_entity.expect("表树面板应创建");

    cx.update(|_, app| {
        panel.update(app, |panel, _| {
            panel.connection = Some(ConnectionConfig::new_mysql(
                "测试连接",
                "127.0.0.1",
                3306,
                "root",
            ));
            panel.schemas = vec![Schema {
                name: "public".into(),
                charset: None,
                collation: None,
            }];
            panel.open_schemas.insert("public".into());
            panel.expanded.insert(
                "public".into(),
                SchemaTables {
                    tables: vec![Table {
                        name: "users".into(),
                        schema: "public".into(),
                        comment: None,
                        is_view: false,
                        size_bytes: None,
                    }],
                    ..Default::default()
                },
            );
            panel.table_columns.insert(
                ("public".into(), "users".into()),
                TableColumns {
                    columns: vec![Column {
                        name: "account_identifier".into(),
                        data_type: ColumnType {
                            kind: ColumnKind::Text,
                            raw_type: "varchar(64)".into(),
                        },
                        nullable: false,
                        default_value: None,
                        is_primary_key: false,
                        comment: None,
                        ordinal_position: Some(1),
                        is_auto_increment: false,
                        generation_expression: None,
                        generated_storage: None,
                        identity_generation: None,
                    }],
                    triggers: vec![Trigger {
                        name: "users_audit".into(),
                        timing: "AFTER".into(),
                        event: "INSERT".into(),
                        definition: "BEGIN END".into(),
                    }],
                    ..Default::default()
                },
            );
            panel.invalidate_tree_rows();
        });
    });
    panel.update(cx, |_, cx| cx.notify());
    cx.simulate_resize(size(px(1024.0), px(600.0)));
    cx.run_until_parked();

    let name_bounds = cx
        .debug_bounds("tree-column-copy-public-users-0-field-name-frame")
        .expect("字段名应渲染为可选中文本");
    let trigger_bounds = cx
        .debug_bounds("tree-trigger-action-public-users-0-users_audit")
        .expect("触发器应显示在表树中");
    assert!(name_bounds.size.width > px(0.0));
    assert!(trigger_bounds.size.width > px(0.0));
}
