use serde_json::json;

use super::super::{
    ApiAuth, ApiCollection, ApiEnvironment, ApiWorkspace, HttpRequestSpec, MAX_API_IMPORT_BYTES,
};
use super::{ApiImportFormat, import_api_json};

fn postman_info() -> serde_json::Value {
    json!({
        "name": "Billing API",
        "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
    })
}

#[test]
fn imports_nested_postman_requests_and_preserves_supported_fields() -> Result<(), String> {
    let document = json!({
        "info": postman_info(),
        "variable": [
            {"key": "baseUrl", "value": "http://127.0.0.1:8080"},
            {"key": "token", "value": "secret", "type": "secret"}
        ],
        "item": [{
            "name": "Users",
            "item": [{
                "name": "List",
                "event": [{"listen": "test", "script": {"exec": ["pm.test('ok')"]}}],
                "request": {
                    "method": "POST",
                    "header": [
                        {"key": "Content-Type", "value": "application/json"},
                        {"key": "Disabled", "value": "ignored", "disabled": true}
                    ],
                    "auth": {
                        "type": "bearer",
                        "bearer": [{"key": "token", "value": "{{token}}"}]
                    },
                    "url": {
                        "raw": "{{baseUrl}}/users?ignored=true",
                        "query": [
                            {"key": "page", "value": "2"},
                            {"key": "skip", "value": "x", "disabled": true}
                        ]
                    },
                    "body": {
                        "mode": "raw",
                        "raw": "{\"active\":true}",
                        "options": {"raw": {"language": "json"}}
                    }
                }
            }]
        }]
    });

    let bundle = import_api_json(&document.to_string())?;
    assert_eq!(
        bundle.summary().format,
        ApiImportFormat::PostmanCollectionV21
    );
    assert_eq!(bundle.summary().collection_count, 1);
    assert_eq!(bundle.summary().request_count, 1);
    assert_eq!(bundle.summary().environment_count, 1);
    assert_eq!(bundle.summary().warnings.len(), 1);

    let mut target = ApiWorkspace::new("Target");
    let summary = bundle.merge_into(&mut target)?;
    assert_eq!(summary.request_count, 1);
    assert_eq!(target.collections[0].name, "Billing API");
    let request = &target.collections[0].requests[0];
    assert_eq!(request.name, "Users / List");
    let super::super::ApiRequestSpec::Http(spec) = &request.request else {
        return Err("expected HTTP request".into());
    };
    assert_eq!(spec.method, "POST");
    assert_eq!(spec.url_template, "{{baseUrl}}/users");
    assert_eq!(spec.query.len(), 1);
    assert_eq!(spec.query[0].name, "page");
    assert_eq!(spec.headers.len(), 1);
    assert_eq!(
        spec.body.as_ref().map(|body| body.value.as_str()),
        Some("{\"active\":true}")
    );
    assert_eq!(
        spec.body
            .as_ref()
            .and_then(|body| body.content_type.as_deref()),
        Some("application/json")
    );
    assert_eq!(
        spec.auth,
        ApiAuth::Bearer {
            token: "{{token}}".into()
        }
    );
    assert_eq!(
        target.environments[0].variables["baseUrl"],
        "http://127.0.0.1:8080"
    );
    assert_eq!(
        target.environments[0].sensitive_variable_refs,
        vec!["token"]
    );
    Ok(())
}

#[test]
fn imports_ramag_workspace_and_remaps_conflicting_environment_names() -> Result<(), String> {
    let mut source = ApiWorkspace::new("Imported Workspace");
    let mut environment = ApiEnvironment::new("Local");
    environment
        .variables
        .insert("base_url".into(), "http://127.0.0.1".into());
    source.default_environment_id = Some(environment.id.clone());
    source.environments.push(environment);
    let mut collection = ApiCollection::new("Smoke");
    collection
        .requests
        .push(super::super::ApiRequestRecord::new_http(
            "Health",
            HttpRequestSpec::new("GET", "{{base_url}}/health"),
        ));
    source.collections.push(collection);

    let raw = serde_json::to_string(&source).map_err(|error| error.to_string())?;
    let bundle = import_api_json(&raw)?;
    assert_eq!(bundle.summary().format, ApiImportFormat::RamagJson);

    let mut target = ApiWorkspace::new("Target");
    target.environments.push(ApiEnvironment::new("Local"));
    let original_request_id = source.collections[0].requests[0].id.clone();
    bundle.merge_into(&mut target)?;

    assert_eq!(target.collections.len(), 1);
    assert_eq!(target.environments.len(), 2);
    assert_eq!(target.environments[1].name, "Local (导入)");
    assert_eq!(
        target.default_environment_id,
        Some(target.environments[1].id.clone())
    );
    assert_ne!(target.collections[0].requests[0].id, original_request_id);
    Ok(())
}

#[test]
fn rejects_unsupported_postman_body_with_json_path() -> Result<(), String> {
    let document = json!({
        "info": postman_info(),
        "item": [{
            "name": "Upload",
            "request": {
                "method": "POST",
                "url": "https://example.test/upload",
                "body": {"mode": "file"}
            }
        }]
    });

    let error = match import_api_json(&document.to_string()) {
        Ok(_) => return Err("file body must be rejected".into()),
        Err(error) => error,
    };
    assert!(error.contains("$.item[0].request.body.mode"), "{error}");
    Ok(())
}

#[test]
fn rejects_imports_above_the_file_size_limit_before_parsing() -> Result<(), String> {
    let raw = "x".repeat(MAX_API_IMPORT_BYTES + 1);
    let error = match import_api_json(&raw) {
        Ok(_) => return Err("oversized JSON must be rejected".into()),
        Err(error) => error,
    };
    assert!(error.contains("bytes 上限"), "{error}");
    Ok(())
}
