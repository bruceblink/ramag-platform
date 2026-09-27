//! 本机协作共享包的加密 redb 表；只保存经过领域校验的共享包。

use std::sync::Arc;

use parking_lot::RwLock;
use redb::{Database, ReadableDatabase as _, ReadableTable, TableDefinition};
use tracing::{debug, info};

use ramag_domain::entities::{
    CollaborationShare, CollaborationShareId, MAX_COLLABORATION_PAYLOAD_BYTES,
    MAX_COLLABORATION_SHARES,
};
use ramag_domain::error::{DomainError, Result};

use crate::encryption::Cipher;
use crate::repos::bounded_json;

const MAX_SHARE_RECORD_BYTES: usize = MAX_COLLABORATION_PAYLOAD_BYTES + 512 * 1024;
const MAX_SHARE_LIST_BYTES: usize = 128 * 1024 * 1024;

/// 键为共享包 UUID，值为 AES-GCM hex 密文。
pub(crate) const COLLABORATION_SHARES_TABLE: TableDefinition<&str, &str> =
    TableDefinition::new("collaboration_shares");

fn encode_share(share: &CollaborationShare, cipher: &Cipher) -> Result<String> {
    share
        .validate()
        .map_err(|error| DomainError::InvalidConfig(error.to_string()))?;
    let json = bounded_json::serialize(share, MAX_SHARE_RECORD_BYTES, "协作共享包")?;
    cipher.encrypt(&json)
}

fn decode_share(key: &str, value: &str, cipher: &Cipher) -> Result<CollaborationShare> {
    bounded_json::ensure_len(
        value.len(),
        MAX_SHARE_RECORD_BYTES * 2 + 64,
        &format!("协作共享包 {key}"),
    )?;
    let json = cipher
        .decrypt(value)
        .map_err(|error| DomainError::Storage(format!("解密协作共享包 {key} 失败：{error}")))?;
    let share: CollaborationShare = serde_json::from_str(&json)
        .map_err(|error| DomainError::Storage(format!("反序列化协作共享包 {key} 失败：{error}")))?;
    share
        .validate()
        .map_err(|error| DomainError::Storage(format!("解密后的协作共享包 {key} 无效：{error}")))?;
    if share.id.to_string() != key {
        return Err(DomainError::Storage(format!(
            "协作共享包键与内容 ID 不一致：{key}"
        )));
    }
    Ok(share)
}

pub(crate) fn list(
    db: Arc<Database>,
    cipher: Arc<RwLock<Cipher>>,
) -> Result<Vec<CollaborationShare>> {
    let read_txn = db
        .begin_read()
        .map_err(|error| DomainError::Storage(format!("启动读事务失败：{error}")))?;
    let table = read_txn
        .open_table(COLLABORATION_SHARES_TABLE)
        .map_err(|error| DomainError::Storage(format!("打开协作共享包表失败：{error}")))?;
    let cipher = cipher.read();
    let mut shares = Vec::new();
    let mut retained_bytes = 0usize;
    for entry in table
        .iter()
        .map_err(|error| DomainError::Storage(format!("遍历协作共享包失败：{error}")))?
    {
        let (key, value) =
            entry.map_err(|error| DomainError::Storage(format!("读取协作共享包失败：{error}")))?;
        let (_, next_bytes) = bounded_json::next_collection_budget(
            shares.len(),
            retained_bytes,
            value.value().len(),
            MAX_COLLABORATION_SHARES,
            MAX_SHARE_LIST_BYTES,
            "协作共享包列表",
        )?;
        retained_bytes = next_bytes;
        shares.push(decode_share(key.value(), value.value(), &cipher)?);
    }
    shares.sort_by_key(|share| std::cmp::Reverse(share.updated_at));
    debug!(
        operation = "collaboration_share_list",
        count = shares.len(),
        "collaboration shares loaded"
    );
    Ok(shares)
}

pub(crate) fn get(
    db: Arc<Database>,
    cipher: Arc<RwLock<Cipher>>,
    id: CollaborationShareId,
) -> Result<Option<CollaborationShare>> {
    let id = id.to_string();
    let read_txn = db
        .begin_read()
        .map_err(|error| DomainError::Storage(format!("启动读事务失败：{error}")))?;
    let table = read_txn
        .open_table(COLLABORATION_SHARES_TABLE)
        .map_err(|error| DomainError::Storage(format!("打开协作共享包表失败：{error}")))?;
    let value = table
        .get(id.as_str())
        .map_err(|error| DomainError::Storage(format!("读取协作共享包 {id} 失败：{error}")))?;
    value
        .map(|value| decode_share(&id, value.value(), &cipher.read()))
        .transpose()
}

pub(crate) fn save(
    db: Arc<Database>,
    cipher: Arc<RwLock<Cipher>>,
    share: CollaborationShare,
) -> Result<()> {
    let value = encode_share(&share, &cipher.read())?;
    let id = share.id.to_string();
    let write_txn = db
        .begin_write()
        .map_err(|error| DomainError::Storage(format!("启动写事务失败：{error}")))?;
    {
        let mut table = write_txn
            .open_table(COLLABORATION_SHARES_TABLE)
            .map_err(|error| DomainError::Storage(format!("打开协作共享包表失败：{error}")))?;
        let mut count = 0usize;
        let mut total_bytes = 0usize;
        let mut replaced_bytes = None;
        for entry in table
            .iter()
            .map_err(|error| DomainError::Storage(format!("遍历协作共享包失败：{error}")))?
        {
            let (key, existing) = entry
                .map_err(|error| DomainError::Storage(format!("读取协作共享包失败：{error}")))?;
            (count, total_bytes) = bounded_json::next_collection_budget(
                count,
                total_bytes,
                existing.value().len(),
                MAX_COLLABORATION_SHARES,
                MAX_SHARE_LIST_BYTES,
                "协作共享包列表",
            )?;
            if key.value() == id {
                replaced_bytes = Some(existing.value().len());
            }
        }
        let final_count = count + usize::from(replaced_bytes.is_none());
        let final_bytes = total_bytes
            .checked_sub(replaced_bytes.unwrap_or(0))
            .and_then(|bytes| bytes.checked_add(value.len()))
            .ok_or_else(|| DomainError::Storage("协作共享包列表总大小溢出".into()))?;
        bounded_json::ensure_collection_budget(
            final_count,
            final_bytes,
            MAX_COLLABORATION_SHARES,
            MAX_SHARE_LIST_BYTES,
            "协作共享包列表",
        )?;
        table
            .insert(id.as_str(), value.as_str())
            .map_err(|error| DomainError::Storage(format!("写入协作共享包 {id} 失败：{error}")))?;
    }
    write_txn
        .commit()
        .map_err(|error| DomainError::Storage(format!("提交协作共享包失败：{error}")))?;
    info!(operation = "collaboration_share_save", share_id = %id, "collaboration share saved");
    Ok(())
}

pub(crate) fn delete(db: Arc<Database>, id: CollaborationShareId) -> Result<()> {
    let id = id.to_string();
    let write_txn = db
        .begin_write()
        .map_err(|error| DomainError::Storage(format!("启动写事务失败：{error}")))?;
    {
        let mut table = write_txn
            .open_table(COLLABORATION_SHARES_TABLE)
            .map_err(|error| DomainError::Storage(format!("打开协作共享包表失败：{error}")))?;
        table
            .remove(id.as_str())
            .map_err(|error| DomainError::Storage(format!("删除协作共享包 {id} 失败：{error}")))?;
    }
    write_txn
        .commit()
        .map_err(|error| DomainError::Storage(format!("提交协作共享包删除失败：{error}")))?;
    info!(operation = "collaboration_share_delete", share_id = %id, "collaboration share deleted");
    Ok(())
}

pub(crate) fn ensure_table(write_txn: &redb::WriteTransaction) -> Result<()> {
    write_txn
        .open_table(COLLABORATION_SHARES_TABLE)
        .map_err(|error| DomainError::Storage(format!("打开协作共享包表失败：{error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::CollaborationArtifact;

    #[test]
    fn encrypted_share_does_not_expose_plaintext() {
        let cipher = Cipher::new(&[5; 32]);
        let share = CollaborationShare::new_local(
            "本机文档",
            vec![CollaborationArtifact::document("说明", "private-content")],
        )
        .unwrap();
        let encoded = encode_share(&share, &cipher).unwrap();
        assert!(!encoded.contains("本机文档"));
        assert!(!encoded.contains("private-content"));
        assert_eq!(
            decode_share(&share.id.to_string(), &encoded, &cipher).unwrap(),
            share
        );
    }
}
