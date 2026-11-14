use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ramag_app::{ObjectStorageService, ObjectStorageTransferTarget, configured_mounts};
use ramag_domain::entities::{
    CloudProvider, ManualBucket, ObjectListQuery, ObjectStorageAccount, ObjectStorageEndpoint,
    OverwritePolicy, SecretString, TransferCancellation,
};
use ramag_infra_object_storage::ObjectStorageInfra;
use ramag_infra_storage::RedbStorage;

const PREVIEW_ENVIRONMENT: &str = "RAMAG_OBJECT_PREVIEW";
pub(super) struct TestEnvironment {
    pub service: Arc<ObjectStorageService>,
    pub account_a: ObjectStorageAccount,
    pub account_b: ObjectStorageAccount,
    pub target_a: ObjectStorageTransferTarget,
    pub target_b: ObjectStorageTransferTarget,
    prefix: String,
    keys: Vec<(ObjectStorageTransferTarget, String)>,
}

impl TestEnvironment {
    pub async fn from_env() -> Result<(Self, tempfile::TempDir), String> {
        let endpoint = required_env("RAMAG_OBJECT_PREVIEW_ENDPOINT")?;
        let endpoint = ObjectStorageEndpoint::parse_s3_compatible(&endpoint)
            .map_err(|error| format!("invalid {PREVIEW_ENVIRONMENT} S3 endpoint: {error}"))?;
        let parsed = url::Url::parse(endpoint.as_str())
            .map_err(|_| format!("{PREVIEW_ENVIRONMENT} endpoint must be a URL"))?;
        if parsed.scheme() != "http" || !is_loopback_host(parsed.host()) {
            return Err(format!(
                "{PREVIEW_ENVIRONMENT} endpoint must be an explicit loopback HTTP endpoint"
            ));
        }
        let access_key = required_env("RAMAG_OBJECT_PREVIEW_ACCESS_KEY")?;
        let secret_key = required_env("RAMAG_OBJECT_PREVIEW_SECRET_KEY")?;
        let bucket_a = required_env("RAMAG_OBJECT_PREVIEW_BUCKET_A")?;
        let bucket_b = required_env("RAMAG_OBJECT_PREVIEW_BUCKET_B")?;
        if bucket_a == bucket_b {
            return Err(format!(
                "{PREVIEW_ENVIRONMENT}_BUCKET_A and _BUCKET_B must be distinct dedicated buckets"
            ));
        }

        let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
        let storage = Arc::new(
            RedbStorage::open_with_key(&directory.path().join("object-transfer.redb"), &[0x5a; 32])
                .map_err(|error| error.to_string())?,
        );
        let service = Arc::new(ObjectStorageService::new(
            Arc::new(ObjectStorageInfra::new().map_err(|error| error.to_string())?),
            storage.clone(),
        ));
        let make_account = |name: &str, bucket: String| {
            let mut account = ObjectStorageAccount::new(name, CloudProvider::S3Compatible);
            account.s3_endpoint = Some(endpoint.clone());
            account.access_key_id = SecretString::new(access_key.clone());
            account.access_key_secret = SecretString::new(secret_key.clone());
            account.manual_buckets = vec![ManualBucket::new(bucket, "us-east-1")];
            account
        };
        let account_a = match service
            .save_account(make_account("object transfer test A", bucket_a))
            .await
        {
            Ok(saved) => saved.account,
            Err(error) => {
                let _ = service.shutdown().await;
                return Err(format!("could not save isolated test account A: {error}"));
            }
        };
        let account_b = match service
            .save_account(make_account("object transfer test B", bucket_b))
            .await
        {
            Ok(saved) => saved.account,
            Err(error) => {
                let _ = service.delete_account(&account_a.id).await;
                let _ = service.shutdown().await;
                return Err(format!("could not save isolated test account B: {error}"));
            }
        };
        let target_a = target_for(&account_a)?;
        let target_b = target_for(&account_b)?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let environment = Self {
            service,
            account_a,
            account_b,
            target_a,
            target_b,
            prefix: format!("ramag-object-target-{}-{nonce}/", std::process::id()),
            keys: Vec::new(),
        };
        if let Err(error) = environment.ensure_prefix_is_empty().await {
            let cleanup = environment.cleanup().await;
            return Err(format!(
                "test prefix precondition failed: {error}; cleanup_failures={cleanup:?}"
            ));
        }
        Ok((environment, directory))
    }

    async fn ensure_prefix_is_empty(&self) -> Result<(), String> {
        for target in [&self.target_a, &self.target_b] {
            let query =
                ObjectListQuery::new(&self.prefix, "").map_err(|error| error.to_string())?;
            let page = self
                .service
                .start_listing(
                    &target.account_id,
                    &target.mount,
                    query.directory_prefix(),
                    "",
                )
                .await
                .map_err(|_| {
                    format!(
                        "could not inspect isolated test prefix in bucket {}",
                        target.mount.bucket
                    )
                })?;
            if !page.page.entries.is_empty() {
                return Err(format!(
                    "isolated test prefix is not empty in bucket {}",
                    target.mount.bucket
                ));
            }
        }
        Ok(())
    }

    pub fn register_key(&mut self, target: &ObjectStorageTransferTarget, label: &str) -> String {
        let key = format!("{}{}.txt", self.prefix, label);
        self.keys.push((target.clone(), key.clone()));
        key
    }

    pub async fn read_remote(
        &self,
        target: &ObjectStorageTransferTarget,
        key: &str,
        directory: &Path,
        label: &str,
    ) -> Result<Vec<u8>, String> {
        let path = local_file(directory, &format!("read-{label}"), b"")?;
        self.service
            .download_object(
                target,
                key.to_owned(),
                path.clone(),
                OverwritePolicy::Overwrite,
                TransferCancellation::default(),
                progress(),
            )
            .await
            .map_err(|_| format!("could not read back isolated test object {key}"))?;
        std::fs::read(&path).map_err(|error| error.to_string())
    }

    pub async fn cleanup(&self) -> Vec<String> {
        let mut failures = Vec::new();
        for (target, key) in &self.keys {
            if self
                .service
                .delete_object(&target.account_id, &target.mount, key)
                .await
                .is_err()
            {
                failures.push(format!(
                    "cleanup failed for isolated key in bucket {}: {key}",
                    target.mount.bucket
                ));
            }
        }
        for account in [&self.account_a, &self.account_b] {
            if self.service.delete_account(&account.id).await.is_err() {
                failures.push(format!(
                    "cleanup failed for isolated test account {}",
                    account.id
                ));
            }
        }
        if self.service.shutdown().await.is_err() {
            failures.push("object storage runtime shutdown failed".into());
        }
        failures
    }
}

pub(super) fn target_for(
    account: &ObjectStorageAccount,
) -> Result<ObjectStorageTransferTarget, String> {
    let mount = configured_mounts(account)
        .map_err(|_| "could not derive configured test mount".to_string())?
        .into_iter()
        .next()
        .ok_or_else(|| "isolated test account has no configured mount".to_string())?;
    Ok(ObjectStorageTransferTarget::new(account, mount))
}

pub(super) fn local_file(
    directory: &Path,
    label: &str,
    contents: &[u8],
) -> Result<std::path::PathBuf, String> {
    let path = directory.join(format!("{label}.txt"));
    std::fs::write(&path, contents).map_err(|error| error.to_string())?;
    Ok(path)
}

pub(super) fn progress() -> ramag_domain::entities::ObjectProgressFn {
    Arc::new(|_| {})
}

fn required_env(name: &str) -> Result<String, String> {
    let value = std::env::var(name)
        .map_err(|_| format!("required loopback test setting {name} is missing"))?;
    if value.trim().is_empty() {
        return Err(format!("required loopback test setting {name} is empty"));
    }
    Ok(value)
}

fn is_loopback_host(host: Option<url::Host<&str>>) -> bool {
    match host {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(host)) => host.is_loopback(),
        Some(url::Host::Ipv6(host)) => host.is_loopback(),
        None => false,
    }
}
