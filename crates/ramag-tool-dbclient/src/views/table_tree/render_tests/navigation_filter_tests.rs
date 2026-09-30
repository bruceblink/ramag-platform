use gpui_kit::TestAppContext;

use super::*;
use crate::views::table_tree::navigation::TableTreeFilter;

/// 记录最近访问表后，当前对象树筛选必须立即使用新状态重建行视图。
#[gpui_kit::test]
fn recent_filter_refreshes_after_recording_table(cx: &mut TestAppContext) {
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
                name: "ramag_test".into(),
                charset: None,
                collation: None,
            }];
            panel.open_schemas.insert("ramag_test".into());
            panel.expanded.insert(
                "ramag_test".into(),
                super::super::SchemaTables {
                    tables: vec![Table {
                        name: "recent_table".into(),
                        schema: "ramag_test".into(),
                        comment: None,
                        is_view: false,
                        size_bytes: None,
                    }],
                    ..Default::default()
                },
            );
            panel.table_filter = TableTreeFilter::Recent;
            panel.invalidate_tree_rows();

            let rows = panel.tree_rows_view("").rows;
            assert!(!rows.iter().any(|row| {
                matches!(
                    row,
                    super::super::row::TreeRow::Table { key, .. }
                        if key.1 == "recent_table"
                )
            }));
        });
    });

    panel.update(cx, |panel, cx| {
        let revision_before = panel.tree_revision;
        panel.record_recent_table("ramag_test".into(), "recent_table".into(), cx);

        assert!(panel.tree_revision > revision_before);
        assert!(panel.tree_rows_view("").rows.iter().any(|row| {
            matches!(
                row,
                super::super::row::TreeRow::Table { key, .. }
                    if key.0 == "ramag_test" && key.1 == "recent_table"
            )
        }));
    });
}
