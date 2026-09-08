//! 表树顶部工具栏的窄窗口布局回归测试。

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{AppContext as _, TestAppContext, px, size};
use ramag_app::{ConnectionService, MongoService, RedisService};
use ramag_domain::entities::{ConnectionConfig, ConnectionId, QueryRecord, QueryRecordId, Schema};
use ramag_domain::error::Result;
use ramag_domain::traits::Storage;

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
    parent: gpui::Bounds<gpui::Pixels>,
    child: gpui::Bounds<gpui::Pixels>,
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
    left: gpui::Bounds<gpui::Pixels>,
    right_name: &str,
    right: gpui::Bounds<gpui::Pixels>,
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

/// 表树实际位于 SQL 会话的可调整侧栏中，因此额外覆盖其 180px 最小宽度。
#[gpui::test]
fn table_tree_toolbar_wraps_inside_sidebar_widths(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
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
        gpui_component::Root::new(panel, window, cx)
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
