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

#[test]
fn imports_openapi_servers_parameters_body_refs_and_authentication() -> Result<(), String> {
    let document = serde_json::json!({
        "openapi": "3.0.3",
        "info": {"title": "Inventory API", "version": "1.0.0"},
        "servers": [{
            "url": "http://127.0.0.1:18089/{version}",
            "variables": {"version": {"default": "api"}}
        }],
        "security": [{"bearerAuth": []}],
        "components": {
            "parameters": {
                "ItemId": {
                    "name": "id",
                    "in": "path",
                    "required": true,
                    "schema": {"type": "string"},
                    "example": "42"
                }
            },
            "schemas": {
                "CreateItem": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "enabled": {"type": "boolean"}
                    }
                }
            },
            "securitySchemes": {
                "bearerAuth": {"type": "http", "scheme": "bearer"}
            }
        },
        "paths": {
            "/items/{id}": {
                "parameters": [{"$ref": "#/components/parameters/ItemId"}],
                "get": {
                    "operationId": "getItem",
                    "parameters": [
                        {
                            "name": "q",
                            "in": "query",
                            "required": true,
                            "example": "active"
                        },
                        {
                            "name": "X-Trace",
                            "in": "header",
                            "example": "trace-1"
                        }
                    ],
                    "responses": {"200": {"description": "ok"}}
                }
            },
            "/items": {
                "post": {
                    "operationId": "createItem",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/CreateItem"},
                                "examples": {
                                    "default": {
                                        "value": {"name": "demo", "enabled": true}
                                    }
                                }
                            }
                        }
                    },
                    "responses": {"201": {"description": "created"}}
                }
            }
        }
    });

    let bundle = import_api_json(&document.to_string())?;
    assert_eq!(bundle.summary().format, ApiImportFormat::OpenApi3Json);
    assert_eq!(bundle.summary().collection_count, 1);
    assert_eq!(bundle.summary().request_count, 2);
    assert_eq!(bundle.summary().environment_count, 1);

    let mut target = ApiWorkspace::new("Target");
    bundle.merge_into(&mut target)?;
    let collection = &target.collections[0];
    let get_item = collection
        .requests
        .iter()
        .find(|request| request.name == "getItem")
        .ok_or_else(|| "GET 请求未导入".to_string())?;
    let super::super::ApiRequestSpec::Http(get_spec) = &get_item.request else {
        return Err("GET 请求协议错误".into());
    };
    assert_eq!(
        get_spec.url_template,
        "http://127.0.0.1:18089/{{version}}/items/42"
    );
    assert_eq!(
        get_spec.query,
        vec![super::super::ApiParameter::new("q", "active", false)]
    );
    assert_eq!(
        get_spec.headers,
        vec![super::super::ApiParameter::new("X-Trace", "trace-1", false)]
    );
    assert_eq!(
        get_spec.auth,
        ApiAuth::Bearer {
            token: "{{openapi_bearerAuth_token}}".into()
        }
    );
    let post_item = collection
        .requests
        .iter()
        .find(|request| request.name == "createItem")
        .ok_or_else(|| "POST 请求未导入".to_string())?;
    let super::super::ApiRequestSpec::Http(post_spec) = &post_item.request else {
        return Err("POST 请求协议错误".into());
    };
    let body = post_spec
        .body
        .as_ref()
        .ok_or_else(|| "POST 请求缺少正文".to_string())?;
    let body_value = serde_json::from_str::<serde_json::Value>(&body.value)
        .map_err(|error| format!("POST 请求正文不是有效 JSON：{error}"))?;
    assert_eq!(
        body_value,
        serde_json::json!({"enabled": true, "name": "demo"})
    );
    assert_eq!(target.environments[0].variables["version"], "api");
    assert_eq!(
        target.environments[0].sensitive_variable_refs,
        vec!["openapi_bearerAuth_token"]
    );
    Ok(())
}

#[test]
fn rejects_openapi_external_refs_with_json_path() -> Result<(), String> {
    let document = serde_json::json!({
        "openapi": "3.0.3",
        "info": {"title": "External Ref", "version": "1.0.0"},
        "servers": [{"url": "http://127.0.0.1:18089"}],
        "paths": {
            "/users": {
                "get": {
                    "parameters": [{
                        "$ref": "https://example.test/parameters.json#/UserId"
                    }],
                    "responses": {"200": {"description": "ok"}}
                }
            }
        }
    });
    let error = match import_api_json(&document.to_string()) {
        Ok(_) => return Err("external ref must be rejected".into()),
        Err(error) => error,
    };
    assert!(
        error.contains("$.paths./users.get.parameters[0].$ref"),
        "{error}"
    );
    assert!(error.contains("本地 $ref"), "{error}");
    Ok(())
}

#[test]
fn rejects_openapi_schema_ref_cycles_with_json_path() -> Result<(), String> {
    let document = serde_json::json!({
        "openapi": "3.0.3",
        "info": {"title": "Cyclic Schema", "version": "1.0.0"},
        "servers": [{"url": "http://127.0.0.1:18089"}],
        "components": {
            "schemas": {
                "Node": {
                    "type": "object",
                    "properties": {
                        "next": {"$ref": "#/components/schemas/Node"}
                    }
                }
            }
        },
        "paths": {
            "/nodes": {
                "post": {
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/Node"}
                            }
                        }
                    },
                    "responses": {"200": {"description": "ok"}}
                }
            }
        }
    });
    let error = match import_api_json(&document.to_string()) {
        Ok(_) => return Err("cyclic schema ref must be rejected".into()),
        Err(error) => error,
    };
    assert!(error.contains("循环引用"), "{error}");
    assert!(error.contains("$.paths./nodes.post.requestBody"), "{error}");
    Ok(())
}
