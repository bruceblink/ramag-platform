#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

//! 本地存储：redb 嵌入式 DB；密码 AES-GCM 加密，主密钥存系统凭据库。
//! 业务按表拆到 `repos` 子模块（同步），lib 用 `run_blocking` 异步化。
//! 数据目录由 `directories::ProjectDirs` 按当前平台定位。

mod encrypted_records;
pub mod encryption;
pub mod keyring;
mod repos;
mod worker_pool;

pub(crate) use encrypted_records::has_encrypted_records as database_has_encrypted_records;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use async_trait::async_trait;
use directories::ProjectDirs;
use parking_lot::RwLock;
use redb::Database;
use tracing::{debug, info, warn};

use ramag_domain::entities::{
    ApiHistoryRecord, ApiWorkspace, ApiWorkspaceId, ClipId, ClipItem, ClipSearchResult,
    CollaborationShare, CollaborationShareId, ConnectionConfig, ConnectionId, KafkaClusterConfig,
    KafkaClusterId, MAX_CLIPBOARD_SEARCH_BYTES, MqttProfile, MqttProfileId, ObjectStorageAccount,
    ObjectStorageAccountId, QueryHistoryPage, QueryRecord, QueryRecordId, RepoConfig, RepoId,
    SshProfile, SshProfileId,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::Storage;

use crate::encryption::Cipher;
use crate::worker_pool::run as run_blocking;

pub struct RedbStorage {
    db: Arc<Database>,
    cipher: Arc<RwLock<Cipher>>,
    path: PathBuf,
}

impl RedbStorage {
    /// 默认路径，首次会创建文件并在系统凭据库生成主密钥
    pub fn open_default() -> Result<Self> {
        let path = default_db_path()?;
        Self::open(&path)
    }

    /// 生产入口：从系统凭据库读取主密钥
    pub fn open(path: &Path) -> Result<Self> {
        // 先获取 redb 的进程级文件锁，防止两个首次启动进程竞争生成并覆盖主密钥。
        let db = open_database(path)?;
        let allow_create = !database_has_encrypted_records(&db)?;
        let master_key = keyring::get_or_create_master_key(allow_create)?;
        Self::initialize(db, path, &master_key)
    }

    /// 测试入口：注入固定密钥，避免污染真实系统凭据库
    pub fn open_with_key(path: &Path, master_key: &[u8; 32]) -> Result<Self> {
        let db = open_database(path)?;
        Self::initialize(db, path, master_key)
    }

    fn initialize(db: Database, path: &Path, master_key: &[u8; 32]) -> Result<Self> {
        // 每次启动都补齐完整结构，兼容全新数据库和旧版本升级。
        let write_txn = db
            .begin_write()
            .map_err(|e| DomainError::Storage(format!("启动写事务失败：{e}")))?;
        repos::ensure_schema(&write_txn)?;
        write_txn
            .commit()
            .map_err(|e| DomainError::Storage(format!("提交事务失败：{e}")))?;

        let db = Arc::new(db);
        let cipher = Arc::new(RwLock::new(Cipher::new(master_key)));

        // 首启迁移：为存量历史构建时间 / 去重索引（空库或已建则瞬时返回）
        repos::clip_repo::migrate_indexes(db.clone(), cipher.clone())?;
        let _ = repos::api_workspace_repo::list(db.clone(), cipher.clone())?;
        let _ = repos::collaboration_repo::list(db.clone(), cipher.clone())?;
        let _ = repos::connection_repo::list(db.clone(), cipher.clone())?;
        let _ = repos::kafka_cluster_repo::list(db.clone(), cipher.clone())?;
        let _ = repos::mqtt_profile_repo::list(db.clone(), cipher.clone())?;
        let _ = repos::ssh_profile_repo::list(db.clone(), cipher.clone())?;
        let _ = repos::object_storage_account_repo::list(db.clone(), cipher.clone())?;
        repos::clip_repo::validate_key(db.clone(), cipher.clone())?;
        repos::clip_repo::initialize_search_index(db.clone(), cipher.clone())?;

        info!(operation = "storage_open", path = %path.display(), "redb storage opened");

        Ok(Self {
            db,
            cipher,
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn default_db_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("com", "ramag", "ramag")
        .ok_or_else(|| DomainError::Storage("无法定位用户目录".into()))?;
    Ok(dirs.data_dir().join("ramag.redb"))
}

fn open_database(path: &Path) -> Result<Database> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DomainError::Storage(format!("创建数据目录失败：{e}")))?;
        reject_symlink(parent, "数据目录")?;
    }
    reject_symlink(path, "数据库文件")?;
    let database = Database::create(path)
        .map_err(|e| DomainError::Storage(format!("打开 redb 数据库失败：{e}")))?;
    set_private_file_permissions(path)
        .map_err(|e| DomainError::Storage(format!("收紧 redb 数据库权限失败：{e}")))?;
    Ok(database)
}

fn reject_symlink(path: &Path, label: &str) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(DomainError::Storage(format!(
            "{label}不能是符号链接：{}",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(DomainError::Storage(format!(
            "检查{label}失败 {}：{error}",
            path.display()
        ))),
    }
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn validate_clip_search_query(query: &str) -> Result<()> {
    if query.len() > MAX_CLIPBOARD_SEARCH_BYTES {
        return Err(DomainError::InvalidConfig(format!(
            "剪贴历史搜索词超过 {MAX_CLIPBOARD_SEARCH_BYTES} bytes 上限"
        )));
    }
    Ok(())
}

include!("storage_impl.rs");

#[cfg(test)]
mod tests;
