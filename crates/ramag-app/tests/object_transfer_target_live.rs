//! 真实应用层对象传输目标集成测试。
//! 仅用于显式配置的本机回环 S3 兼容测试环境，不连接用户云账号。

#[path = "object_transfer_target_live/support.rs"]
mod support;

use std::path::Path;

use ramag_app::ObjectStorageTransferTarget;
use ramag_domain::entities::{
    CloudProvider, ObjectStorageAccount, OverwritePolicy, TransferCancellation,
};
use ramag_domain::error::{DomainError, ObjectStorageErrorCategory};
use support::{TestEnvironment, local_file, progress, target_for};

#[tokio::test]
#[ignore = "requires an explicitly configured loopback S3-compatible test server and two dedicated buckets"]
async fn transfer_targets_keep_upload_and_download_bound_to_the_original_bucket()
-> Result<(), String> {
    let (mut environment, directory) = TestEnvironment::from_env().await?;
    let result = exercise(&mut environment, directory.path()).await;
    let cleanup = environment.cleanup().await;
    drop(environment);
    drop(directory);
    match (result, cleanup.is_empty()) {
        (Ok(()), true) => Ok(()),
        (result, cleanup_ok) => Err(format!(
            "object transfer integration failed: result={result:?}; cleanup_ok={cleanup_ok}; cleanup_failures={cleanup:?}"
        )),
    }
}

async fn exercise(environment: &mut TestEnvironment, directory: &Path) -> Result<(), String> {
    let target_a = environment.target_a.clone();
    let target_b = environment.target_b.clone();
    let key_a = environment.register_key(&target_a, "same-key");
    let key_b = environment.register_key(&target_b, "same-key");
    let cancelled_key = environment.register_key(&target_a, "cancelled-upload");
    let source = local_file(directory, "source", b"original bucket A object")?;
    let replacement = local_file(directory, "replacement", b"replacement bucket A object")?;
    let sentinel_b = local_file(directory, "sentinel-b", b"B bucket sentinel")?;

    upload_seed(environment, &target_a, &key_a, &source).await?;
    upload_seed(environment, &target_b, &key_b, &sentinel_b).await?;
    verify_overwrite(
        environment,
        directory,
        &target_a,
        &target_b,
        &key_a,
        &key_b,
        &replacement,
    )
    .await?;
    verify_cancellation(
        environment,
        directory,
        &target_a,
        &cancelled_key,
        &key_a,
        &replacement,
    )
    .await?;
    verify_stale_account_and_mount(
        environment,
        directory,
        &target_a,
        &target_b,
        &key_a,
        &key_b,
        &replacement,
    )
    .await?;
    verify_deleted_account(environment, directory, &target_b, &key_b, &replacement).await
}

async fn upload_seed(
    environment: &TestEnvironment,
    target: &ObjectStorageTransferTarget,
    key: &str,
    source: &Path,
) -> Result<(), String> {
    environment
        .service
        .upload_object(
            target,
            key.to_owned(),
            source.to_path_buf(),
            OverwritePolicy::Refuse,
            TransferCancellation::default(),
            progress(),
        )
        .await
        .map_err(|_| {
            format!(
                "could not seed isolated object in bucket {}",
                target.mount.bucket
            )
        })
}

async fn verify_overwrite(
    environment: &TestEnvironment,
    directory: &Path,
    target_a: &ObjectStorageTransferTarget,
    target_b: &ObjectStorageTransferTarget,
    key_a: &str,
    key_b: &str,
    replacement: &Path,
) -> Result<(), String> {
    if !matches!(
        environment
            .service
            .upload_object(
                target_a,
                key_a.to_owned(),
                replacement.to_path_buf(),
                OverwritePolicy::Refuse,
                TransferCancellation::default(),
                progress(),
            )
            .await,
        Err(DomainError::ObjectStorage(ref error))
            if error.category == ObjectStorageErrorCategory::Conflict
    ) {
        return Err("Refuse upload did not report an existing-object conflict".into());
    }
    environment
        .service
        .upload_object(
            target_a,
            key_a.to_owned(),
            replacement.to_path_buf(),
            OverwritePolicy::Overwrite,
            TransferCancellation::default(),
            progress(),
        )
        .await
        .map_err(|_| "could not overwrite the object in original bucket A".to_string())?;
    if environment
        .read_remote(target_a, key_a, directory, "upload-a")
        .await?
        != b"replacement bucket A object"
        || environment
            .read_remote(target_b, key_b, directory, "upload-b")
            .await?
            != b"B bucket sentinel"
    {
        return Err("upload target binding changed the A/B same-key sentinel".into());
    }

    let local_path = local_file(directory, "download-target", b"local sentinel")?;
    if !matches!(
        environment
            .service
            .download_object(
                target_a,
                key_a.to_owned(),
                local_path.clone(),
                OverwritePolicy::Refuse,
                TransferCancellation::default(),
                progress(),
            )
            .await,
        Err(DomainError::ObjectStorage(ref error))
            if error.category == ObjectStorageErrorCategory::Conflict
    ) {
        return Err("Refuse download did not report an existing-local-file conflict".into());
    }
    environment
        .service
        .download_object(
            target_a,
            key_a.to_owned(),
            local_path.clone(),
            OverwritePolicy::Overwrite,
            TransferCancellation::default(),
            progress(),
        )
        .await
        .map_err(|_| "could not overwrite the local download target".to_string())?;
    if std::fs::read(local_path).map_err(|error| error.to_string())?
        != b"replacement bucket A object"
    {
        return Err("overwrite download did not read the original bucket A object".into());
    }
    Ok(())
}

async fn verify_cancellation(
    environment: &TestEnvironment,
    directory: &Path,
    target: &ObjectStorageTransferTarget,
    cancelled_key: &str,
    existing_key: &str,
    source: &Path,
) -> Result<(), String> {
    let cancellation = TransferCancellation::default();
    cancellation.cancel();
    if !matches!(
        environment
            .service
            .upload_object(
                target,
                cancelled_key.to_owned(),
                source.to_path_buf(),
                OverwritePolicy::Overwrite,
                cancellation,
                progress(),
            )
            .await,
        Err(DomainError::ObjectStorage(ref error))
            if error.category == ObjectStorageErrorCategory::Cancelled
    ) {
        return Err("pre-cancelled upload did not return Cancelled".into());
    }
    if !matches!(
        environment
            .service
            .stat_object(&target.account_id, &target.mount, cancelled_key)
            .await,
        Err(DomainError::ObjectStorage(ref error))
            if error.category == ObjectStorageErrorCategory::NotFound
    ) {
        return Err(
            "pre-cancelled upload created an object or absence could not be verified".into(),
        );
    }

    let local_path = local_file(directory, "cancelled-download", b"local sentinel")?;
    let cancellation = TransferCancellation::default();
    cancellation.cancel();
    if !matches!(
        environment
            .service
            .download_object(
                target,
                existing_key.to_owned(),
                local_path.clone(),
                OverwritePolicy::Overwrite,
                cancellation,
                progress(),
            )
            .await,
        Err(DomainError::ObjectStorage(ref error))
            if error.category == ObjectStorageErrorCategory::Cancelled
    ) {
        return Err("pre-cancelled download did not return Cancelled".into());
    }
    if std::fs::read(local_path).map_err(|error| error.to_string())? != b"local sentinel" {
        return Err("cancelled download changed the existing local file".into());
    }
    Ok(())
}

async fn verify_stale_account_and_mount(
    environment: &mut TestEnvironment,
    directory: &Path,
    target_a: &ObjectStorageTransferTarget,
    target_b: &ObjectStorageTransferTarget,
    key_a: &str,
    key_b: &str,
    source: &Path,
) -> Result<(), String> {
    let mut changed = environment.account_a.clone();
    changed.name.push_str(" updated");
    let updated = environment
        .service
        .save_account(changed)
        .await
        .map_err(|_| "could not update isolated account A".to_string())?
        .account;
    if !matches!(
        environment
            .service
            .upload_object(
                target_a,
                key_a.to_owned(),
                source.to_path_buf(),
                OverwritePolicy::Overwrite,
                TransferCancellation::default(),
                progress(),
            )
            .await,
        Err(DomainError::InvalidConfig(_))
    ) {
        return Err("stale account revision was not rejected before upload".into());
    }
    let current_a = target_for(&updated)?;
    environment.account_a = updated.clone();
    environment.target_a = current_a.clone();
    if environment
        .read_remote(&current_a, key_a, directory, "revision-a")
        .await?
        != b"replacement bucket A object"
        || environment
            .read_remote(target_b, key_b, directory, "revision-b")
            .await?
            != b"B bucket sentinel"
    {
        return Err("stale revision rejection changed the A/B same-key sentinel".into());
    }

    let changed_mount = ObjectStorageTransferTarget {
        account_id: updated.id.clone(),
        account_revision: updated.revision,
        mount: {
            let mut mount = target_a.mount.clone();
            mount.root_prefix = Some("changed/".into());
            mount
        },
    };
    if !matches!(
        environment
            .service
            .download_object(
                &changed_mount,
                key_a.to_owned(),
                directory.join("changed-mount.txt"),
                OverwritePolicy::Overwrite,
                TransferCancellation::default(),
                progress(),
            )
            .await,
        Err(DomainError::InvalidConfig(_))
    ) {
        return Err("changed mount snapshot was not rejected before download".into());
    }
    Ok(())
}

async fn verify_deleted_account(
    environment: &mut TestEnvironment,
    directory: &Path,
    target_b: &ObjectStorageTransferTarget,
    key_b: &str,
    source: &Path,
) -> Result<(), String> {
    let mut account = ObjectStorageAccount::new(
        "object transfer deleted account",
        CloudProvider::S3Compatible,
    );
    account.s3_endpoint = environment.account_b.s3_endpoint.clone();
    account.access_key_id = environment.account_b.access_key_id.clone();
    account.access_key_secret = environment.account_b.access_key_secret.clone();
    account.manual_buckets = environment.account_b.manual_buckets.clone();
    let deleted = environment
        .service
        .save_account(account)
        .await
        .map_err(|_| "could not save isolated deletion-test account".to_string())?
        .account;
    let target_deleted = target_for(&deleted)?;
    environment
        .service
        .delete_account(&deleted.id)
        .await
        .map_err(|_| "could not delete isolated test account".to_string())?;
    if !matches!(
        environment
            .service
            .upload_object(
                &target_deleted,
                key_b.to_owned(),
                source.to_path_buf(),
                OverwritePolicy::Overwrite,
                TransferCancellation::default(),
                progress(),
            )
            .await,
        Err(DomainError::NotFound(_))
    ) {
        return Err("deleted account target was not rejected before upload".into());
    }
    if !matches!(
        environment
            .service
            .download_object(
                &target_deleted,
                key_b.to_owned(),
                directory.join("deleted-account-local.txt"),
                OverwritePolicy::Overwrite,
                TransferCancellation::default(),
                progress(),
            )
            .await,
        Err(DomainError::NotFound(_))
    ) {
        return Err("deleted account target was not rejected before download".into());
    }
    if environment
        .read_remote(target_b, key_b, directory, "deleted-account-b")
        .await?
        != b"B bucket sentinel"
    {
        return Err("deleted-account rejection changed the B bucket sentinel".into());
    }
    Ok(())
}
