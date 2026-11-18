//! DBClient 原生验收数据与失败控制；只包装本次预览的临时存储。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use async_trait::async_trait;
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Entity, IntoElement, KeyBinding, Render, Styled, Window, div};
use ramag_app::{
    ConnectionService, DataSyncGate, DataSyncPermit, DataSyncService, MongoService, RedisService,
};
use ramag_domain::entities::{
    ConnectionConfig, ConnectionId, DriverKind, Query, QueryRecord, QueryRecordId, TlsVerify,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::{Driver, Storage};
use ramag_infra_sqlite::SqliteDriver;
use ramag_ui::Shell;

gpui_kit::actions!(
    dbclient_ui_preview,
    [FailReload, RecoverReload, LongReloadError]
);

#[derive(Clone, Copy)]
enum ReadMode {
    Healthy = 0,
    Failed = 1,
    LongError = 2,
}

/// 失败只影响列表读取；连接、历史和偏好写入仍由临时 redb 完成。
/// 原子状态仅用于验收控制，不改变生产 Storage 或读取失败时的数据。
struct PreviewStorage {
    backing: Arc<dyn Storage>,
    mode: AtomicU8,
    reads: AtomicU64,
}

#[async_trait]
impl Storage for PreviewStorage {
    async fn list_connections(&self) -> Result<Vec<ConnectionConfig>> {
        let read = self.reads.fetch_add(1, Ordering::Relaxed) + 1;
        match self.mode.load(Ordering::Acquire) {
            0 => self.backing.list_connections().await,
            2 => Err(DomainError::Storage(format!(
                "UI 验收模拟长错误（第 {read} 次读取）：{}",
                "临时连接列表读取失败，请保留搜索条件和旧连接。".repeat(30)
            ))),
            _ => Err(DomainError::Storage(format!(
                "UI 验收模拟读取失败（第 {read} 次读取）"
            ))),
        }
    }

    async fn get_connection(&self, id: &ConnectionId) -> Result<Option<ConnectionConfig>> {
        self.backing.get_connection(id).await
    }

    async fn save_connection(&self, config: &ConnectionConfig) -> Result<()> {
        self.backing.save_connection(config).await
    }

    async fn save_connections(&self, configs: &[ConnectionConfig]) -> Result<()> {
        self.backing.save_connections(configs).await
    }

    async fn delete_connection(&self, id: &ConnectionId) -> Result<()> {
        self.backing.delete_connection(id).await
    }

    async fn append_history(&self, record: &QueryRecord) -> Result<()> {
        self.backing.append_history(record).await
    }

    async fn list_history(
        &self,
        connection_id: Option<&ConnectionId>,
        limit: usize,
    ) -> Result<Vec<QueryRecord>> {
        self.backing.list_history(connection_id, limit).await
    }

    async fn delete_history(&self, id: &QueryRecordId) -> Result<()> {
        self.backing.delete_history(id).await
    }

    async fn clear_history(&self, connection_id: Option<&ConnectionId>) -> Result<()> {
        self.backing.clear_history(connection_id).await
    }

    async fn get_preference(&self, key: &str) -> Result<Option<String>> {
        self.backing.get_preference(key).await
    }

    async fn set_preference(&self, key: &str, value: &str) -> Result<()> {
        self.backing.set_preference(key, value).await
    }

    async fn delete_preference(&self, key: &str) -> Result<()> {
        self.backing.delete_preference(key).await
    }
}

/// 每次启动新建三条无密码 MySQL 记录。SQL 驱动表为空，连接和同步
/// 在驱动查找阶段报错；这些预置记录不会向保留地址发送数据库请求。
pub(super) struct DbclientPreview {
    pub connection: Arc<ConnectionService>,
    pub gate: Arc<DataSyncGate>,
    pub sqlite_config: Option<ConnectionConfig>,
    storage: Arc<PreviewStorage>,
    sample: ConnectionConfig,
    redis: Arc<RedisService>,
    mongo: Arc<MongoService>,
}

impl DbclientPreview {
    pub fn new(backing: Arc<dyn Storage>) -> anyhow::Result<Self> {
        Self::new_inner(backing, None)
    }

    /// 仅为显式 SQLite 设计器验收模式注册本地驱动并保存临时连接。
    pub fn new_sqlite_designer(
        backing: Arc<dyn Storage>,
        sqlite_path: &Path,
    ) -> anyhow::Result<Self> {
        let config = ConnectionConfig::new_sqlite(
            "UI 表设计器验收",
            sqlite_path.to_string_lossy().into_owned(),
        );
        Self::new_inner(backing, Some(config))
    }

    fn new_inner(
        backing: Arc<dyn Storage>,
        sqlite_config: Option<ConnectionConfig>,
    ) -> anyhow::Result<Self> {
        let configs = sqlite_config
            .as_ref()
            .map_or_else(sample_connections, |config| vec![config.clone()]);
        futures::executor::block_on(backing.save_connections(&configs))?;
        let offline = Arc::new(super::offline_database::OfflineDatabase);
        let redis = Arc::new(RedisService::new(offline.clone(), backing.clone()));
        let mongo = Arc::new(MongoService::new(offline, backing.clone()));
        let storage = Arc::new(PreviewStorage {
            backing,
            mode: AtomicU8::new(ReadMode::Healthy as u8),
            reads: AtomicU64::new(0),
        });
        let mut drivers = HashMap::new();
        if sqlite_config.is_some() {
            let driver: Arc<dyn Driver> = Arc::new(SqliteDriver::new());
            drivers.insert(DriverKind::Sqlite, driver);
        }
        let connection = Arc::new(ConnectionService::new(drivers, storage.clone()));
        if let Some(config) = &sqlite_config {
            seed_sqlite_designer_table(&connection, config)?;
        }
        Ok(Self {
            connection,
            gate: Arc::new(DataSyncGate::default()),
            sqlite_config,
            storage,
            sample: configs[0].clone(),
            redis,
            mongo,
        })
    }

    pub fn create_view(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<ramag_tool_dbclient::DbClientView> {
        let sync = Arc::new(DataSyncService::new(
            self.connection.clone(),
            self.mongo.clone(),
            self.gate.clone(),
        ));
        ramag_tool_dbclient::create_dbclient_view(
            self.connection.clone(),
            self.redis.clone(),
            self.mongo.clone(),
            sync,
            window,
            cx,
        )
    }
}

/// 只在独立 SQLite 预览中写入本地表和一条样例记录。
fn seed_sqlite_designer_table(
    connection: &ConnectionService,
    config: &ConnectionConfig,
) -> anyhow::Result<()> {
    let query = Query::new(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            display_name TEXT NOT NULL,
            email TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            quota INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT,
            notes TEXT,
            region TEXT,
            phone TEXT,
            tenant_id TEXT NOT NULL,
            retry_count INTEGER NOT NULL DEFAULT 0,
            is_enabled INTEGER NOT NULL DEFAULT 1,
            metadata_json TEXT,
            reference_code TEXT,
            archived_at TEXT
        );
        INSERT INTO users (
            display_name, email, status, quota, created_at, updated_at,
            notes, region, phone, tenant_id, retry_count, is_enabled,
            metadata_json, reference_code, archived_at
        ) VALUES (
            'UI preview', 'preview@example.invalid', 'active', 12,
            '2026-01-01T00:00:00Z', NULL, 'local acceptance data', 'test',
            '000-0000', 'preview-tenant', 0, 1, '{\"source\":\"ui-preview\"}',
            'preview-001', NULL
        );",
    )
    .transactional();
    futures::executor::block_on(connection.execute(config, &query))?;
    Ok(())
}

fn sample_connections() -> Vec<ConnectionConfig> {
    [
        ("UI acceptance source", "192.0.2.10", false),
        ("UI acceptance target", "192.0.2.11", false),
        (
            "UI acceptance production long connection name",
            "192.0.2.12",
            true,
        ),
    ]
    .into_iter()
    .map(|(name, host, production)| ConnectionConfig {
        id: ConnectionId::new(),
        name: name.into(),
        driver: DriverKind::Mysql,
        host: host.into(),
        port: 3306,
        username: "preview".into(),
        password: String::new(),
        database: Some("preview".into()),
        auth_source: None,
        remark: None,
        environment: Some(if production { "production" } else { "test" }.into()),
        production,
        tls: false,
        tls_verify: TlsVerify::default(),
        ca_cert_path: None,
        ssh_target: None,
        ssh_port: None,
    })
    .collect()
}

pub(super) fn bind_controls(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("f6", FailReload, None),
        KeyBinding::new("f7", RecoverReload, None),
        KeyBinding::new("f8", LongReloadError, None),
    ]);
}

/// Shell 的祖先接收验收快捷键，子视图及所有行操作仍使用生产实现。
pub(super) struct PreviewHost {
    pub shell: Entity<Shell>,
    pub preview: DbclientPreview,
    /// 保留受控同步许可，确保预览窗口中的运行状态不会被提前释放。
    pub _sync_permit: Option<DataSyncPermit>,
}

impl PreviewHost {
    /// 正常保存路径增加修订号，触发生产列表刷新。只重存临时样例，
    /// 不写用户连接。窗口关闭后弱实体更新失败即结束，不重建窗口。
    fn change_mode(&mut self, mode: ReadMode, cx: &mut Context<Self>) {
        self.preview
            .storage
            .mode
            .store(mode as u8, Ordering::Release);
        let service = self.preview.connection.clone();
        let sample = self.preview.sample.clone();
        cx.spawn(async move |this, cx| {
            if let Err(error) = service.save(&sample).await {
                eprintln!("UI preview refresh failed: {error}");
            }
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
}

impl Render for PreviewHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .on_action(cx.listener(|this, _: &FailReload, _, cx| {
                this.change_mode(ReadMode::Failed, cx);
            }))
            .on_action(cx.listener(|this, _: &RecoverReload, _, cx| {
                this.change_mode(ReadMode::Healthy, cx);
            }))
            .on_action(cx.listener(|this, _: &LongReloadError, _, cx| {
                this.change_mode(ReadMode::LongError, cx);
            }))
            .child(self.shell.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_designer_preview_seeds_local_table_and_only_registers_sqlite() -> anyhow::Result<()> {
        let mut directory = crate::PreviewSqliteDirectory::create()?;
        let sqlite_path = directory.database_path();
        let redb_path = directory.path.join("preview.redb");
        let backing: Arc<dyn Storage> = Arc::new(ramag_infra_storage::RedbStorage::open_with_key(
            &redb_path,
            &[0x42; 32],
        )?);
        {
            let preview = DbclientPreview::new_sqlite_designer(backing.clone(), &sqlite_path)?;
            let config = preview
                .sqlite_config
                .as_ref()
                .expect("SQLite 预览应保存本地连接");
            let connections = futures::executor::block_on(preview.connection.list())?;
            assert_eq!(connections.len(), 1);
            assert_eq!(connections[0].id, config.id);
            assert_eq!(connections[0].driver, DriverKind::Sqlite);

            let schemas = futures::executor::block_on(preview.connection.list_schemas(config))?;
            assert!(schemas.iter().any(|schema| schema.name == "main"));
            let tables =
                futures::executor::block_on(preview.connection.list_tables(config, "main"))?;
            assert!(tables.iter().any(|table| table.name == "users"));
            let columns = futures::executor::block_on(
                preview.connection.list_columns(config, "main", "users"),
            )?;
            assert_eq!(columns.len(), 16);

            let result = futures::executor::block_on(
                preview
                    .connection
                    .execute(config, &Query::new("SELECT * FROM users")),
            )?;
            assert_eq!(result.rows.len(), 1);
            assert_eq!(result.columns.len(), 16);

            preview.connection.evict_pool(config);
        }
        drop(backing);
        directory.cleanup()?;
        Ok(())
    }

    #[test]
    fn failed_list_reads_preserve_records_and_other_storage_operations() -> anyhow::Result<()> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ramag-preview-test-{}-{nonce}.redb",
            std::process::id()
        ));
        let backing: Arc<dyn Storage> = Arc::new(ramag_infra_storage::RedbStorage::open_with_key(
            &path,
            &[0x42; 32],
        )?);
        {
            let preview = DbclientPreview::new(backing.clone())?;
            futures::executor::block_on(async {
                let before = preview.connection.list().await?;
                assert_eq!(before.len(), 3);
                preview
                    .storage
                    .mode
                    .store(ReadMode::Failed as u8, Ordering::Release);
                let first = preview.connection.list().await;
                let second = preview.connection.list().await;
                assert!(first.is_err());
                assert!(second.is_err());
                assert_ne!(
                    first.err().map(|e| e.to_string()),
                    second.err().map(|e| e.to_string())
                );
                assert_eq!(backing.list_connections().await?, before);
                let revision = preview.connection.revision();
                preview.connection.save(&preview.sample).await?;
                assert!(preview.connection.revision() > revision);
                preview
                    .storage
                    .set_preference("preview", "preserved")
                    .await?;
                assert_eq!(
                    preview.storage.get_preference("preview").await?,
                    Some("preserved".into())
                );
                preview
                    .storage
                    .mode
                    .store(ReadMode::LongError as u8, Ordering::Release);
                assert!(
                    preview.connection.list().await.err().is_some_and(|e| e
                        .to_string()
                        .chars()
                        .count()
                        > 240)
                );
                preview
                    .storage
                    .mode
                    .store(ReadMode::Healthy as u8, Ordering::Release);
                assert_eq!(preview.connection.list().await?, before);
                // 验收组合中的实际服务必须拒绝表单改为 Redis/MongoDB 后的连接测试。
                let mut config = preview.sample.clone();
                config.driver = DriverKind::Redis;
                assert!(
                    preview
                        .redis
                        .test(&config)
                        .await
                        .err()
                        .is_some_and(|e| e.to_string().contains("UI 预览禁止数据库网络请求"))
                );
                config.driver = DriverKind::Mongodb;
                assert!(
                    preview
                        .mongo
                        .test(&config)
                        .await
                        .err()
                        .is_some_and(|e| e.to_string().contains("UI 预览禁止数据库网络请求"))
                );
                Ok::<_, anyhow::Error>(())
            })?;
        }
        drop(backing);
        std::fs::remove_file(path)?;
        Ok(())
    }
}
