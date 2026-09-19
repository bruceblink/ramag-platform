use redb::{Database, ReadableDatabase as _, ReadableTableMetadata as _, TableError};

use ramag_domain::error::{DomainError, Result};

use crate::repos;

/// 任一加密业务表存在记录时都必须复用原主密钥，不能静默创建新密钥。
pub(crate) fn has_encrypted_records(db: &Database) -> Result<bool> {
    let read_txn = db
        .begin_read()
        .map_err(|e| DomainError::Storage(format!("检查加密数据失败：{e}")))?;
    for definition in [
        repos::api_workspace_repo::API_WORKSPACES_TABLE,
        repos::api_history_repo::API_HISTORY_TABLE,
        repos::connection_repo::CONNECTIONS_TABLE,
        repos::clip_repo::CLIPS_TABLE,
        repos::ssh_profile_repo::SSH_PROFILES_TABLE,
        repos::object_storage_account_repo::OBJECT_STORAGE_ACCOUNTS_TABLE,
        repos::kafka_cluster_repo::KAFKA_CLUSTERS_TABLE,
        repos::mqtt_profile_repo::MQTT_PROFILES_TABLE,
    ] {
        match read_txn.open_table(definition) {
            Ok(table)
                if !table
                    .is_empty()
                    .map_err(|e| DomainError::Storage(format!("检查加密数据表失败：{e}")))? =>
            {
                return Ok(true);
            }
            Ok(_) | Err(TableError::TableDoesNotExist(_)) => {}
            Err(error) => {
                return Err(DomainError::Storage(format!("打开加密数据表失败：{error}")));
            }
        }
    }
    Ok(false)
}
