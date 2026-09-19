use super::*;

#[test]
fn workspace_validation_covers_protocol_and_reference_limits() {
    let mut workspace = ApiWorkspace::new("local");
    let mut collection = ApiCollection::new("smoke");
    collection.requests.push(ApiRequestRecord::new_http(
        "health",
        HttpRequestSpec::new("GET", "{{base_url}}/health"),
    ));
    workspace.collections.push(collection);

    let mut environment = ApiEnvironment::new("dev");
    environment
        .variables
        .insert("base_url".into(), "http://127.0.0.1:8080".into());
    environment.sensitive_variable_refs.push("missing".into());
    workspace.environments.push(environment);

    let error = match workspace.validate() {
        Ok(()) => String::new(),
        Err(error) => error,
    };
    assert!(error.contains("敏感变量引用不存在"));
}

#[test]
fn grpc_request_rejects_mismatched_client_certificate_paths() {
    let mut request = GrpcRequestSpec::new("https://127.0.0.1:50051", "api.Echo", "Unary");
    request.tls.client_cert_path = Some("client.pem".into());
    assert!(request.validate().is_err());
}

#[test]
fn debug_output_redacts_auth_body_and_environment_values() {
    let mut environment = ApiEnvironment::new("dev");
    environment
        .variables
        .insert("token".into(), "very-secret-token".into());
    let request = ApiRequestRecord::new_http(
        "secret",
        HttpRequestSpec {
            method: "POST".into(),
            url_template: "https://example.test".into(),
            query: Vec::new(),
            headers: vec![ApiParameter::new("Authorization", "secret-header", true)],
            auth: ApiAuth::Bearer {
                token: "secret-token".into(),
            },
            body: Some(ApiBody::text(
                "secret-body",
                Some("application/json".into()),
            )),
            tls: ApiTlsConfig::default(),
            timeout_millis: 30_000,
        },
    );
    let output = format!("{environment:?} {request:?}");
    assert!(!output.contains("very-secret-token"));
    assert!(!output.contains("secret-token"));
    assert!(!output.contains("secret-body"));
}

#[test]
fn response_body_is_bounded_without_losing_original_size() {
    let body = vec![b'x'; MAX_API_RESPONSE_BODY_BYTES + 1];
    let (bounded, size, truncated) = bound_response_body(body);
    assert_eq!(bounded.len(), MAX_API_RESPONSE_BODY_BYTES);
    assert_eq!(size, (MAX_API_RESPONSE_BODY_BYTES + 1) as u64);
    assert!(truncated);
}

#[test]
fn legacy_request_json_uses_defaults_for_new_optional_fields() {
    let parsed = serde_json::from_str::<ApiRequestRecord>(
        r#"{
            "id": "11111111-1111-1111-1111-111111111111",
            "name": "legacy-health",
            "request": {
                "Http": {
                    "method": "GET",
                    "url_template": "http://127.0.0.1/health"
                }
            }
        }"#,
    );
    assert!(parsed.is_ok(), "旧 API 请求 JSON 应使用默认字段");
    let request = match parsed {
        Ok(request) => request,
        Err(_) => return,
    };

    assert_eq!(request.protocol, ApiProtocol::Http);
    assert!(request.assertions.is_empty());
    assert!(request.validate().is_ok());
}

#[test]
fn template_variables_and_assertions_cover_missing_and_failed_cases() {
    let mut variables = std::collections::BTreeMap::new();
    variables.insert("base_url".into(), "http://127.0.0.1".into());
    let resolved = resolve_template(
        "{{base_url}}/json",
        &variables,
        "URL",
        MAX_API_URL_TEMPLATE_BYTES,
    );
    assert!(resolved.is_ok(), "有效变量应展开：{resolved:?}");
    if let Ok(resolved) = resolved {
        assert_eq!(resolved, "http://127.0.0.1/json");
    }
    let missing = resolve_template("{{missing}}", &variables, "URL", 128);
    assert!(missing.is_err());
    if let Err(error) = missing {
        assert!(error.contains("缺少 API 环境变量"));
    }

    let body = br#"{"ok":true,"nested":{"id":7}}"#.to_vec();
    let snapshot = ApiResponseSnapshot::new(ApiResponseSnapshotParts {
        protocol: ApiProtocol::Http,
        status: ApiResponseStatus::Http { code: 200 },
        headers: vec![ApiParameter::new("content-type", "application/json", false)],
        metadata: Vec::new(),
        body: body.clone(),
        elapsed_millis: 4,
        size_bytes: body.len() as u64,
        truncated: false,
        error: None,
    });
    assert!(snapshot.is_ok(), "应构造 JSON 响应：{snapshot:?}");
    let Ok(snapshot) = snapshot else {
        return;
    };
    let results = evaluate_assertions(
        &[
            ApiAssertion::HttpStatus { expected: 200 },
            ApiAssertion::JsonPathEquals {
                path: "$.nested.id".into(),
                expected: "7".into(),
            },
            ApiAssertion::BodyContains {
                expected: "missing".into(),
            },
        ],
        &snapshot,
    );
    assert!(results.is_ok(), "断言应执行：{results:?}");
    let Ok(results) = results else {
        return;
    };
    assert_eq!(results.len(), 3);
    assert!(results[0].passed);
    assert!(results[1].passed);
    assert!(!results[2].passed);
}

#[test]
fn history_summary_redacts_sensitive_environment_values() {
    let mut environment = ApiEnvironment::new("local");
    environment
        .variables
        .insert("token".into(), "secret-token".into());
    environment.sensitive_variable_refs.push("token".into());
    let record = ApiRequestRecord::new_http("secret-request", HttpRequestSpec::new("GET", "/"));
    let body = b"{\"token\":\"secret-token\"}".to_vec();
    let snapshot = ApiResponseSnapshot::new(ApiResponseSnapshotParts {
        protocol: ApiProtocol::Http,
        status: ApiResponseStatus::Http { code: 200 },
        headers: Vec::new(),
        metadata: Vec::new(),
        body: body.clone(),
        elapsed_millis: 1,
        size_bytes: body.len() as u64,
        truncated: false,
        error: None,
    });
    assert!(snapshot.is_ok(), "应构造历史响应：{snapshot:?}");
    let Ok(snapshot) = snapshot else {
        return;
    };
    let result = ApiExecutionResult {
        snapshot,
        assertions: Vec::new(),
        passed: true,
    };
    let history = ApiHistoryRecord::from_success(&record, &result, &environment);
    assert!(!history.body_preview.contains("secret-token"));
    assert!(history.body_preview.contains("[REDACTED]"));
    assert!(history.validate().is_ok());
}
