use std::sync::Arc;

use parking_lot::RwLock;
use redb::{Database, ReadableDatabase as _, ReadableTable, TableDefinition};
use tracing::{debug, info};

use ramag_domain::entities::{
    ApiHistoryRecord, ApiWorkspaceId, MAX_API_HISTORY, MAX_API_HISTORY_LIST_BYTES,
};
use ramag_domain::error::{DomainError, Result};

use crate::encryption::Cipher;
use crate::repos::bounded_json;

const MAX_API_HISTORY_RECORD_BYTES: usize = 128 * 1024;

pub(crate) const API_HISTORY_TABLE: TableDefinition<&str, &str> =
    TableDefinition::new("api_history");

fn key(workspace_id: &ApiWorkspaceId, record: &ApiHistoryRecord) -> String {
    format!("{workspace_id}:{}", record.id)
}

fn prefix(workspace_id: &ApiWorkspaceId) -> String {
    format!("{workspace_id}:")
}

fn encode(record: &ApiHistoryRecord, cipher: &Cipher) -> Result<String> {
    record.validate().map_err(DomainError::InvalidConfig)?;
    let json = bounded_json::serialize(record, MAX_API_HISTORY_RECORD_BYTES, "API 历史")?;
    cipher.encrypt(&json)
}

fn decode(key: &str, value: &str, cipher: &Cipher) -> Result<ApiHistoryRecord> {
    bounded_json::ensure_len(
        value.len(),
        MAX_API_HISTORY_RECORD_BYTES * 2 + 64,
        &format!("API 历史 {key}"),
    )?;
    let json = cipher
        .decrypt(value)
        .map_err(|error| DomainError::Storage(format!("解密 API 历史失败：{error}")))?;
    let record: ApiHistoryRecord = serde_json::from_str(&json)
        .map_err(|error| DomainError::Storage(format!("反序列化 API 历史失败：{error}")))?;
    record
        .validate()
        .map_err(|error| DomainError::Storage(format!("API 历史无效：{error}")))?;
    Ok(record)
}

pub(crate) fn append(
    db: Arc<Database>,
    cipher: Arc<RwLock<Cipher>>,
    workspace_id: ApiWorkspaceId,
    record: ApiHistoryRecord,
) -> Result<()> {
    let value = encode(&record, &cipher.read())?;
    let record_key = key(&workspace_id, &record);
    let record_prefix = prefix(&workspace_id);
    let write_txn = db
        .begin_write()
        .map_err(|error| DomainError::Storage(format!("启动 API 历史写事务失败：{error}")))?;
    let mut remove_keys = Vec::new();
    {
        let mut table = write_txn
            .open_table(API_HISTORY_TABLE)
            .map_err(|error| DomainError::Storage(format!("打开 API 历史表失败：{error}")))?;
        let mut entries = Vec::new();
        for entry in table
            .iter()
            .map_err(|error| DomainError::Storage(format!("遍历 API 历史失败：{error}")))?
        {
            let (stored_key, stored_value) = entry
                .map_err(|error| DomainError::Storage(format!("读取 API 历史失败：{error}")))?;
            if stored_key.value().starts_with(&record_prefix) {
                let history = decode(stored_key.value(), stored_value.value(), &cipher.read())?;
                entries.push((
                    stored_key.value().to_string(),
                    stored_value.value().len(),
                    history.created_at,
                ));
            }
        }
        entries.retain(|entry| entry.0 != record_key);
        entries.push((record_key.clone(), value.len(), record.created_at));
        entries.sort_by_key(|entry| entry.2);
        let mut total_bytes = entries.iter().map(|entry| entry.1).sum::<usize>();
        while entries.len() > MAX_API_HISTORY || total_bytes > MAX_API_HISTORY_LIST_BYTES {
            let removed = entries.remove(0);
            total_bytes = total_bytes.saturating_sub(removed.1);
            remove_keys.push(removed.0);
        }
        for remove_key in &remove_keys {
            table
                .remove(remove_key.as_str())
                .map_err(|error| DomainError::Storage(format!("清理 API 历史失败：{error}")))?;
        }
        table
            .insert(record_key.as_str(), value.as_str())
            .map_err(|error| DomainError::Storage(format!("写入 API 历史失败：{error}")))?;
    }
    write_txn
        .commit()
        .map_err(|error| DomainError::Storage(format!("提交 API 历史失败：{error}")))?;
    info!(
        operation = "api_history_append",
        workspace_id = %workspace_id,
        removed = remove_keys.len(),
        "api history appended"
    );
    Ok(())
}

pub(crate) fn list(
    db: Arc<Database>,
    cipher: Arc<RwLock<Cipher>>,
    workspace_id: ApiWorkspaceId,
    limit: usize,
) -> Result<Vec<ApiHistoryRecord>> {
    let limit = limit.min(MAX_API_HISTORY);
    if limit == 0 {
        return Ok(Vec::new());
    }
    let read_txn = db
        .begin_read()
        .map_err(|error| DomainError::Storage(format!("启动 API 历史读事务失败：{error}")))?;
    let table = read_txn
        .open_table(API_HISTORY_TABLE)
        .map_err(|error| DomainError::Storage(format!("打开 API 历史表失败：{error}")))?;
    let record_prefix = prefix(&workspace_id);
    let cipher = cipher.read();
    let mut records = Vec::new();
    let mut retained_bytes = 0usize;
    for entry in table
        .iter()
        .map_err(|error| DomainError::Storage(format!("遍历 API 历史失败：{error}")))?
    {
        let (stored_key, stored_value) =
            entry.map_err(|error| DomainError::Storage(format!("读取 API 历史失败：{error}")))?;
        if !stored_key.value().starts_with(&record_prefix) {
            continue;
        }
        let (_, next_bytes) = bounded_json::next_collection_budget(
            records.len(),
            retained_bytes,
            stored_value.value().len(),
            MAX_API_HISTORY,
            MAX_API_HISTORY_LIST_BYTES,
            "API 历史列表",
        )?;
        retained_bytes = next_bytes;
        records.push(decode(stored_key.value(), stored_value.value(), &cipher)?);
    }
    records.sort_by_key(|record| std::cmp::Reverse(record.created_at));
    records.truncate(limit);
    debug!(
        operation = "api_history_list",
        workspace_id = %workspace_id,
        count = records.len(),
        "api history listed"
    );
    Ok(records)
}

pub(crate) fn clear(db: Arc<Database>, workspace_id: ApiWorkspaceId) -> Result<()> {
    let record_prefix = prefix(&workspace_id);
    let write_txn = db
        .begin_write()
        .map_err(|error| DomainError::Storage(format!("启动 API 历史清理事务失败：{error}")))?;
    {
        let mut table = write_txn
            .open_table(API_HISTORY_TABLE)
            .map_err(|error| DomainError::Storage(format!("打开 API 历史表失败：{error}")))?;
        let keys = table
            .iter()
            .map_err(|error| DomainError::Storage(format!("遍历 API 历史失败：{error}")))?
            .filter_map(|entry| entry.ok())
            .filter(|(stored_key, _)| stored_key.value().starts_with(&record_prefix))
            .map(|(stored_key, _)| stored_key.value().to_string())
            .collect::<Vec<_>>();
        for key in keys {
            table
                .remove(key.as_str())
                .map_err(|error| DomainError::Storage(format!("删除 API 历史失败：{error}")))?;
        }
    }
    write_txn
        .commit()
        .map_err(|error| DomainError::Storage(format!("提交 API 历史清理失败：{error}")))?;
    Ok(())
}

pub(crate) fn ensure_table(write_txn: &redb::WriteTransaction) -> Result<()> {
    let _ = write_txn
        .open_table(API_HISTORY_TABLE)
        .map_err(|error| DomainError::Storage(format!("打开 API 历史表失败：{error}")))?;
    Ok(())
}
