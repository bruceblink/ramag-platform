use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use ramag_domain::entities::{OverwritePolicy, TransferCancellation};

use super::*;
use crate::usecases::object_storage_service::configured_mounts;

fn transfer_target(
    account: &ramag_domain::entities::ObjectStorageAccount,
) -> ObjectStorageTransferTarget {
    let mount = configured_mounts(account)
        .expect("configured mount")
        .remove(0);
    ObjectStorageTransferTarget::new(account, mount)
}

fn progress() -> ramag_domain::entities::ObjectProgressFn {
    Arc::new(|_| {})
}

#[tokio::test]
async fn transfer_target_rejects_stale_account_revision_before_driver_call() {
    let (_directory, storage) = storage();
    let service = ObjectStorageService::new(Arc::new(FakeDriver::default()), storage.clone());
    let account = service
        .save_account(account())
        .await
        .expect("save account")
        .account;
    let target = transfer_target(&account);
    let driver = Arc::new(FakeDriver::default());
    let service = ObjectStorageService::new(driver.clone(), storage);
    let mut updated = account;
    updated.name = "updated account".into();
    service.save_account(updated).await.expect("update account");

    let result = service
        .upload_object(
            &target,
            "object.txt".into(),
            PathBuf::from("local.txt"),
            OverwritePolicy::Overwrite,
            TransferCancellation::default(),
            progress(),
        )
        .await;

    assert!(result.is_err());
    assert_eq!(driver.uploads.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn transfer_target_rejects_mount_not_in_current_account_configuration() {
    let (_directory, storage) = storage();
    let account = account();
    storage
        .save_object_storage_account(&account)
        .await
        .expect("seed account");
    let mut target = transfer_target(&account);
    target.mount.root_prefix = Some("changed/".into());
    let driver = Arc::new(FakeDriver::default());
    let service = ObjectStorageService::new(driver.clone(), storage);

    let result = service
        .download_object(
            &target,
            "object.txt".into(),
            PathBuf::from("local.txt"),
            OverwritePolicy::Overwrite,
            TransferCancellation::default(),
            progress(),
        )
        .await;

    assert!(result.is_err());
    assert_eq!(driver.downloads.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn transfer_target_rejects_deleted_account_before_driver_call() {
    let (_directory, storage) = storage();
    let account = account();
    storage
        .save_object_storage_account(&account)
        .await
        .expect("seed account");
    let target = transfer_target(&account);
    let driver = Arc::new(FakeDriver::default());
    let service = ObjectStorageService::new(driver.clone(), storage);
    service
        .delete_account(&account.id)
        .await
        .expect("delete account");

    let result = service
        .upload_object(
            &target,
            "object.txt".into(),
            PathBuf::from("local.txt"),
            OverwritePolicy::Overwrite,
            TransferCancellation::default(),
            progress(),
        )
        .await;

    assert!(result.is_err());
    assert_eq!(driver.uploads.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn current_transfer_target_allows_upload_and_download() {
    let (_directory, storage) = storage();
    let account = account();
    storage
        .save_object_storage_account(&account)
        .await
        .expect("seed account");
    let target = transfer_target(&account);
    let driver = Arc::new(FakeDriver::default());
    let service = ObjectStorageService::new(driver.clone(), storage);

    service
        .upload_object(
            &target,
            "upload.txt".into(),
            PathBuf::from("upload.txt"),
            OverwritePolicy::Refuse,
            TransferCancellation::default(),
            progress(),
        )
        .await
        .expect("current upload target");
    service
        .download_object(
            &target,
            "download.txt".into(),
            PathBuf::from("download.txt"),
            OverwritePolicy::Refuse,
            TransferCancellation::default(),
            progress(),
        )
        .await
        .expect("current download target");

    assert_eq!(driver.uploads.load(Ordering::Relaxed), 1);
    assert_eq!(driver.downloads.load(Ordering::Relaxed), 1);
}
