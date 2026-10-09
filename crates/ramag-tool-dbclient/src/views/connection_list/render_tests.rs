use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled as _,
    TestAppContext, Window, div, px, size,
};
use ramag_app::{ConnectionService, MongoService, RedisService};
use ramag_domain::entities::{ConnectionConfig, ConnectionId, QueryRecord};
use ramag_domain::error::Result;
use ramag_domain::traits::Storage;

use super::ConnectionListPanel;

struct ConnectionListTestHost {
    panel: Entity<ConnectionListPanel>,
}

/// Exercise the actual virtual rows and fixed headings, including empty optional metadata.
#[gpui_kit::test]
fn connection_rows_and_headings_share_bounds_and_uniform_height(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let storage: Arc<dyn Storage> = Arc::new(NoopStorage);
    let service = Arc::new(ConnectionService::new(HashMap::new(), storage.clone()));
    let redis = Arc::new(RedisService::new(
        Arc::new(ramag_infra_redis::RedisDriver::new()),
        storage.clone(),
    ));
    let mongo = Arc::new(MongoService::new(
        Arc::new(ramag_infra_mongodb::MongoDriver::new()),
        storage,
    ));
    let mut connections = Vec::new();
    for driver in [
        ramag_domain::entities::DriverKind::Mysql,
        ramag_domain::entities::DriverKind::Postgres,
        ramag_domain::entities::DriverKind::Sqlite,
        ramag_domain::entities::DriverKind::Redis,
        ramag_domain::entities::DriverKind::Mongodb,
        ramag_domain::entities::DriverKind::Mysql,
    ] {
        let mut connection = ConnectionConfig::new_mysql(
            "很长的数据源名称用于验证操作仍可达和完整名称提示",
            "127.0.0.1",
            13306,
            "ramag",
        );
        connection.driver = driver;
        if connections.is_empty() {
            connection.environment = Some("很长的自定义环境名称".into());
            connection.production = true;
        }
        connections.push(connection);
    }
    let keys: Vec<_> = connections.iter().map(|c| c.id.to_string()).collect();
    let mut panel_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let panel = cx.new(|panel_cx| {
            let mut panel = ConnectionListPanel::new(service, redis, mongo, window, panel_cx);
            panel.refresh_generation = panel.refresh_generation.wrapping_add(1);
            panel.loading = false;
            panel.connections = Arc::new(connections);
            panel
                .versions
                .insert(panel.connections[0].id.clone(), "8.4.0".into());
            panel.loaded_revision = panel.service.revision();
            panel
        });
        panel_entity = Some(panel.clone());
        ConnectionListTestHost { panel }
    });
    let panel = panel_entity.unwrap();
    for mode in [ramag_ui::Mode::Dark, ramag_ui::Mode::Light] {
        for text_size in [
            ramag_ui::InterfaceTextSize::Standard,
            ramag_ui::InterfaceTextSize::Large,
        ] {
            cx.update(|_, app| {
                ramag_ui::set_system_settings(
                    ramag_ui::SystemSettings {
                        text_size,
                        ..Default::default()
                    },
                    app,
                );
                ramag_ui::apply_theme(mode, app);
            });
            for width in [360.0, 899.0, 900.0, 1024.0, 1119.0, 1120.0, 1440.0] {
                cx.simulate_resize(size(px(width), px(900.0)));
                panel.update(cx, |_, cx| cx.notify());
                cx.run_until_parked();
                let surface = cx.debug_bounds("connection-list-panel").unwrap();
                let toolbar = cx.debug_bounds("connection-list-toolbar").unwrap();
                let heading = cx.debug_bounds("connection-list-column-header").unwrap();
                assert_eq!(toolbar.origin.x, heading.origin.x);
                assert_eq!(toolbar.size.width, heading.size.width);
                assert!(heading.bottom() < surface.bottom());
                let mut row_height = None;
                for key in &keys {
                    let row_selector: &'static str =
                        Box::leak(format!("connection-row-{key}").into_boxed_str());
                    let name_selector: &'static str =
                        Box::leak(format!("connection-row-name-{key}").into_boxed_str());
                    let actions_selector: &'static str =
                        Box::leak(format!("connection-row-actions-{key}").into_boxed_str());
                    let row = cx.debug_bounds(row_selector).unwrap();
                    let name = cx.debug_bounds(name_selector).unwrap();
                    let actions = cx.debug_bounds(actions_selector).unwrap();
                    assert!(
                        name.size.width >= px(80.0),
                        "name collapsed at {width}: {name:?}"
                    );
                    assert!(name.origin.x >= row.origin.x && name.right() <= actions.origin.x);
                    assert!(actions.right() <= row.right() && actions.bottom() <= row.bottom());
                    assert_eq!(row.size.height, *row_height.get_or_insert(row.size.height));
                    assert_eq!(
                        actions.origin.x,
                        cx.debug_bounds("connection-header-actions")
                            .unwrap()
                            .origin
                            .x
                    );
                    if width >= 900.0 {
                        for column in ["kind", "address"] {
                            let cell_selector: &'static str = Box::leak(
                                format!("connection-row-{column}-{key}").into_boxed_str(),
                            );
                            let header_selector: &'static str =
                                Box::leak(format!("connection-header-{column}").into_boxed_str());
                            let cell = cx.debug_bounds(cell_selector).unwrap();
                            let header = cx.debug_bounds(header_selector).unwrap();
                            assert_eq!(cell.origin.x, header.origin.x);
                            assert_eq!(cell.size.width, header.size.width);
                        }
                    }
                }
            }
        }
    }
}

impl Render for ConnectionListTestHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.panel.clone())
    }
}

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

#[gpui_kit::test]
fn connection_list_pulse_header_and_panel_fit_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let storage: Arc<dyn Storage> = Arc::new(NoopStorage);
    let service = Arc::new(ConnectionService::new(HashMap::new(), storage.clone()));
    let redis_service = Arc::new(RedisService::new(
        Arc::new(ramag_infra_redis::RedisDriver::new()),
        storage.clone(),
    ));
    let mongo_service = Arc::new(MongoService::new(
        Arc::new(ramag_infra_mongodb::MongoDriver::new()),
        storage,
    ));
    let mut panel_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let entity = cx.new(|panel_cx| {
            let mut panel =
                ConnectionListPanel::new(service, redis_service, mongo_service, window, panel_cx);
            // Invalidate the constructor's empty async refresh and use deterministic test data.
            panel.refresh_generation = panel.refresh_generation.wrapping_add(1);
            panel.loading = false;
            panel.connections = Arc::new(vec![ConnectionConfig::new_mysql(
                "一个很长的数据库连接名称用于标题和列表边界验证",
                "db.internal.example",
                3306,
                "ramag",
            )]);
            panel.loaded_revision = panel.service.revision();
            panel
        });
        panel_entity = Some(entity.clone());
        ConnectionListTestHost { panel: entity }
    });
    let panel = panel_entity.expect("连接列表面板应创建");

    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        panel.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();

        let title = cx
            .debug_bounds("connection-list-page-title")
            .expect("数据库客户端页面标题应渲染");
        let toolbar = cx
            .debug_bounds("connection-list-toolbar")
            .expect("连接列表工具栏应渲染");
        let list = cx
            .debug_bounds("connection-list-panel")
            .expect("连接列表 Pulse 面板应渲染");
        let search = cx
            .debug_bounds("connection-search-field")
            .expect("连接搜索框应渲染");
        let add = cx
            .debug_bounds("add-connection")
            .expect("新建连接按钮应渲染");

        for (label, bounds) in [
            ("标题", title),
            ("工具栏", toolbar),
            ("列表", list),
            ("搜索", search),
            ("新建", add),
        ] {
            assert!(
                bounds.right() <= px(width) && bounds.bottom() <= px(height),
                "{label}不能越出窗口：{bounds:?} at {width}x{height}"
            );
        }
        assert!(title.size.width > px(0.0));
        assert!(toolbar.size.width > px(0.0));
        assert!(list.size.width > px(0.0));
        assert!(
            list.size.height >= px(400.0),
            "连接列表视口应填充标题栏下方的工作区：{list:?} at {width}px"
        );
        assert!(search.size.width > px(0.0));
        assert!(add.size.width > px(0.0));
    }
}

#[gpui_kit::test]
fn empty_connection_list_action_stays_visible_at_supported_sizes(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let storage: Arc<dyn Storage> = Arc::new(NoopStorage);
    let service = Arc::new(ConnectionService::new(HashMap::new(), storage.clone()));
    let redis_service = Arc::new(RedisService::new(
        Arc::new(ramag_infra_redis::RedisDriver::new()),
        storage.clone(),
    ));
    let mongo_service = Arc::new(MongoService::new(
        Arc::new(ramag_infra_mongodb::MongoDriver::new()),
        storage,
    ));
    let mut panel_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let panel = cx.new(|panel_cx| {
            let mut panel =
                ConnectionListPanel::new(service, redis_service, mongo_service, window, panel_cx);
            panel.refresh_generation = panel.refresh_generation.wrapping_add(1);
            panel.loading = false;
            panel.loaded_revision = panel.service.revision();
            panel
        });
        panel_entity = Some(panel.clone());
        ConnectionListTestHost { panel }
    });
    let panel = panel_entity.expect("连接列表面板应创建");

    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        panel.update(visual_cx, |_, cx| cx.notify());
        visual_cx.run_until_parked();

        let title = visual_cx
            .debug_bounds("connection-list-page-title")
            .expect("数据库客户端页面标题应渲染");
        let action = visual_cx
            .debug_bounds("connection-list-empty-add")
            .expect("空连接状态应提供可见的新建操作");

        assert!(
            action.origin.x >= px(0.0)
                && action.origin.y >= title.bottom()
                && action.right() <= px(width)
                && action.bottom() <= px(height),
            "空状态操作必须位于标题下方并保持在窗口内：action={action:?}, title={title:?}, size={width}x{height}"
        );
        assert!(
            action.size.width > px(0.0) && action.size.height > px(0.0),
            "空状态操作必须有可见尺寸：{action:?}"
        );
    }
}

#[gpui_kit::test]
fn connection_list_loading_and_failures_use_pulse_status_notices(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let storage: Arc<dyn Storage> = Arc::new(NoopStorage);
    let service = Arc::new(ConnectionService::new(HashMap::new(), storage.clone()));
    let redis_service = Arc::new(RedisService::new(
        Arc::new(ramag_infra_redis::RedisDriver::new()),
        storage.clone(),
    ));
    let mongo_service = Arc::new(MongoService::new(
        Arc::new(ramag_infra_mongodb::MongoDriver::new()),
        storage,
    ));
    let mut panel_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let entity = cx.new(|panel_cx| {
            let mut panel =
                ConnectionListPanel::new(service, redis_service, mongo_service, window, panel_cx);
            panel.refresh_generation = panel.refresh_generation.wrapping_add(1);
            panel.loading = true;
            panel.loaded_revision = panel.service.revision();
            panel
        });
        panel_entity = Some(entity.clone());
        ConnectionListTestHost { panel: entity }
    });
    let panel = panel_entity.expect("连接列表面板应创建");

    cx.simulate_resize(size(px(360.0), px(640.0)));
    cx.run_until_parked();
    let loading = cx
        .debug_bounds("connection-list-loading-notice")
        .expect("加载态应使用 Pulse 状态通知");
    assert!(loading.size.width > px(0.0) && loading.size.height > px(0.0));

    panel.update(cx, |panel, cx| {
        panel.loading = false;
        panel.load_error = Some("读取连接列表失败：存储暂时不可用".into());
        panel.connections = Arc::new(Vec::new());
        cx.notify();
    });
    cx.run_until_parked();
    let error = cx
        .debug_bounds("connection-list-error-notice")
        .expect("空列表失败态应使用 Pulse 状态通知");
    let retry = cx
        .debug_bounds("conn-list-retry")
        .expect("空列表失败态应保留重试入口");
    assert!(error.right() <= px(360.0));
    assert!(retry.right() <= px(360.0) && retry.bottom() <= px(640.0));

    panel.update(cx, |panel, cx| {
        panel.connections = Arc::new(vec![ConnectionConfig::new_mysql(
            "stale connection",
            "db.internal.example",
            3306,
            "ramag",
        )]);
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("connection-list-error-notice").is_some(),
        "保留旧列表时也必须显示加载失败原因"
    );
    assert!(
        cx.debug_bounds("connection-list-panel").is_some(),
        "加载失败不应隐藏仍可用的旧列表"
    );
}
