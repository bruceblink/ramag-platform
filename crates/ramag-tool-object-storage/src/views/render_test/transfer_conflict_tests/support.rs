use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Waker;

use async_trait::async_trait;
use gpui_kit::{Entity, TestAppContext, VisualTestContext};
use ramag_app::{ObjectStorageService, ObjectStorageTransferTarget, configured_mounts};
use ramag_domain::entities::{
    CloudProvider, ConnectionConfig, ConnectionId, ManualBucket, ObjectCapabilities,
    ObjectDownloadRequest, ObjectListCursor, ObjectListQuery, ObjectMetadata, ObjectPage,
    ObjectStorageAccount, ObjectStorageAccountId, ObjectStorageAccountSnapshot, ObjectStorageMount,
    ObjectTextPreview, ObjectUploadRequest, OverwritePolicy, QueryRecord, QueryRecordId,
    SecretString,
};
use ramag_domain::error::{
    ObjectStorageError, ObjectStorageErrorCategory, ObjectStorageResult, Result,
};
use ramag_domain::traits::{ObjectStorageDriver, Storage};

use super::super::super::ObjectStorageView;
use super::super::add_workspace_window;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Call {
    pub account_id: ObjectStorageAccountId,
    pub bucket: String,
    pub key: String,
    pub path: PathBuf,
    pub overwrite: OverwritePolicy,
    pub upload: bool,
}

#[derive(Default)]
pub(super) struct Gate {
    released: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

impl Gate {
    pub fn release(&self) {
        self.released.store(true, Ordering::Release);
        if let Some(waker) = self.waker.lock().expect("gate waker lock").take() {
            waker.wake();
        }
    }

    async fn wait(&self) {
        std::future::poll_fn(|context| {
            if self.released.load(Ordering::Acquire) {
                std::task::Poll::Ready(())
            } else {
                *self.waker.lock().expect("gate waker lock") = Some(context.waker().clone());
                if self.released.load(Ordering::Acquire) {
                    std::task::Poll::Ready(())
                } else {
                    std::task::Poll::Pending
                }
            }
        })
        .await;
    }
}

pub(super) struct ControlledDriver {
    calls: Mutex<Vec<Call>>,
    conflict_refuse: bool,
    gate: Option<Arc<Gate>>,
}

impl ControlledDriver {
    pub fn new(conflict_refuse: bool, gate: Option<Arc<Gate>>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            conflict_refuse,
            gate,
        }
    }

    fn record(
        &self,
        account_id: ObjectStorageAccountId,
        mount: ObjectStorageMount,
        key: String,
        path: PathBuf,
        overwrite: OverwritePolicy,
        upload: bool,
    ) -> ObjectStorageResult<()> {
        self.calls.lock().expect("driver calls lock").push(Call {
            account_id,
            bucket: mount.bucket,
            key,
            path,
            overwrite,
            upload,
        });
        if self.conflict_refuse && overwrite == OverwritePolicy::Refuse {
            Err(ObjectStorageError::new(
                ObjectStorageErrorCategory::Conflict,
                if upload { "upload" } else { "download" },
                "test conflict",
            ))
        } else {
            Ok(())
        }
    }

    async fn record_transfer(
        &self,
        account_id: ObjectStorageAccountId,
        mount: ObjectStorageMount,
        key: String,
        path: PathBuf,
        overwrite: OverwritePolicy,
        upload: bool,
    ) -> ObjectStorageResult<()> {
        let result = self.record(account_id, mount, key, path, overwrite, upload);
        if overwrite == OverwritePolicy::Refuse
            && let Some(gate) = &self.gate
        {
            gate.wait().await;
        }
        result
    }

    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().expect("driver calls lock").clone()
    }
}

struct MemoryStorage {
    accounts: Mutex<Vec<ObjectStorageAccount>>,
    preferences: Mutex<HashMap<String, String>>,
}

impl MemoryStorage {
    fn new(accounts: Vec<ObjectStorageAccount>) -> Self {
        Self {
            accounts: Mutex::new(accounts),
            preferences: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl Storage for MemoryStorage {
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

    async fn list_object_storage_accounts(&self) -> Result<Vec<ObjectStorageAccount>> {
        Ok(self.accounts.lock().expect("accounts lock").clone())
    }

    async fn get_object_storage_account(
        &self,
        id: &ObjectStorageAccountId,
    ) -> Result<Option<ObjectStorageAccount>> {
        Ok(self
            .accounts
            .lock()
            .expect("accounts lock")
            .iter()
            .find(|account| &account.id == id)
            .cloned())
    }

    async fn save_object_storage_account(&self, account: &ObjectStorageAccount) -> Result<()> {
        let mut accounts = self.accounts.lock().expect("accounts lock");
        if let Some(existing) = accounts.iter_mut().find(|entry| entry.id == account.id) {
            *existing = account.clone();
        } else {
            accounts.push(account.clone());
        }
        Ok(())
    }

    async fn delete_object_storage_account(
        &self,
        id: &ObjectStorageAccountId,
        preference_key: &str,
    ) -> Result<()> {
        self.accounts
            .lock()
            .expect("accounts lock")
            .retain(|account| &account.id != id);
        self.preferences
            .lock()
            .expect("preferences lock")
            .remove(preference_key);
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

    async fn delete_history(&self, _id: &QueryRecordId) -> Result<()> {
        Ok(())
    }

    async fn clear_history(&self, _connection_id: Option<&ConnectionId>) -> Result<()> {
        Ok(())
    }

    async fn get_preference(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .preferences
            .lock()
            .expect("preferences lock")
            .get(key)
            .cloned())
    }

    async fn set_preference(&self, key: &str, value: &str) -> Result<()> {
        self.preferences
            .lock()
            .expect("preferences lock")
            .insert(key.into(), value.into());
        Ok(())
    }

    async fn seal(&self, plain: &[u8]) -> Result<Vec<u8>> {
        Ok(plain.to_vec())
    }

    async fn unseal(&self, cipher: &[u8]) -> Result<Vec<u8>> {
        Ok(cipher.to_vec())
    }
}

#[async_trait]
impl ObjectStorageDriver for ControlledDriver {
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
        _query: &ObjectListQuery,
        _cursor: Option<&ObjectListCursor>,
        _request_generation: u64,
    ) -> ObjectStorageResult<ObjectPage> {
        Ok(ObjectPage {
            entries: Vec::new(),
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
        Ok(ObjectMetadata {
            key: key.into(),
            size: 12,
            last_modified: None,
            etag: None,
            version: None,
            content_type: None,
            user_metadata: Vec::new(),
            storage_class: None,
        })
    }

    async fn read_text_preview(
        &self,
        _account: &ObjectStorageAccountSnapshot,
        _mount: &ObjectStorageMount,
        _key: &str,
    ) -> ObjectStorageResult<ObjectTextPreview> {
        Ok(ObjectTextPreview {
            content: String::new(),
            total_bytes: 0,
            truncated: false,
        })
    }

    async fn upload(&self, request: ObjectUploadRequest) -> ObjectStorageResult<()> {
        self.record_transfer(
            request.account.id,
            request.mount,
            request.key,
            request.local_path,
            request.overwrite,
            true,
        )
        .await
    }

    async fn download(&self, request: ObjectDownloadRequest) -> ObjectStorageResult<()> {
        self.record_transfer(
            request.account.id,
            request.mount,
            request.key,
            request.local_path,
            request.overwrite,
            false,
        )
        .await
    }

    async fn delete(
        &self,
        _account: &ObjectStorageAccountSnapshot,
        _mount: &ObjectStorageMount,
        _key: &str,
    ) -> ObjectStorageResult<()> {
        Ok(())
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

pub(super) fn accounts() -> Vec<ObjectStorageAccount> {
    ["account-a", "account-b"]
        .map(|name| {
            let mut account = ObjectStorageAccount::new(name, CloudProvider::AliyunOss);
            account.access_key_id = SecretString::new("test-access-key");
            account.access_key_secret = SecretString::new("test-secret-key");
            let mut first = ManualBucket::new(format!("{name}-bucket-a"), "cn-hangzhou");
            first.root_prefix = Some("team-a/".into());
            let mut second = ManualBucket::new(format!("{name}-bucket-b"), "cn-hangzhou");
            second.root_prefix = Some("team-b/".into());
            account.manual_buckets = vec![first, second];
            account
        })
        .into()
}

pub(super) fn service(
    accounts: Vec<ObjectStorageAccount>,
    driver: Arc<ControlledDriver>,
) -> Arc<ObjectStorageService> {
    Arc::new(ObjectStorageService::new(
        driver,
        Arc::new(MemoryStorage::new(accounts)),
    ))
}

pub(super) fn account_target(
    account: &ObjectStorageAccount,
    bucket_suffix: &str,
) -> ObjectStorageTransferTarget {
    let mount = configured_mounts(account)
        .expect("test account mounts should be valid")
        .into_iter()
        .find(|mount| mount.bucket.ends_with(bucket_suffix))
        .expect("test mount should exist");
    ObjectStorageTransferTarget::new(account, mount)
}

pub(super) fn setup_transfer<'a>(
    cx: &'a mut TestAppContext,
    accounts: &[ObjectStorageAccount],
    driver: Arc<ControlledDriver>,
) -> (
    Entity<ObjectStorageView>,
    &'a mut VisualTestContext,
    ObjectStorageTransferTarget,
) {
    let (view, visual) = add_workspace_window(cx, service(accounts.to_vec(), driver));
    let target = account_target(&accounts[0], "bucket-a");
    let open_ids = accounts.iter().map(|account| account.id.clone()).collect();
    view.update(visual, |view, cx| {
        view.accounts = Arc::new(accounts.to_vec());
        view.selected_account_id = Some(accounts[0].id.clone());
        view.open_account_ids = open_ids;
        view.management_visible = false;
        view.mounts = Arc::new(configured_mounts(&accounts[0]).expect("mounts"));
        view.selected_mount = Some(target.mount.clone());
        view.loading = false;
        cx.notify();
    });
    visual.run_until_parked();
    (view, visual, target)
}

pub(super) fn pending_id(
    view: &Entity<ObjectStorageView>,
    visual: &mut VisualTestContext,
    upload: bool,
) -> Option<u64> {
    visual.update(|_, app| {
        let view = view.read(app);
        if upload {
            view.pending_upload
                .as_ref()
                .map(|pending| pending.transfer_id)
        } else {
            view.pending_download
                .as_ref()
                .map(|pending| pending.transfer_id)
        }
    })
}

pub(super) fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let bounds = visual
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} should be present"));
    visual.simulate_click(bounds.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
}

pub(super) fn select_other_account(
    view: &Entity<ObjectStorageView>,
    visual: &mut VisualTestContext,
    accounts: &[ObjectStorageAccount],
) {
    let other = accounts[1].clone();
    visual.update(|window, app| {
        view.update(app, |view, cx| {
            view.select_account(other.id.clone(), window, cx)
        });
    });
    visual.run_until_parked();
}

pub(super) fn select_other_mount(
    view: &Entity<ObjectStorageView>,
    visual: &mut VisualTestContext,
    account: &ObjectStorageAccount,
) {
    let mount = account_target(account, "bucket-b").mount;
    visual.update(|window, app| {
        view.update(app, |view, cx| view.select_mount(mount.clone(), window, cx));
    });
    visual.run_until_parked();
}

pub(super) fn start_upload(
    view: &Entity<ObjectStorageView>,
    visual: &mut VisualTestContext,
    target: ObjectStorageTransferTarget,
) {
    visual.update(|window, app| {
        view.update(app, |view, cx| {
            view.run_upload_to(
                target,
                PathBuf::from("test-upload.txt"),
                "team-a/test.txt".into(),
                OverwritePolicy::Refuse,
                window,
                cx,
            );
        });
    });
    visual.run_until_parked();
}

pub(super) fn start_download(
    view: &Entity<ObjectStorageView>,
    visual: &mut VisualTestContext,
    target: ObjectStorageTransferTarget,
) {
    visual.update(|window, app| {
        view.update(app, |view, cx| {
            view.run_download_to(
                target,
                PathBuf::from("existing-local.txt"),
                "team-a/test.txt".into(),
                OverwritePolicy::Refuse,
                window,
                cx,
            );
        });
    });
    visual.run_until_parked();
}

pub(super) fn expect_target(
    call: &Call,
    target: &ObjectStorageTransferTarget,
    upload: bool,
    policy: OverwritePolicy,
) {
    assert_eq!(call.account_id, target.account_id);
    assert_eq!(call.bucket, target.mount.bucket);
    assert_eq!(call.overwrite, policy);
    assert_eq!(call.upload, upload);
}
