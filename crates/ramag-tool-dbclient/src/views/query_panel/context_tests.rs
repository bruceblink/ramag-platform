use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AppContext as _, Context, Entity, IntoElement, Modifiers, ParentElement as _, Render,
    Styled as _, TestAppContext, VisualTestContext, Window, div, point,
};
use ramag_app::ConnectionService;
use ramag_domain::entities::{ConnectionConfig, ConnectionId, QueryRecord};
use ramag_domain::error::Result;
use ramag_domain::traits::Storage;

use super::QueryPanel;
use crate::sql_completion::SchemaCache;

#[derive(Default)]
struct NoopStorage {
    history: Vec<QueryRecord>,
}

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
        connection_id: Option<&ConnectionId>,
        limit: usize,
    ) -> Result<Vec<QueryRecord>> {
        Ok(self
            .history
            .iter()
            .filter(|record| {
                connection_id
                    .map(|connection_id| record.connection_id == *connection_id)
                    .unwrap_or(true)
            })
            .take(limit)
            .cloned()
            .collect())
    }

    async fn delete_history(&self, _id: &ramag_domain::entities::QueryRecordId) -> Result<()> {
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

/// Test host that renders the query panel together with the component dialog layer.
struct ContextTestHost {
    panel: Entity<QueryPanel>,
}

impl Render for ContextTestHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = gpui_component::Root::render_dialog_layer(window, cx);
        div()
            .relative()
            .size_full()
            .child(self.panel.clone())
            .children(dialog_layer)
    }
}

/// Clicks a confirmation button after the dialog animation has settled.
fn click_dialog_button(cx: &mut VisualTestContext, selector: &'static str) {
    assert!(
        cx.debug_bounds(selector).is_some(),
        "确认对话框按钮应渲染: {selector}"
    );
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("确认对话框按钮应渲染: {selector}"));
    let center = point(
        bounds.origin.x + bounds.size.width / 2.0,
        bounds.origin.y + bounds.size.height / 2.0,
    );
    cx.simulate_mouse_move(center, None, Modifiers::default());
    let bounds = cx.debug_bounds(selector).unwrap_or(bounds);
    let center = point(
        bounds.origin.x + bounds.size.width / 2.0,
        bounds.origin.y + bounds.size.height / 2.0,
    );
    cx.simulate_mouse_down(center, gpui::MouseButton::Left, Modifiers::default());
    let release_bounds = cx.debug_bounds(selector).unwrap_or(bounds);
    let release_center = point(
        release_bounds.origin.x + release_bounds.size.width / 2.0,
        release_bounds.origin.y + release_bounds.size.height / 2.0,
    );
    cx.simulate_mouse_up(
        release_center,
        gpui::MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
}

/// Seeds a local result edit on one tab without starting a display-view task.
fn stage_pending_result_change(
    cx: &mut VisualTestContext,
    panel: &Entity<QueryPanel>,
    tab_index: usize,
) {
    cx.update(|_, app| {
        panel.update(app, |panel, cx| {
            let Some(tab) = panel.tabs.get(tab_index).cloned() else {
                panic!("查询面板缺少测试标签 {tab_index}");
            };
            tab.update(cx, |tab, cx| {
                tab.result.update(cx, |result, _cx| {
                    result.set_pinned_target(Some((Some("public".into()), "users".into())));
                    result.seed_pending_cell_edit_for_test();
                });
            });
        });
    });
    cx.run_until_parked();
}

/// Counts staged result changes across every query tab.
fn pending_result_change_total(cx: &mut VisualTestContext, panel: &Entity<QueryPanel>) -> usize {
    cx.update(|_, app| {
        panel
            .read(app)
            .tabs
            .iter()
            .map(|tab| tab.read(app).pending_result_change_count(app))
            .sum()
    })
}

/// Context changes must keep staged result edits until the user explicitly confirms loss.
#[gpui::test]
fn context_switches_confirm_before_discarding_pending_result_changes(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let service = Arc::new(ConnectionService::new(
        HashMap::new(),
        Arc::new(NoopStorage::default()),
    ));
    let connection = ConnectionConfig::new_mysql("上下文测试连接", "127.0.0.1", 3306, "root");
    let schema_cache = SchemaCache::new_shared();
    let mut panel_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let panel = cx.new(|cx| {
            QueryPanel::new(
                service.clone(),
                schema_cache,
                ramag_ui::ResultMemoryBudget::default(),
                window,
                cx,
            )
        });
        panel_entity = Some(panel.clone());
        let host = cx.new(|_| ContextTestHost {
            panel: panel.clone(),
        });
        gpui_component::Root::new(host, window, cx)
    });
    let panel = panel_entity.expect("SQL 查询面板应创建");

    visual_cx.update(|window, app| {
        panel.update(app, |panel, cx| {
            panel.set_connection(Some(connection.clone()), window, cx);
            assert!(panel.add_tab(window, cx));
        });
    });
    visual_cx.run_until_parked();
    stage_pending_result_change(visual_cx, &panel, 1);

    let applied = visual_cx.update(|window, app| {
        panel.update(app, |panel, cx| {
            panel.set_active_schema(Some("analytics".into()), window, cx)
        })
    });
    assert!(!applied, "存在未提交结果修改时不能直接切换 Schema");
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("ramag-confirm-ok").is_some());
    click_dialog_button(visual_cx, "ramag-confirm-cancel");

    let schema = visual_cx.update(|_, app| panel.read(app).active_schema.clone());
    let pending = pending_result_change_total(visual_cx, &panel);
    assert!(schema.is_none(), "取消确认后 Schema 不应改变");
    assert_eq!(pending, 1, "取消确认后未提交修改必须保留");

    let applied = visual_cx.update(|window, app| {
        panel.update(app, |panel, cx| {
            panel.set_active_schema(Some("analytics".into()), window, cx)
        })
    });
    assert!(!applied);
    visual_cx.run_until_parked();
    click_dialog_button(visual_cx, "ramag-confirm-ok");

    let schema = visual_cx.update(|_, app| panel.read(app).active_schema.clone());
    let pending = pending_result_change_total(visual_cx, &panel);
    assert_eq!(schema.as_deref(), Some("analytics"));
    assert_eq!(pending, 0, "确认后才允许清理未提交修改");

    stage_pending_result_change(visual_cx, &panel, 1);
    let another_connection = ConnectionConfig::new_mysql("另一个连接", "127.0.0.1", 3307, "root");
    visual_cx.update(|window, app| {
        panel.update(app, |panel, cx| {
            panel.set_connection(Some(another_connection.clone()), window, cx);
        });
    });
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("ramag-confirm-ok").is_some());
    click_dialog_button(visual_cx, "ramag-confirm-cancel");

    let connection_id = visual_cx.update(|_, app| {
        panel
            .read(app)
            .connection
            .as_ref()
            .map(|connection| connection.id.clone())
    });
    let pending = pending_result_change_total(visual_cx, &panel);
    assert_eq!(
        connection_id.as_ref(),
        Some(&connection.id),
        "取消确认后连接不应改变"
    );
    assert_eq!(pending, 1, "取消连接切换后未提交修改必须保留");

    visual_cx.update(|window, app| {
        panel.update(app, |panel, cx| {
            panel.set_connection(Some(another_connection), window, cx);
        });
    });
    visual_cx.run_until_parked();
    click_dialog_button(visual_cx, "ramag-confirm-ok");
    let pending = pending_result_change_total(visual_cx, &panel);
    assert_eq!(pending, 0, "确认连接切换后才允许清理未提交修改");
}

/// Closing a query tab must confirm before releasing staged result state.
#[gpui::test]
fn closing_tab_confirms_before_releasing_pending_result_changes(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let service = Arc::new(ConnectionService::new(
        HashMap::new(),
        Arc::new(NoopStorage::default()),
    ));
    let connection = ConnectionConfig::new_mysql("关闭标签测试连接", "127.0.0.1", 3306, "root");
    let schema_cache = SchemaCache::new_shared();
    let mut panel_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let panel = cx.new(|cx| {
            QueryPanel::new(
                service.clone(),
                schema_cache,
                ramag_ui::ResultMemoryBudget::default(),
                window,
                cx,
            )
        });
        panel_entity = Some(panel.clone());
        let host = cx.new(|_| ContextTestHost {
            panel: panel.clone(),
        });
        gpui_component::Root::new(host, window, cx)
    });
    let panel = panel_entity.expect("SQL 查询面板应创建");

    visual_cx.update(|window, app| {
        panel.update(app, |panel, cx| {
            panel.set_connection(Some(connection.clone()), window, cx);
            assert!(panel.add_tab(window, cx));
        });
    });
    visual_cx.run_until_parked();
    stage_pending_result_change(visual_cx, &panel, 0);

    visual_cx.update(|window, app| {
        panel.update(app, |panel, cx| panel.close_tab(0, window, cx));
    });
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("ramag-confirm-ok").is_some());
    click_dialog_button(visual_cx, "ramag-confirm-cancel");

    let (tab_count, pending) = visual_cx.update(|_, app| {
        let panel_state = panel.read(app);
        (
            panel_state.tabs.len(),
            panel_state
                .tabs
                .first()
                .map_or(0, |tab| tab.read(app).pending_result_change_count(app)),
        )
    });
    assert_eq!(tab_count, 2, "取消关闭后标签必须保留");
    assert_eq!(pending, 1, "取消关闭后未提交修改必须保留");

    visual_cx.update(|window, app| {
        panel.update(app, |panel, cx| panel.close_tab(0, window, cx));
    });
    visual_cx.run_until_parked();
    click_dialog_button(visual_cx, "ramag-confirm-ok");

    let tab_count = visual_cx.update(|_, app| panel.read(app).tabs.len());
    assert_eq!(tab_count, 1, "确认关闭后才释放查询标签");
}
