use super::{make_test_storage, repos};
use ramag_domain::entities::{
    ApiAuth, ApiBody, ApiCollection, ApiEnvironment, ApiParameter, ApiRequestRecord,
    ApiRequestSpec, ApiWorkspace, HttpRequestSpec,
};
use ramag_domain::traits::Storage;
use redb::ReadableDatabase as _;

fn sample_api_workspace() -> ApiWorkspace {
    let mut workspace = ApiWorkspace::new("api-dev");
    let mut environment = ApiEnvironment::new("local");
    environment
        .variables
        .insert("base_url".into(), "http://127.0.0.1:8080".into());
    environment
        .variables
        .insert("token".into(), "secret-token".into());
    environment.sensitive_variable_refs.push("token".into());
    workspace.default_environment_id = Some(environment.id.clone());
    workspace.environments.push(environment);

    let mut request = ApiRequestRecord::new_http(
        "health",
        HttpRequestSpec::new("POST", "{{base_url}}/health"),
    );
    if let ApiRequestSpec::Http(spec) = &mut request.request {
        spec.headers
            .push(ApiParameter::new("Authorization", "secret-header", true));
        spec.auth = ApiAuth::Bearer {
            token: "secret-token".into(),
        };
        spec.body = Some(ApiBody::text(
            "secret-body",
            Some("application/json".into()),
        ));
    }
    let mut collection = ApiCollection::new("smoke");
    collection.requests.push(request);
    workspace.collections.push(collection);
    workspace
}

#[tokio::test]
async fn api_workspace_roundtrip_encrypts_sensitive_request_data() {
    let (storage, _tmp) = make_test_storage();
    let workspace = sample_api_workspace();

    storage.save_api_workspace(&workspace).await.unwrap();
    assert_eq!(
        storage.list_api_workspaces().await.unwrap(),
        vec![workspace.clone()]
    );
    assert_eq!(
        storage.get_api_workspace(&workspace.id).await.unwrap(),
        Some(workspace.clone())
    );

    let read_txn = storage.db.begin_read().unwrap();
    let table = read_txn
        .open_table(repos::api_workspace_repo::API_WORKSPACES_TABLE)
        .unwrap();
    let raw = table
        .get(workspace.id.to_string().as_str())
        .unwrap()
        .unwrap();
    assert!(!raw.value().contains("api-dev"));
    assert!(!raw.value().contains("secret-token"));
    assert!(!raw.value().contains("secret-body"));
    drop(raw);
    drop(table);
    drop(read_txn);

    storage.delete_api_workspace(&workspace.id).await.unwrap();
    assert!(
        storage
            .get_api_workspace(&workspace.id)
            .await
            .unwrap()
            .is_none()
    );
}
