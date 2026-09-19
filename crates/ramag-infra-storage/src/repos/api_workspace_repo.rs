//! API 测试工作区 CRUD。整个工作区经主密钥加密，避免请求凭据和环境变量明文落盘。

use std::sync::Arc;

use parking_lot::RwLock;
use redb::{Database, ReadableDatabase as _, ReadableTable, TableDefinition};
use tracing::{debug, info};

use ramag_domain::entities::{ApiWorkspace, MAX_API_WORKSPACES};
use ramag_domain::error::{DomainError, Result};

use crate::encryption::Cipher;
use crate::repos::bounded_json;

const MAX_API_WORKSPACE_RECORD_BYTES: usize = 64 * 1024 * 1024;
const MAX_API_WORKSPACE_LIST_BYTES: usize = 128 * 1024 * 1024;

/// 键为 ApiWorkspaceId UUID，值为加密后的 ApiWorkspace JSON。
pub(crate) const API_WORKSPACES_TABLE: TableDefinition<&str, &str> =
    TableDefinition::new("api_workspaces");

fn encode_workspace(workspace: &ApiWorkspace, cipher: &Cipher) -> Result<String> {
    workspace.validate().map_err(DomainError::InvalidConfig)?;
    let json = bounded_json::serialize(workspace, MAX_API_WORKSPACE_RECORD_BYTES, "API Workspace")?;
    cipher.encrypt(&json)
}

fn decode_workspace(key: &str, value: &str, cipher: &Cipher) -> Result<ApiWorkspace> {
    bounded_json::ensure_len(
        value.len(),
        MAX_API_WORKSPACE_RECORD_BYTES * 2 + 64,
        &format!("API Workspace {key}"),
    )?;
    let json = cipher
        .decrypt(value)
        .map_err(|error| DomainError::Storage(format!("解密 API Workspace {key} 失败：{error}")))?;
    let workspace: ApiWorkspace = serde_json::from_str(&json).map_err(|error| {
        DomainError::Storage(format!("反序列化 API Workspace {key} 失败：{error}"))
    })?;
    workspace.validate().map_err(|error| {
        DomainError::Storage(format!("解密后的 API Workspace {key} 无效：{error}"))
    })?;
    if workspace.id.to_string() != key {
        return Err(DomainError::Storage(format!(
            "API Workspace 键与内容 ID 不一致：{key}"
        )));
    }
    Ok(workspace)
}

/// 读取并按名称排序所有工作区，同时限制记录数量和加密值总大小。
pub(crate) fn list(db: Arc<Database>, cipher: Arc<RwLock<Cipher>>) -> Result<Vec<ApiWorkspace>> {
    let read_txn = db
        .begin_read()
        .map_err(|error| DomainError::Storage(format!("启动读事务失败：{error}")))?;
    let table = read_txn
        .open_table(API_WORKSPACES_TABLE)
        .map_err(|error| DomainError::Storage(format!("打开 api_workspaces 表失败：{error}")))?;
    let cipher = cipher.read();
    let mut workspaces = Vec::new();
    let mut retained_bytes = 0usize;
    for entry in table
        .iter()
        .map_err(|error| DomainError::Storage(format!("遍历 API Workspace 失败：{error}")))?
    {
        let (key, value) = entry
            .map_err(|error| DomainError::Storage(format!("读取 API Workspace 失败：{error}")))?;
        let (_, next_bytes) = bounded_json::next_collection_budget(
            workspaces.len(),
            retained_bytes,
            value.value().len(),
            MAX_API_WORKSPACES,
            MAX_API_WORKSPACE_LIST_BYTES,
            "API Workspace 列表",
        )?;
        retained_bytes = next_bytes;
        workspaces.push(decode_workspace(key.value(), value.value(), &cipher)?);
    }
    workspaces.sort_by(|left, right| left.name.cmp(&right.name));
    debug!(
        operation = "api_workspace_list",
        count = workspaces.len(),
        "api workspace listing completed"
    );
    Ok(workspaces)
}

pub(crate) fn get(
    db: Arc<Database>,
    cipher: Arc<RwLock<Cipher>>,
    id: String,
) -> Result<Option<ApiWorkspace>> {
    let read_txn = db
        .begin_read()
        .map_err(|error| DomainError::Storage(format!("启动读事务失败：{error}")))?;
    let table = read_txn
        .open_table(API_WORKSPACES_TABLE)
        .map_err(|error| DomainError::Storage(format!("打开 api_workspaces 表失败：{error}")))?;
    let value = table
        .get(id.as_str())
        .map_err(|error| DomainError::Storage(format!("读取 API Workspace {id} 失败：{error}")))?;
    value
        .map(|value| decode_workspace(&id, value.value(), &cipher.read()))
        .transpose()
}

pub(crate) fn save(
    db: Arc<Database>,
    cipher: Arc<RwLock<Cipher>>,
    workspace: ApiWorkspace,
) -> Result<()> {
    let value = encode_workspace(&workspace, &cipher.read())?;
    let id = workspace.id.to_string();
    let write_txn = db
        .begin_write()
        .map_err(|error| DomainError::Storage(format!("启动写事务失败：{error}")))?;
    {
        let mut table = write_txn
            .open_table(API_WORKSPACES_TABLE)
            .map_err(|error| {
                DomainError::Storage(format!("打开 api_workspaces 表失败：{error}"))
            })?;
        let mut count = 0usize;
        let mut total_bytes = 0usize;
        let mut replaced_bytes = None;
        for entry in table
            .iter()
            .map_err(|error| DomainError::Storage(format!("遍历 API Workspace 失败：{error}")))?
        {
            let (key, existing) = entry.map_err(|error| {
                DomainError::Storage(format!("读取 API Workspace 失败：{error}"))
            })?;
            (count, total_bytes) = bounded_json::next_collection_budget(
                count,
                total_bytes,
                existing.value().len(),
                MAX_API_WORKSPACES,
                MAX_API_WORKSPACE_LIST_BYTES,
                "API Workspace 列表",
            )?;
            if key.value() == id {
                replaced_bytes = Some(existing.value().len());
            }
        }
        let final_count = count + usize::from(replaced_bytes.is_none());
        let final_bytes = total_bytes
            .checked_sub(replaced_bytes.unwrap_or(0))
            .and_then(|bytes| bytes.checked_add(value.len()))
            .ok_or_else(|| DomainError::Storage("API Workspace 列表总大小溢出".into()))?;
        bounded_json::ensure_collection_budget(
            final_count,
            final_bytes,
            MAX_API_WORKSPACES,
            MAX_API_WORKSPACE_LIST_BYTES,
            "API Workspace 列表",
        )?;
        table.insert(id.as_str(), value.as_str()).map_err(|error| {
            DomainError::Storage(format!("写入 API Workspace {id} 失败：{error}"))
        })?;
    }
    write_txn
        .commit()
        .map_err(|error| DomainError::Storage(format!("提交事务失败：{error}")))?;
    info!(operation = "api_workspace_save", workspace_id = %id, "api workspace saved");
    Ok(())
}

pub(crate) fn delete(db: Arc<Database>, id: String) -> Result<()> {
    let write_txn = db
        .begin_write()
        .map_err(|error| DomainError::Storage(format!("启动写事务失败：{error}")))?;
    {
        let mut table = write_txn
            .open_table(API_WORKSPACES_TABLE)
            .map_err(|error| {
                DomainError::Storage(format!("打开 api_workspaces 表失败：{error}"))
            })?;
        table.remove(id.as_str()).map_err(|error| {
            DomainError::Storage(format!("删除 API Workspace {id} 失败：{error}"))
        })?;
    }
    write_txn
        .commit()
        .map_err(|error| DomainError::Storage(format!("提交事务失败：{error}")))?;
    info!(operation = "api_workspace_delete", workspace_id = %id, "api workspace deleted");
    Ok(())
}

pub(crate) fn ensure_table(write_txn: &redb::WriteTransaction) -> Result<()> {
    let _ = write_txn
        .open_table(API_WORKSPACES_TABLE)
        .map_err(|error| DomainError::Storage(format!("打开 api_workspaces 表失败：{error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::{ApiCollection, ApiRequestRecord, HttpRequestSpec};

    fn sample_workspace() -> ApiWorkspace {
        let mut workspace = ApiWorkspace::new("local");
        let mut collection = ApiCollection::new("smoke");
        collection.requests.push(ApiRequestRecord::new_http(
            "health",
            HttpRequestSpec::new("GET", "{{base_url}}/health"),
        ));
        workspace.collections.push(collection);
        workspace
    }

    #[test]
    fn encrypted_workspace_does_not_contain_request_fields() {
        let cipher = Cipher::new(&[8; 32]);
        let workspace = sample_workspace();
        let encoded = encode_workspace(&workspace, &cipher).unwrap();

        assert!(!encoded.contains("local"));
        assert!(!encoded.contains("base_url"));
        assert!(!encoded.contains("health"));
        assert_eq!(
            decode_workspace(&workspace.id.to_string(), &encoded, &cipher).unwrap(),
            workspace
        );
    }
}
