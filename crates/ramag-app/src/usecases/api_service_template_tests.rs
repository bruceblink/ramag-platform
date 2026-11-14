use std::sync::Arc;
use std::sync::atomic::Ordering;

use ramag_domain::entities::{
    ApiEnvironment, ApiProtocol, ApiRequestRecord, HttpRequestSpec, MAX_API_URL_TEMPLATE_BYTES,
};
use tempfile::tempdir;

use super::tests::RecordingDriver;
use super::{ApiService, new_api_cancellation};

#[tokio::test]
async fn oversized_environment_template_is_recorded_without_calling_driver() {
    let directory = tempdir().expect("create API template test directory");
    let storage = ramag_infra_storage::RedbStorage::open_with_key(
        &directory.path().join("api-template.redb"),
        &[0x47; 32],
    )
    .expect("open API template test storage");
    let http_driver = Arc::new(RecordingDriver {
        protocol: ApiProtocol::Http,
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let grpc_driver = Arc::new(RecordingDriver {
        protocol: ApiProtocol::Grpc,
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let service = ApiService::new(http_driver.clone(), grpc_driver, Arc::new(storage))
        .expect("construct API service with matching protocol drivers");
    let workspace = ramag_domain::entities::ApiWorkspace::new("template bounds");
    let mut environment = ApiEnvironment::new("local");
    let value = "x".repeat(MAX_API_URL_TEMPLATE_BYTES / 2 + 1);
    environment.variables.insert("payload".into(), value);
    let record = ApiRequestRecord::new_http(
        "oversized URL template",
        HttpRequestSpec::new("GET", "http://127.0.0.1/{{payload}}{{payload}}"),
    );
    let original_record = record.clone();
    let original_environment = environment.clone();

    let outcome = service
        .execute_record(
            &workspace.id,
            &record,
            &mut environment,
            new_api_cancellation(),
        )
        .await
        .expect("template error should produce a recorded outcome");

    assert!(outcome.result.is_none());
    assert!(
        outcome
            .error
            .as_deref()
            .is_some_and(|error| error.contains(&MAX_API_URL_TEMPLATE_BYTES.to_string()))
    );
    assert!(
        outcome
            .history
            .error
            .as_deref()
            .is_some_and(|error| { error.contains(&MAX_API_URL_TEMPLATE_BYTES.to_string()) })
    );
    assert_eq!(http_driver.calls.load(Ordering::Relaxed), 0);
    assert_eq!(record, original_record);
    assert_eq!(environment, original_environment);
    assert!(
        service
            .list_history(&workspace.id, 10)
            .await
            .expect("read recorded template failure")
            .iter()
            .any(|history| history.request_name == record.name && !history.passed)
    );
}

#[tokio::test]
async fn valid_template_still_reaches_recording_driver() {
    let directory = tempdir().expect("create valid API template test directory");
    let storage = ramag_infra_storage::RedbStorage::open_with_key(
        &directory.path().join("api-template-valid.redb"),
        &[0x48; 32],
    )
    .expect("open valid API template test storage");
    let http_driver = Arc::new(RecordingDriver {
        protocol: ApiProtocol::Http,
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let grpc_driver = Arc::new(RecordingDriver {
        protocol: ApiProtocol::Grpc,
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let service = ApiService::new(http_driver.clone(), grpc_driver, Arc::new(storage))
        .expect("construct API service with matching protocol drivers");
    let workspace = ramag_domain::entities::ApiWorkspace::new("valid template");
    let mut environment = ApiEnvironment::new("local");
    environment.variables.insert("path".into(), "health".into());
    let record = ApiRequestRecord::new_http(
        "valid URL template",
        HttpRequestSpec::new("GET", "http://127.0.0.1/{{path}}"),
    );

    let outcome = service
        .execute_record(
            &workspace.id,
            &record,
            &mut environment,
            new_api_cancellation(),
        )
        .await
        .expect("valid template should execute");

    assert!(outcome.result.is_some());
    assert_eq!(http_driver.calls.load(Ordering::Relaxed), 1);
}
