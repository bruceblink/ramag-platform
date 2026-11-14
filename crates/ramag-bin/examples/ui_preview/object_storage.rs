//! 本机对象存储验收装配，只连接调用方明确提供的回环测试服务。

use std::sync::Arc;

use ramag_app::ObjectStorageService;
use ramag_domain::entities::{
    CloudProvider, ManualBucket, ObjectStorageAccount, ObjectStorageEndpoint,
    ObjectStorageSessionPreference, SecretString,
};
use ramag_domain::traits::Storage;
use ramag_infra_object_storage::ObjectStorageInfra;

pub(super) fn service(storage: Arc<dyn Storage>) -> anyhow::Result<Arc<ObjectStorageService>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let endpoint = std::env::var("RAMAG_OBJECT_PREVIEW_ENDPOINT")?;
    let endpoint =
        ObjectStorageEndpoint::parse_s3_compatible(&endpoint).map_err(anyhow::Error::msg)?;
    if !(endpoint.as_str().starts_with("http://127.0.0.1:")
        || endpoint.as_str().starts_with("http://localhost:"))
    {
        anyhow::bail!("对象存储预览只允许显式配置的本机回环 HTTP 测试服务");
    }
    let access_key = std::env::var("RAMAG_OBJECT_PREVIEW_ACCESS_KEY")?;
    let secret_key = std::env::var("RAMAG_OBJECT_PREVIEW_SECRET_KEY")?;
    let first_bucket = std::env::var("RAMAG_OBJECT_PREVIEW_BUCKET_A")?;
    let second_bucket = std::env::var("RAMAG_OBJECT_PREVIEW_BUCKET_B")?;
    let service = Arc::new(ObjectStorageService::new(
        Arc::new(ObjectStorageInfra::new().map_err(anyhow::Error::msg)?),
        storage,
    ));
    futures::executor::block_on(async {
        let mut ids = Vec::new();
        for (name, buckets) in [
            (
                "本机覆盖验收 A",
                vec![first_bucket.clone(), second_bucket.clone()],
            ),
            ("本机覆盖验收 B", vec![second_bucket]),
        ] {
            let mut account = ObjectStorageAccount::new(name, CloudProvider::S3Compatible);
            account.read_only = false;
            account.s3_endpoint = Some(endpoint.clone());
            account.access_key_id = SecretString::new(access_key.clone());
            account.access_key_secret = SecretString::new(secret_key.clone());
            account.manual_buckets = buckets
                .into_iter()
                .map(|bucket| ManualBucket::new(bucket, "us-east-1"))
                .collect();
            let saved = service.save_account(account).await?;
            ids.push(saved.account.id);
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
