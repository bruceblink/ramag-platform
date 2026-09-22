//! 表树顶部工具栏的窄窗口布局回归测试。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use gpui_kit::{AppContext as _, Modifiers, TestAppContext, px, size};
use ramag_app::{ConnectionService, MongoService, RedisService};
use ramag_domain::entities::{
    Column, ConnectionConfig, ConnectionId, DriverKind, ForeignKey, Index, Query, QueryRecord,
    QueryRecordId, QueryResult, Schema, Table, Trigger,
};
use ramag_domain::error::Result;
use ramag_domain::traits::{Driver, Storage};

use super::TableTreePanel;
use crate::sql_completion::SchemaCache;
use crate::views::connection_list::ConnectionListPanel;

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

struct TableSizeDriver {
    size_bytes: Arc<AtomicU64>,
}

#[async_trait]
impl Driver for TableSizeDriver {
    fn name(&self) -> &'static str {
        "table-size-test"
    }

    async fn test_connection(&self, _config: &ConnectionConfig) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _config: &ConnectionConfig, _query: &Query) -> Result<QueryResult> {
        self.size_bytes.store(0, Ordering::SeqCst);
        Ok(QueryResult {
            columns: Vec::new(),
            column_types: Vec::new(),
            rows: Vec::new(),
            affected_rows: 0,
            elapsed_ms: 0,
            warnings: Vec::new(),
            truncated: false,
        })
    }

    async fn list_schemas(&self, _config: &ConnectionConfig) -> Result<Vec<Schema>> {
        Ok(vec![Schema {
            name: "ship-db".into(),
            charset: None,
            collation: None,
        }])
    }

    async fn list_tables(&self, _config: &ConnectionConfig, schema: &str) -> Result<Vec<Table>> {
        Ok(vec![Table {
            name: "collision_other_ais_msg".into(),
            schema: schema.into(),
            comment: None,
            is_view: false,
            size_bytes: Some(self.size_bytes.load(Ordering::SeqCst)),
        }])
    }

    async fn list_columns(
        &self,
        _config: &ConnectionConfig,
        _schema: &str,
        _table: &str,
    ) -> Result<Vec<Column>> {
        Ok(Vec::new())
    }

    async fn list_indexes(
        &self,
        _config: &ConnectionConfig,
        _schema: &str,
        _table: &str,
    ) -> Result<Vec<Index>> {
        Ok(Vec::new())
    }

    async fn list_foreign_keys(
        &self,
        _config: &ConnectionConfig,
        _schema: &str,
        _table: &str,
    ) -> Result<Vec<ForeignKey>> {
        Ok(Vec::new())
    }

    async fn list_triggers(
        &self,
        _config: &ConnectionConfig,
        _schema: &str,
        _table: &str,
    ) -> Result<Vec<Trigger>> {
        Ok(Vec::new())
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

fn assert_inside(
    parent: gpui_kit::Bounds<gpui_kit::Pixels>,
    child: gpui_kit::Bounds<gpui_kit::Pixels>,
    label: &str,
) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label} 越出父容器：parent={parent:?}, child={child:?}"
    );
}

fn assert_non_overlapping(
    left_name: &str,
    left: gpui_kit::Bounds<gpui_kit::Pixels>,
    right_name: &str,
    right: gpui_kit::Bounds<gpui_kit::Pixels>,
) {
    let separated = left.right() <= right.origin.x
        || right.right() <= left.origin.x
        || left.bottom() <= right.origin.y
        || right.bottom() <= left.origin.y;
    assert!(
        separated,
        "表树工具栏控件不能重叠：{left_name} / {right_name}; left={left:?}, right={right:?}"
    );
}

#[gpui_kit::test]
fn truncating_table_refreshes_size_without_dropping_selection(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let size_bytes = Arc::new(AtomicU64::new(76 * 1024 * 1024 * 1024));
    let driver = TableSizeDriver {
        size_bytes: size_bytes.clone(),
    };
    let mut drivers = HashMap::new();
    drivers.insert(DriverKind::Mysql, Arc::new(driver) as Arc<dyn Driver>);
    let storage: Arc<dyn Storage> = Arc::new(NoopStorage);
    let service = Arc::new(ConnectionService::new(drivers, storage));
    let (list_service, redis_service, mongo_service) = build_services();
    let mut panel_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let connection_list = cx.new(|cx| {
            ConnectionListPanel::new(list_service, redis_service, mongo_service, window, cx)
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
                name: "ship-db".into(),
                charset: None,
                collation: None,
            }];
            panel.open_schemas.insert("ship-db".into());
            panel.expanded.insert(
                "ship-db".into(),
                super::SchemaTables {
                    tables: vec![Table {
                        name: "collision_other_ais_msg".into(),
                        schema: "ship-db".into(),
                        comment: None,
                        is_view: false,
                        size_bytes: Some(size_bytes.load(Ordering::SeqCst)),
                    }],
                    ..Default::default()
                },
            );
            panel.selected = Some(("ship-db".into(), "collision_other_ais_msg".into()));
            panel.invalidate_tree_rows();
        });
    });

    cx.update(|_, app| {
        panel.update(app, |panel, cx| {
            panel.truncate_table("ship-db".into(), "collision_other_ais_msg".into(), cx);
        });
    });
    cx.run_until_parked();

    panel.read_with(cx, |panel, _| {
        let table = &panel
            .expanded
            .get("ship-db")
            .expect("schema should remain loaded")
            .tables[0];
        assert_eq!(table.size_bytes, Some(0));
        assert_eq!(
            panel
                .selected
                .as_ref()
                .map(|(schema, table)| (schema.as_str(), table.as_str())),
            Some(("ship-db", "collision_other_ais_msg"))
        );
    });
}

/// 表树实际位于 SQL 会话的可调整侧栏中，因此额外覆盖其 180px 最小宽度。
#[gpui_kit::test]
fn table_tree_toolbar_wraps_inside_sidebar_widths(cx: &mut TestAppContext) {
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
            panel.loading_schemas = false;
            panel.schemas = vec![Schema {
                name: "application_database".to_string(),
                charset: None,
                collation: None,
            }];
            panel.invalidate_tree_rows();
        });
    });
    cx.run_until_parked();

    let controls = [
        ("搜索", "table-tree-search"),
        ("筛选", "table-tree-filter"),
        ("系统库", "toggle-system"),
        ("刷新", "refresh-schemas"),
        ("编辑器", "toggle-query-panel"),
    ];

    for width in [180.0, 280.0, 360.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(480.0)));
        panel.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();

        let header = cx
            .debug_bounds("table-tree-header")
            .expect("表树工具栏应渲染");
        assert!(header.size.width > px(0.0), "表树工具栏不能为零宽");
        assert!(header.right() <= px(width), "表树工具栏不能越出窗口");
        assert!(header.bottom() <= px(480.0), "表树工具栏不能越出窗口底部");

        let bounds = controls
            .iter()
            .map(|(label, selector)| {
                let bounds = cx
                    .debug_bounds(selector)
                    .unwrap_or_else(|| panic!("表树工具栏控件应渲染：{selector}"));
                assert_inside(header, bounds, label);
                assert!(bounds.size.width > px(0.0), "{label}不能被压缩为零宽");
                (*label, bounds)
            })
            .collect::<Vec<_>>();
        for (index, (left_name, left)) in bounds.iter().enumerate() {
            for (right_name, right) in bounds.iter().skip(index + 1) {
                assert_non_overlapping(left_name, *left, right_name, *right);
            }
        }
    }
}

/// Refreshing an active connection keeps the existing tree usable while metadata is reloaded.
#[gpui_kit::test]
fn table_tree_refresh_keeps_existing_rows_during_reload_and_failure(cx: &mut TestAppContext) {
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
                name: "ramag_test".to_string(),
                charset: None,
                collation: None,
            }];
            panel.open_schemas.insert("ramag_test".to_string());
            panel.expanded.insert(
                "ramag_test".to_string(),
                super::SchemaTables {
                    tables: vec![Table {
                        name: "bulk_records".to_string(),
                        schema: "ramag_test".to_string(),
                        comment: None,
                        is_view: false,
                        size_bytes: Some(1024),
                    }],
                    ..Default::default()
                },
            );
            panel.selected = Some(("ramag_test".to_string(), "bulk_records".to_string()));
            panel.loading_schemas = true;
            panel.invalidate_tree_rows();
        });
    });
    panel.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    assert!(cx.debug_bounds("table-tree-header").is_some());
    let has_table_row = cx.update(|_, app| {
        panel.read(app).tree_rows_view("").rows.iter().any(|row| {
            matches!(row, super::row::TreeRow::Table { key, .. } if key.0 == "ramag_test" && key.1 == "bulk_records")
        })
    });
    assert!(has_table_row);
    assert!(cx.debug_bounds("table-tree-status").is_some());

    cx.update(|_, app| {
        panel.update(app, |panel, cx| {
            panel.loading_schemas = false;
            panel.error = Some("连接暂时不可用".to_string());
            cx.notify();
        });
    });
    cx.run_until_parked();

    let has_table_row = cx.update(|_, app| {
        panel.read(app).tree_rows_view("").rows.iter().any(|row| {
            matches!(row, super::row::TreeRow::Table { key, .. } if key.0 == "ramag_test" && key.1 == "bulk_records")
        })
    });
    assert!(has_table_row);
    assert!(cx.debug_bounds("retry-schemas").is_some());
    assert!(cx.debug_bounds("table-tree-status").is_some());
}

#[gpui_kit::test]
fn table_group_header_click_collapses_only_its_group(cx: &mut TestAppContext) {
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
                service.clone(),
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
                super::SchemaTables {
                    tables: vec![
                        Table {
                            name: "users".into(),
                            schema: "public".into(),
                            comment: None,
                            is_view: false,
                            size_bytes: None,
                        },
                        Table {
                            name: "audit_log".into(),
                            schema: "public".into(),
                            comment: None,
                            is_view: true,
                            size_bytes: None,
                        },
                    ],
                    ..Default::default()
                },
            );
            panel.invalidate_tree_rows();
        });
    });
    panel.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    let tables_header = cx
        .debug_bounds("table-group-public-tables")
        .expect("表分组标题应渲染");
    assert!(cx.update(|_, app| {
        panel
            .read(app)
            .tree_rows_view("")
            .rows
            .iter()
            .any(|row| matches!(row, super::row::TreeRow::Table { key, is_view: false, .. } if key.1 == "users"))
    }));

    cx.simulate_click(tables_header.center(), Modifiers::default());
    cx.run_until_parked();

    let rows = cx.update(|_, app| panel.read(app).tree_rows_view("").rows.clone());
    assert!(!rows.iter().any(|row| {
        matches!(row, super::row::TreeRow::Table { key, is_view: false, .. } if key.1 == "users")
    }));
    assert!(rows.iter().any(|row| {
        matches!(row, super::row::TreeRow::Table { key, is_view: true, .. } if key.1 == "audit_log")
    }));
}
