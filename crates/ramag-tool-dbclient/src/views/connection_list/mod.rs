//! 连接列表、搜索与版本预取。

mod render;
mod row;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext as _, Context, Entity, EventEmitter, Window};
use ramag_app::{ConnectionService, MongoService, RedisService};
use ramag_domain::entities::{
    ConnectionConfig, ConnectionId, DriverKind, contains_case_insensitive,
};
use tracing::{debug, error};

pub struct ConnectionListPanel {
    pub(super) service: Arc<ConnectionService>,
    redis_service: Arc<RedisService>,
    mongo_service: Arc<MongoService>,
    pub(super) connections: Arc<Vec<ConnectionConfig>>,
    filtered_indices_cache: RefCell<Option<FilteredIndicesCacheEntry>>,
    pub(super) selected: Option<ConnectionId>,
    pub(super) loading: bool,
    /// 加载失败信息。
    pub(super) load_error: Option<String>,
    pub(super) search: Entity<InputState>,
    /// 小写搜索词。
    pub(super) query: String,
    /// 服务端版本缓存。
    pub(super) versions: HashMap<ConnectionId, String>,
    /// 版本探测状态；取消后完成时仍清理资源。
    version_requests: HashMap<ConnectionId, VersionRequest>,
    refresh_generation: u64,
    loaded_revision: u64,
    pub(super) focused_search_once: bool,
    _subscriptions: Vec<gpui_kit::Subscription>,
}

struct FilteredIndicesCacheEntry {
    connections: Arc<Vec<ConnectionConfig>>,
    query_lower: String,
    indices: Arc<Vec<usize>>,
}

struct VersionRequest {
    config: ConnectionConfig,
    cancelled: Arc<AtomicBool>,
    /// 旧请求清理后重试的新配置。
    restart_config: Option<ConnectionConfig>,
}

impl Drop for VersionRequest {
    fn drop(&mut self) {
        // 后台请求可能仍在运行，需标记取消。
        self.cancelled.store(true, Ordering::Release);
    }
}

impl FilteredIndicesCacheEntry {
    fn get(
        &self,
        connections: &Arc<Vec<ConnectionConfig>>,
        query_lower: &str,
    ) -> Option<Arc<Vec<usize>>> {
        (Arc::ptr_eq(&self.connections, connections) && self.query_lower == query_lower)
            .then(|| self.indices.clone())
    }
}

#[derive(Debug, Clone)]
pub enum ListEvent {
    Selected(ConnectionConfig),
    RequestNew,
    RequestSync(ConnectionConfig),
    RequestEdit(ConnectionConfig),
    RequestDuplicate(ConnectionConfig),
    RequestDelete(ConnectionId),
    /// 通知根视图清理旧连接资源。
    ConnectionsChanged(Vec<ConnectionConfig>),
}

impl EventEmitter<ListEvent> for ConnectionListPanel {}

impl ConnectionListPanel {
    pub fn new(
        service: Arc<ConnectionService>,
        redis_service: Arc<RedisService>,
        mongo_service: Arc<MongoService>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| {
            ramag_ui::bounded_search_input(window, cx).placeholder("搜索连接（名称 / 环境 / 类型）")
        });

        let mut subs = Vec::new();
        // set_value 不发 Change，需观察实体处理清除。
        subs.push(cx.observe(&search, |this: &mut Self, _, cx| {
            let query = this.search.read(cx).value().trim().to_lowercase();
            if query == this.query {
                return;
            }
            this.query = query;
            cx.notify();
        }));

        let loaded_revision = service.revision();
        let mut this = Self {
            service,
            redis_service,
            mongo_service,
            connections: Arc::new(Vec::new()),
            filtered_indices_cache: RefCell::new(None),
            selected: None,
            loading: true,
            load_error: None,
            search,
            query: String::new(),
            versions: HashMap::new(),
            version_requests: HashMap::new(),
            refresh_generation: 0,
            loaded_revision,
            focused_search_once: false,
            _subscriptions: subs,
        };
        this.refresh(cx);
        this
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.refresh_generation = self.refresh_generation.wrapping_add(1);
        let generation = self.refresh_generation;
        self.loading = true;
        cx.notify();
        let svc = self.service.clone();
        cx.spawn(async move |this, cx| {
            let revision = svc.revision();
            let result = svc.list().await;
            let _ = this.update(cx, |this, cx| {
                if this.refresh_generation != generation {
                    return;
                }
                this.loading = false;
                this.loaded_revision = revision;
                match result {
                    Ok(list) => {
                        let changed = changed_connections(&this.connections, &list);
                        let current_ids: HashSet<ConnectionId> = list
                            .iter()
                            .map(|connection| connection.id.clone())
                            .collect();
                        // 同一连接 ID 的配置可能更换主机、端口或凭据；旧版本探测不能
                        // 继续代表新端点。取消在途请求并清除缓存，下次打开连接时按新配置重探测。
                        for config in &changed {
                            this.invalidate_version(&config.id);
                        }
                        this.versions.retain(|id, _| current_ids.contains(id));
                        this.connections = Arc::new(list);
                        this.filtered_indices_cache.get_mut().take();
                        this.load_error = None;
                        if !changed.is_empty() {
                            cx.emit(ListEvent::ConnectionsChanged(changed));
                        }
                    }
                    Err(e) => {
                        error!(operation = "connection_list_load", error = %e, "load connections failed");
                        // 保留旧列表，避免伪装成空态。
                        this.load_error = Some(format!("加载连接列表失败：{e}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 未缓存时预取版本。
    pub fn prefetch_version(&mut self, conn: &ConnectionConfig, cx: &mut Context<Self>) {
        if self.versions.contains_key(&conn.id) {
            return;
        }
        if let Some(request) = self.version_requests.get_mut(&conn.id) {
            if request.config == *conn {
                // 同配置重开时复用原探测。
                request.cancelled.store(false, Ordering::Release);
                request.restart_config = None;
            } else {
                // 旧请求清理后再探测新配置。
                request.cancelled.store(true, Ordering::Release);
                request.restart_config = Some(conn.clone());
            }
            return;
        }

        let conn = conn.clone();
        let mysql_svc = self.service.clone();
        let redis_svc = self.redis_service.clone();
        let mongo_svc = self.mongo_service.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.version_requests.insert(
            conn.id.clone(),
            VersionRequest {
                config: conn.clone(),
                cancelled: cancelled.clone(),
                restart_config: None,
            },
        );
        cx.spawn(async move |this, cx| {
            let result = match conn.driver {
                DriverKind::Mysql | DriverKind::Postgres | DriverKind::Sqlite => {
                    mysql_svc.server_version(&conn).await
                }
                DriverKind::Redis => redis_svc.server_version(&conn).await,
                DriverKind::Mongodb => mongo_svc.server_version(&conn).await,
            };
            let update_result = this.update(cx, |this, cx| {
                let Some(current) = this.version_requests.get(&conn.id) else {
                    return;
                };
                if !Arc::ptr_eq(&current.cancelled, &cancelled) {
                    return;
                }
                let was_cancelled = cancelled.load(Ordering::Acquire);
                let Some(request) = this.version_requests.remove(&conn.id) else {
                    return;
                };
                let restart = request.restart_config.clone();

                if was_cancelled {
                    // 配置刷新、关闭标签和保存表单都会在发出取消后由拥有者
                    // 清理连接池。这里不能再按连接 ID 清理：新配置可能已经
                    // 建立了同 ID 的新池，旧探测回包会误删新连接。
                } else {
                    match result {
                        Ok(version) => {
                            this.versions.insert(conn.id.clone(), version);
                            cx.notify();
                        }
                        Err(error) => {
                            debug!(
                                operation = "server_version_prefetch",
                                connection_id = %conn.id,
                                driver = ?conn.driver,
                                host = %conn.host,
                                error = %error,
                                "fetch server version failed"
                            );
                        }
                    }
                }

                if let Some(next) = restart {
                    this.versions.remove(&next.id);
                    this.prefetch_version(&next, cx);
                }
            });
            if update_result.is_err() {
                // 面板已销毁，释放此次探测资源。
                evict_version_resources(&mysql_svc, &redis_svc, &mongo_svc, &conn.id);
            }
        })
        .detach();
    }

    /// 标记版本探测不再需要。
    pub fn cancel_version_prefetch(&mut self, id: &ConnectionId) {
        if let Some(request) = self.version_requests.get_mut(id) {
            request.cancelled.store(true, Ordering::Release);
            request.restart_config = None;
        }
    }

    /// 使版本缓存失效并取消旧探测。
    pub fn invalidate_version(&mut self, id: &ConnectionId) {
        invalidate_version_state(&mut self.versions, &mut self.version_requests, id);
    }

    pub fn connections(&self) -> &[ConnectionConfig] {
        self.connections.as_slice()
    }

    pub(super) fn handle_click(&mut self, conn: ConnectionConfig, cx: &mut Context<Self>) {
        self.selected = Some(conn.id.clone());
        cx.emit(ListEvent::Selected(conn));
        cx.notify();
    }

    pub(super) fn filtered_indices(&self) -> Arc<Vec<usize>> {
        {
            let cache = self.filtered_indices_cache.borrow();
            if let Some(indices) = cache
                .as_ref()
                .and_then(|entry| entry.get(&self.connections, &self.query))
            {
                return indices;
            }
        }

        let q = &self.query;
        let indices: Arc<Vec<usize>> = Arc::new(
            self.connections
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    contains_case_insensitive(&c.name, q)
                        || c.environment
                            .as_deref()
                            .is_some_and(|environment| contains_case_insensitive(environment, q))
                        || contains_case_insensitive(driver_search_label(c.driver), q)
                })
                .map(|(index, _)| index)
                .collect(),
        );
        self.filtered_indices_cache
            .replace(Some(FilteredIndicesCacheEntry {
                connections: self.connections.clone(),
                query_lower: self.query.clone(),
                indices: indices.clone(),
            }));
        indices
    }
}

fn changed_connections(
    previous: &[ConnectionConfig],
    current: &[ConnectionConfig],
) -> Vec<ConnectionConfig> {
    let previous_by_id: HashMap<&ConnectionId, &ConnectionConfig> =
        previous.iter().map(|config| (&config.id, config)).collect();
    let current_ids: HashSet<&ConnectionId> = current.iter().map(|config| &config.id).collect();
    current
        .iter()
        .filter(|config| previous_by_id.get(&config.id).copied() != Some(*config))
        .cloned()
        .chain(
            previous
                .iter()
                .filter(|config| !current_ids.contains(&config.id))
                .cloned(),
        )
        .collect()
}

/// 清理一个连接的版本缓存和在途探测状态。
fn invalidate_version_state(
    versions: &mut HashMap<ConnectionId, String>,
    version_requests: &mut HashMap<ConnectionId, VersionRequest>,
    id: &ConnectionId,
) {
    versions.remove(id);
    if let Some(request) = version_requests.get_mut(id) {
        request.cancelled.store(true, Ordering::Release);
        request.restart_config = None;
    }
}

/// 可写且同引擎存在另一连接时允许同步。
fn syncable_target_ids(connections: &[ConnectionConfig]) -> HashSet<ConnectionId> {
    let mut driver_connections: HashMap<DriverKind, HashSet<ConnectionId>> = HashMap::new();
    for connection in connections {
        driver_connections
            .entry(connection.driver)
            .or_default()
            .insert(connection.id.clone());
    }
    connections
        .iter()
        .filter(|connection| {
            !matches!(connection.driver, DriverKind::Redis | DriverKind::Sqlite)
                && !connection.production
                && driver_connections
                    .get(&connection.driver)
                    .is_some_and(|ids| ids.len() > 1)
        })
        .map(|connection| connection.id.clone())
        .collect()
}

/// 搜索用的数据库类型名。
fn driver_search_label(driver: DriverKind) -> &'static str {
    match driver {
        DriverKind::Mysql => "MySQL",
        DriverKind::Postgres => "PostgreSQL",
        DriverKind::Sqlite => "SQLite",
        DriverKind::Redis => "Redis",
        DriverKind::Mongodb => "MongoDB",
    }
}

fn evict_version_resources(
    sql: &ConnectionService,
    redis: &RedisService,
    mongo: &MongoService,
    id: &ConnectionId,
) {
    sql.evict_all_pools(id);
    redis.evict_pool(id);
    mongo.evict_pool(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtered_indices_cache_requires_same_source_and_query() {
        let connections = Arc::new(Vec::new());
        let indices = Arc::new(Vec::new());
        let cache = FilteredIndicesCacheEntry {
            connections: connections.clone(),
            query_lower: "local".into(),
            indices: indices.clone(),
        };

        let cached = cache.get(&connections, "local");
        assert!(
            cached
                .as_ref()
                .is_some_and(|value| Arc::ptr_eq(value, &indices))
        );
        assert!(cache.get(&connections, "remote").is_none());
        assert!(cache.get(&Arc::new(Vec::new()), "local").is_none());
    }

    #[test]
    fn dropping_version_request_marks_background_cleanup_required() {
        let cancelled = Arc::new(AtomicBool::new(false));
        {
            let _request = VersionRequest {
                config: ConnectionConfig::new_mysql("local", "127.0.0.1", 3306, "root"),
                cancelled: cancelled.clone(),
                restart_config: None,
            };
        }

        assert!(cancelled.load(Ordering::Acquire));
    }

    #[test]
    fn changed_connections_includes_updates_additions_and_deletions() {
        let deleted = ConnectionConfig::new_mysql("deleted", "127.0.0.1", 3306, "root");
        let unchanged = ConnectionConfig::new_redis("same", "127.0.0.1", 6379);
        let mut updated = unchanged.clone();
        updated.name = "updated".into();
        let added = ConnectionConfig::new_redis("added", "127.0.0.1", 6380);

        let changed = changed_connections(
            &[deleted.clone(), unchanged],
            &[updated.clone(), added.clone()],
        );
        assert_eq!(changed, vec![updated, added, deleted]);
    }

    #[test]
    fn changed_connection_invalidates_version_cache_and_request() {
        let previous = ConnectionConfig::new_mysql("local", "127.0.0.1", 13318, "root");
        let mut current = previous.clone();
        current.port = 13306;
        current.username = "ramag".into();
        let changed = changed_connections(
            std::slice::from_ref(&previous),
            std::slice::from_ref(&current),
        );
        assert_eq!(changed, vec![current.clone()]);

        let mut versions = HashMap::from([(current.id.clone(), "8.4.9".to_string())]);
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut version_requests = HashMap::from([(
            current.id.clone(),
            VersionRequest {
                config: previous,
                cancelled: cancelled.clone(),
                restart_config: Some(current.clone()),
            },
        )]);

        invalidate_version_state(&mut versions, &mut version_requests, &current.id);

        assert!(versions.is_empty());
        assert!(cancelled.load(Ordering::Acquire));
        assert_eq!(
            version_requests
                .get(&current.id)
                .and_then(|request| request.restart_config.as_ref()),
            None
        );
    }

    #[test]
    fn sync_entry_requires_writable_target_and_another_same_engine_connection() {
        let mysql_source = ConnectionConfig::new_mysql("source", "mysql-a", 3306, "root");
        let mysql_target = ConnectionConfig::new_mysql("target", "mysql-b", 3306, "root");
        let mut postgres_source =
            ConnectionConfig::new_mysql("pg-source", "postgres-a", 5432, "postgres");
        postgres_source.driver = DriverKind::Postgres;
        let mut postgres_target =
            ConnectionConfig::new_mysql("pg-target", "postgres-b", 5432, "postgres");
        postgres_target.driver = DriverKind::Postgres;
        let mongo_source = ConnectionConfig::new_mongodb("mongo-source", "mongo-a", 27017);
        let mongo_target = ConnectionConfig::new_mongodb("mongo-target", "mongo-b", 27017);
        let redis_source = ConnectionConfig::new_redis("redis-source", "redis-a", 6379);
        let redis_target = ConnectionConfig::new_redis("redis-target", "redis-b", 6379);
        let sqlite_source = ConnectionConfig::new_sqlite("sqlite-source", "sqlite-a.db");
        let sqlite_target = ConnectionConfig::new_sqlite("sqlite-target", "sqlite-b.db");
        let mut production_mysql =
            ConnectionConfig::new_mysql("production", "mysql-c", 3306, "root");
        production_mysql.production = true;

        let lone_postgres = syncable_target_ids(std::slice::from_ref(&postgres_source));
        assert!(!lone_postgres.contains(&postgres_source.id));

        let syncable = syncable_target_ids(&[
            mysql_source.clone(),
            mysql_target.clone(),
            postgres_source.clone(),
            postgres_target.clone(),
            mongo_source.clone(),
            mongo_target.clone(),
            redis_source.clone(),
            redis_target.clone(),
            sqlite_source.clone(),
            sqlite_target.clone(),
            production_mysql.clone(),
        ]);

        assert!(syncable.contains(&mysql_source.id));
        assert!(syncable.contains(&mysql_target.id));
        assert!(syncable.contains(&postgres_source.id));
        assert!(syncable.contains(&postgres_target.id));
        assert!(syncable.contains(&mongo_source.id));
        assert!(syncable.contains(&mongo_target.id));
        assert!(!syncable.contains(&redis_source.id));
        assert!(!syncable.contains(&redis_target.id));
        assert!(!syncable.contains(&sqlite_source.id));
        assert!(!syncable.contains(&sqlite_target.id));
        assert!(!syncable.contains(&production_mysql.id));
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod render_tests;
