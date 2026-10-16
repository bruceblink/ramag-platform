use super::*;

fn valid_account() -> ObjectStorageAccount {
    let mut account = ObjectStorageAccount::new("production", CloudProvider::TencentCos);
    account.access_key_id = SecretString::new("AKIDEXAMPLE");
    account.access_key_secret = SecretString::new("secret-value");
    account.manual_buckets = vec![ManualBucket::new("logs-1250000000", "ap-guangzhou")];
    account
}

#[test]
fn account_defaults_to_writable_and_redacts_debug() {
    let account = valid_account();
    assert!(!account.read_only);
    let debug = format!("{account:?}");
    assert!(!debug.contains("AKIDEXAMPLE"));
    assert!(!debug.contains("secret-value"));
    assert!(account.validate().is_ok());
}

#[test]
fn rejects_incomplete_credentials() {
    let mut account = valid_account();
    account.access_key_secret = SecretString::new("");
    assert!(matches!(account.validate(), Err(error) if error.contains("不能为空")));
}

#[test]
fn rejects_account_without_configured_bucket() {
    let mut account = valid_account();
    account.manual_buckets.clear();
    assert!(matches!(account.validate(), Err(error) if error.contains("至少添加一个 Bucket")));
}

#[test]
fn official_endpoint_rejects_custom_hosts_and_redirect_shapes() {
    assert!(
        HttpsEndpoint::parse_official(
            CloudProvider::TencentCos,
            "https://cos.ap-guangzhou.myqcloud.com"
        )
        .is_ok()
    );
    assert!(
        HttpsEndpoint::parse_official(
            CloudProvider::AliyunOss,
            "https://oss-cn-hangzhou.aliyuncs.com"
        )
        .is_ok()
    );
    for endpoint in [
        "http://cos.ap-guangzhou.myqcloud.com",
        "https://evil.example.com",
        "https://oss-cn-hangzhou.aliyuncs.com@evil.example",
        "https://oss-cn-hangzhou.aliyuncs.com/path",
        "https://bucket.oss-cn-hangzhou.aliyuncs.com",
    ] {
        assert!(HttpsEndpoint::parse_official(CloudProvider::AliyunOss, endpoint).is_err());
    }
    assert!(
        HttpsEndpoint::parse_official(
            CloudProvider::TencentCos,
            "https://cos.ap-guangzhou.extra.myqcloud.com"
        )
        .is_err()
    );
}

#[test]
fn s3_endpoint_accepts_https_and_local_loopback_http_only() {
    assert!(ObjectStorageEndpoint::parse_s3_compatible("https://objects.example.com").is_ok());
    assert!(ObjectStorageEndpoint::parse_s3_compatible("http://localhost:9000").is_ok());
    assert!(ObjectStorageEndpoint::parse_s3_compatible("http://127.0.0.1:9000").is_ok());
    assert!(ObjectStorageEndpoint::parse_s3_compatible("http://[::1]:9000").is_ok());
    for endpoint in [
        "http://objects.example.com",
        "http://192.168.1.10:9000",
        "http://@localhost:9000",
        "https://user:secret@objects.example.com",
        "https://objects.example.com/path",
        "https://objects.example.com/?bucket=bad",
        "https://objects.example.com/#fragment",
        "ftp://objects.example.com",
    ] {
        assert!(
            ObjectStorageEndpoint::parse_s3_compatible(endpoint).is_err(),
            "{endpoint}"
        );
    }
}

#[test]
fn old_account_records_without_s3_endpoint_still_load() -> Result<(), String> {
    let mut value = serde_json::to_value(valid_account()).map_err(|error| error.to_string())?;
    value
        .as_object_mut()
        .ok_or_else(|| "account did not serialize as a JSON object".to_string())?
        .remove("s3_endpoint");
    let account: ObjectStorageAccount =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    assert_eq!(account.provider, CloudProvider::TencentCos);
    assert!(account.s3_endpoint.is_none());
    assert!(account.validate().is_ok());
    Ok(())
}

#[test]
fn s3_account_requires_a_valid_endpoint() -> Result<(), String> {
    let mut account = ObjectStorageAccount::new("local", CloudProvider::S3Compatible);
    account.access_key_id = SecretString::new("access");
    account.access_key_secret = SecretString::new("secret");
    account.manual_buckets = vec![ManualBucket::new("local-bucket", "us-east-1")];
    assert!(account.validate().is_err());
    account.s3_endpoint = Some(
        serde_json::from_value(serde_json::json!("http://example.com"))
            .map_err(|error| error.to_string())?,
    );
    assert!(account.validate().is_err());
    account.s3_endpoint = Some(ObjectStorageEndpoint::parse_s3_compatible(
        "http://127.0.0.1:9000",
    )?);
    assert!(account.validate().is_ok());
    Ok(())
}

#[test]
fn unsafe_keys_are_never_marked_operable() {
    for key in [
        "/leading",
        "trailing ",
        "a//b",
        "a/../b",
        "a/./b",
        "folder/",
        "line\nbreak",
        "rtl\u{202e}name",
    ] {
        assert!(!is_opendal_safe_key(key), "{key:?}");
    }
    for key in ["plain.txt", "目录/文件.txt", "a..b", "percent%2Fname"] {
        assert!(is_opendal_safe_key(key), "{key:?}");
    }
}

#[test]
fn root_prefix_requires_safe_relative_directory_shape() {
    assert!(validate_root_prefix("team/reports/").is_ok());
    assert!(validate_root_prefix("team/reports").is_err());
    assert!(validate_root_prefix("/team/reports/").is_err());
    assert!(validate_root_prefix("team//reports/").is_err());
}

#[test]
fn list_query_keeps_directory_and_name_prefix_separate() -> Result<(), String> {
    let query = ObjectListQuery::new("reports/2026/", "aug")?;
    assert_eq!(query.directory_prefix(), "reports/2026/");
    assert_eq!(query.name_prefix(), "aug");
    assert_eq!(query.list_prefix(), "reports/2026/aug");
    assert!(ObjectListQuery::new("reports/", "../secret").is_err());
    assert!(ObjectListQuery::new("reports/", "bad/name").is_err());
    Ok(())
}

#[test]
fn bucket_and_region_use_shared_official_character_subset() {
    assert!(validate_bucket_name("logs-1234567890").is_ok());
    assert!(validate_bucket_name("UPPER").is_err());
    assert!(validate_bucket_name("-leading").is_err());
    assert!(validate_region("ap-guangzhou").is_ok());
    assert!(validate_region("ap_guangzhou").is_err());
    assert!(validate_bucket_name_for_provider(CloudProvider::AliyunOss, &"a".repeat(64)).is_err());
}

#[test]
fn account_revision_saturates_instead_of_wrapping() {
    let mut account = valid_account();
    account.revision = u64::MAX;
    assert_eq!(account.next_revision(), u64::MAX);
}

#[test]
fn object_storage_user_message_includes_safe_provider_diagnostics() {
    let error = crate::error::ObjectStorageError::new(
        crate::error::ObjectStorageErrorCategory::Provider,
        "list",
        "云服务失败",
    )
    .with_provider_details(Some("ServiceBusy".into()), Some("request-123".into()));
    let message = error.user_message();
    assert!(message.contains("ServiceBusy"));
    assert!(message.contains("request-123"));
}
