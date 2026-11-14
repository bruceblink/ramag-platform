//! 离线对象存储原生 UI 验收服务。

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::channel::oneshot;
use ramag_app::ObjectStorageService;
use ramag_domain::entities::{
    CloudProvider, ManualBucket, ObjectCapabilities, ObjectDownloadRequest, ObjectEntry,
    ObjectEntryKind, ObjectListCursor, ObjectListQuery, ObjectMetadata, ObjectPage,
    ObjectStorageAccount, ObjectStorageAccountId, ObjectStorageAccountSnapshot, ObjectStorageMount,
    ObjectStorageSessionPreference, ObjectTextPreview, ObjectTransferProgress, ObjectUploadRequest,
    OverwritePolicy, SecretString,
};
use ramag_domain::error::{ObjectStorageError, ObjectStorageErrorCategory, ObjectStorageResult};
use ramag_domain::traits::{ObjectStorageDriver, Storage};

const UPLOAD_KEY: &str = "ramag-object-ui-upload.txt";
const DOWNLOAD_KEY: &str = "ramag-object-ui-download.txt";

struct OfflineObjectStorage;

#[async_trait]
impl ObjectStorageDriver for OfflineObjectStorage {
    async fn capabilities(
        &self,
        _account: &ObjectStorageAccountSnapshot,
        _mount: &ObjectStorageMount,
    ) -> ObjectStorageResult<ObjectCapabilities> {
        Ok(ObjectCapabilities {
            stat: true,
            read: true,
            write: true,
            delete: true,
            list: true,
            atomic_create: true,
        })
    }

    async fn list_page(
        &self,
        _account: &ObjectStorageAccountSnapshot,
        _mount: &ObjectStorageMount,
        query: &ObjectListQuery,
        _cursor: Option<&ObjectListCursor>,
        _request_generation: u64,
    ) -> ObjectStorageResult<ObjectPage> {
        let entries = [UPLOAD_KEY, DOWNLOAD_KEY]
            .into_iter()
            .filter(|key| key.starts_with(query.list_prefix()))
            .map(|key| ObjectEntry {
                key: key.into(),
                display_name: key.into(),
                kind: ObjectEntryKind::Object,
                operable: true,
                size: Some(content(key).len() as u64),
                last_modified: None,
                etag: None,
                content_type: Some("text/plain".into()),
                storage_class: None,
            })
            .collect();
        Ok(ObjectPage {
            entries,
            next_cursor: None,
            capped: false,
        })
    }

    async fn stat(
        &self,
        _account: &ObjectStorageAccountSnapshot,
        _mount: &ObjectStorageMount,
        key: &str,
    ) -> ObjectStorageResult<ObjectMetadata> {
        if key != UPLOAD_KEY && key != DOWNLOAD_KEY {
            return Err(not_found("stat"));
        }
        Ok(metadata(key))
    }

    async fn read_text_preview(
        &self,
        _account: &ObjectStorageAccountSnapshot,
        _mount: &ObjectStorageMount,
        key: &str,
    ) -> ObjectStorageResult<ObjectTextPreview> {
        if key != UPLOAD_KEY && key != DOWNLOAD_KEY {
            return Err(not_found("preview"));
        }
        let text = content(key);
        Ok(ObjectTextPreview {
            content: text.into(),
            total_bytes: text.len() as u64,
            truncated: false,
        })
    }

    async fn upload(&self, request: ObjectUploadRequest) -> ObjectStorageResult<()> {
        tracing::info!(
            operation = "object_storage_offline_preview_transfer",
            account_id = %request.account.id,
            bucket = %request.mount.bucket,
            key = %request.key,
            overwrite = ?request.overwrite,
            direction = "upload",
            "offline preview transfer; no network or file I/O"
        );
        if request.cancellation.is_cancelled() {
            return Err(cancelled("upload"));
        }
        if request.key != UPLOAD_KEY {
            return Err(not_found("upload"));
        }
        if request.overwrite == OverwritePolicy::Refuse {
            delay_for_native_switch().await?;
            if request.cancellation.is_cancelled() {
                return Err(cancelled("upload"));
            }
            return Err(conflict("upload"));
        }
        let bytes = content(UPLOAD_KEY).len() as u64;
        (request.progress)(ObjectTransferProgress {
            transferred: bytes,
            total: bytes,
        });
        Ok(())
    }

    async fn download(&self, request: ObjectDownloadRequest) -> ObjectStorageResult<()> {
        tracing::info!(
            operation = "object_storage_offline_preview_transfer",
            account_id = %request.account.id,
            bucket = %request.mount.bucket,
            key = %request.key,
            overwrite = ?request.overwrite,
            direction = "download",
            "offline preview transfer; no network or file I/O"
        );
        if request.cancellation.is_cancelled() {
            return Err(cancelled("download"));
        }
        if request.key != DOWNLOAD_KEY {
            return Err(not_found("download"));
        }
        if request.overwrite == OverwritePolicy::Refuse {
            delay_for_native_switch().await?;
            if request.cancellation.is_cancelled() {
                return Err(cancelled("download"));
            }
            return Err(conflict("download"));
        }
        let bytes = content(DOWNLOAD_KEY).len() as u64;
        (request.progress)(ObjectTransferProgress {
            transferred: bytes,
            total: bytes,
        });
        Ok(())
    }

    async fn delete(
        &self,
        _account: &ObjectStorageAccountSnapshot,
        _mount: &ObjectStorageMount,
        key: &str,
    ) -> ObjectStorageResult<()> {
        if key == UPLOAD_KEY || key == DOWNLOAD_KEY {
            Ok(())
        } else {
            Err(not_found("delete"))
        }
    }

    async fn invalidate_account(
        &self,
        _account_id: &ObjectStorageAccountId,
        _minimum_revision: u64,
    ) -> ObjectStorageResult<()> {
        Ok(())
    }

    async fn shutdown(&self) -> ObjectStorageResult<()> {
        Ok(())
    }
}

/// 延迟首次拒绝响应，给原生窗口时间切换目标；等待在线程中完成，不阻塞 GPUI。
async fn delay_for_native_switch() -> ObjectStorageResult<()> {
    let (sender, receiver) = oneshot::channel();
    std::thread::Builder::new()
        .name("ramag-object-preview-delay".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            let _ = sender.send(());
        })
        .map_err(|_| failure("preview_delay", "无法启动离线验收延迟线程"))?;
    receiver
        .await
        .map_err(|_| failure("preview_delay", "离线验收延迟线程未能返回结果"))
}

pub(super) fn service(storage: Arc<dyn Storage>) -> anyhow::Result<Arc<ObjectStorageService>> {
    let service = Arc::new(ObjectStorageService::new(
        Arc::new(OfflineObjectStorage),
        storage,
    ));
    let accounts = [
        preview_account("离线验收 A", &["bucket-a", "bucket-b"]),
        preview_account("离线验收 B", &["bucket-b"]),
    ];
    futures::executor::block_on(async {
        let mut ids = Vec::with_capacity(accounts.len());
        for account in accounts {
            ids.push(service.save_account(account).await?.account.id);
        }
        service
            .save_session_preference(&ObjectStorageSessionPreference {
                open_account_ids: ids.clone(),
                active_account_id: ids.first().cloned(),
            })
            .await?;
        Ok::<_, anyhow::Error>(())
    })?;
    Ok(service)
}

fn preview_account(name: &str, buckets: &[&str]) -> ObjectStorageAccount {
    let mut account = ObjectStorageAccount::new(name, CloudProvider::AliyunOss);
    account.access_key_id = SecretString::new("preview-key-id");
    account.access_key_secret = SecretString::new("preview-key-secret");
    account.manual_buckets = buckets
        .iter()
        .map(|bucket| ManualBucket::new(*bucket, "cn-hangzhou"))
        .collect();
    account
}

fn content(key: &str) -> &'static str {
    match key {
        UPLOAD_KEY => "离线上传覆盖验收对象。\n",
        DOWNLOAD_KEY => "离线下载覆盖验收对象。\n",
        _ => "",
    }
}

fn metadata(key: &str) -> ObjectMetadata {
    ObjectMetadata {
        key: key.into(),
        size: content(key).len() as u64,
        last_modified: None,
        etag: None,
        version: None,
        content_type: Some("text/plain".into()),
        user_metadata: Vec::new(),
        storage_class: None,
    }
}

fn conflict(operation: &'static str) -> ObjectStorageError {
    ObjectStorageError::new(
        ObjectStorageErrorCategory::Conflict,
        operation,
        "离线验收对象已存在",
    )
}

fn failure(operation: &'static str, message: &'static str) -> ObjectStorageError {
    ObjectStorageError::new(ObjectStorageErrorCategory::Provider, operation, message)
}

fn not_found(operation: &'static str) -> ObjectStorageError {
    ObjectStorageError::new(
        ObjectStorageErrorCategory::NotFound,
        operation,
        "离线验收对象不存在",
    )
}

fn cancelled(operation: &'static str) -> ObjectStorageError {
    ObjectStorageError::new(
        ObjectStorageErrorCategory::Cancelled,
        operation,
        "离线对象操作已取消",
    )
}
