//! 原生 UI 验收专用驱动；所有数据库操作立即失败，不创建连接或网络任务。

use async_trait::async_trait;
use ramag_domain::entities::{
    ConnectionConfig, MongoCollection, MongoCollectionStats, MongoDatabase, MongoDocument,
    MongoIndex, MongoQueryResult, MongoQuerySpec, RedisType, RedisValue, RedisValueLoad,
    ScanResult,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::{DocDriver, KvDriver};

/// 即使在表单中改为 Redis/MongoDB，测试连接、版本读取和会话操作也不能发请求。
/// 未覆盖的可选 trait 方法沿用立即返回 NotImplemented 的默认实现。
pub(super) struct OfflineDatabase;

fn disabled<T>() -> Result<T> {
    Err(DomainError::NotImplemented(
        "UI 预览禁止数据库网络请求".into(),
    ))
}

#[async_trait]
impl KvDriver for OfflineDatabase {
    fn name(&self) -> &'static str {
        "redis-ui-preview"
    }

    async fn test_connection(&self, _config: &ConnectionConfig) -> Result<()> {
        disabled()
    }

    async fn server_version(&self, _config: &ConnectionConfig) -> Result<String> {
        disabled()
    }

    async fn db_size(&self, _config: &ConnectionConfig, _db: u8) -> Result<u64> {
        disabled()
    }

    async fn scan(
        &self,
        _config: &ConnectionConfig,
        _db: u8,
        _cursor: u64,
        _match_pattern: Option<&str>,
        _type_filter: Option<RedisType>,
        _count: u32,
    ) -> Result<ScanResult> {
        disabled()
    }

    async fn key_type(&self, _config: &ConnectionConfig, _db: u8, _key: &str) -> Result<RedisType> {
        disabled()
    }

    async fn key_ttl(&self, _config: &ConnectionConfig, _db: u8, _key: &str) -> Result<i64> {
        disabled()
    }

    async fn get_value(
        &self,
        _config: &ConnectionConfig,
        _db: u8,
        _key: &str,
    ) -> Result<RedisValue> {
        disabled()
    }

    async fn get_value_limited(
        &self,
        _config: &ConnectionConfig,
        _db: u8,
        _key: &str,
        _limit: usize,
    ) -> Result<RedisValueLoad> {
        disabled()
    }

    fn is_write_command(&self, _command: &str) -> bool {
        true
    }

    async fn delete_key(&self, _config: &ConnectionConfig, _db: u8, _key: &str) -> Result<bool> {
        disabled()
    }

    async fn set_ttl(
        &self,
        _config: &ConnectionConfig,
        _db: u8,
        _key: &str,
        _ttl_secs: Option<i64>,
    ) -> Result<bool> {
        disabled()
    }

    async fn execute_command(
        &self,
        _config: &ConnectionConfig,
        _db: u8,
        _argv: Vec<String>,
    ) -> Result<RedisValue> {
        disabled()
    }

    async fn info(&self, _config: &ConnectionConfig, _sections: &[&str]) -> Result<String> {
        disabled()
    }
}

#[async_trait]
impl DocDriver for OfflineDatabase {
    fn name(&self) -> &'static str {
        "mongodb-ui-preview"
    }

    async fn test_connection(&self, _config: &ConnectionConfig) -> Result<()> {
        disabled()
    }

    async fn server_version(&self, _config: &ConnectionConfig) -> Result<String> {
        disabled()
    }

    async fn list_databases(&self, _config: &ConnectionConfig) -> Result<Vec<MongoDatabase>> {
        disabled()
    }

    async fn list_collections(
        &self,
        _config: &ConnectionConfig,
        _db: &str,
    ) -> Result<Vec<MongoCollection>> {
        disabled()
    }

    async fn list_indexes(
        &self,
        _config: &ConnectionConfig,
        _db: &str,
        _coll: &str,
    ) -> Result<Vec<MongoIndex>> {
        disabled()
    }

    async fn collection_stats(
        &self,
        _config: &ConnectionConfig,
        _db: &str,
        _coll: &str,
    ) -> Result<MongoCollectionStats> {
        disabled()
    }

    async fn find(
        &self,
        _config: &ConnectionConfig,
        _db: &str,
        _coll: &str,
        _spec: &MongoQuerySpec,
    ) -> Result<MongoQueryResult> {
        disabled()
    }

    async fn count(
        &self,
        _config: &ConnectionConfig,
        _db: &str,
        _coll: &str,
        _filter: &MongoDocument,
    ) -> Result<u64> {
        disabled()
    }

    async fn aggregate(
        &self,
        _config: &ConnectionConfig,
        _db: &str,
        _coll: &str,
        _pipeline: Vec<MongoDocument>,
    ) -> Result<MongoQueryResult> {
        disabled()
    }

    async fn insert_one(
        &self,
        _config: &ConnectionConfig,
        _db: &str,
        _coll: &str,
        _document: MongoDocument,
    ) -> Result<String> {
        disabled()
    }

    async fn update_one(
        &self,
        _config: &ConnectionConfig,
        _db: &str,
        _coll: &str,
        _filter: &MongoDocument,
        _update: &MongoDocument,
    ) -> Result<MongoQueryResult> {
        disabled()
    }

    async fn delete_one(
        &self,
        _config: &ConnectionConfig,
        _db: &str,
        _coll: &str,
        _filter: &MongoDocument,
    ) -> Result<MongoQueryResult> {
        disabled()
    }

    async fn run_command(
        &self,
        _config: &ConnectionConfig,
        _db: &str,
        _command: MongoDocument,
    ) -> Result<MongoDocument> {
        disabled()
    }
}
