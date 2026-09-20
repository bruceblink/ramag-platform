//! 本机 Docker HTTP 服务集成测试；未配置端点时保持默认测试命令可运行并跳过。

use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ramag_domain::entities::{
    ApiAuth, ApiBody, ApiCancellation, ApiMultipartPart, ApiParameter, ApiRequestSpec,
    ApiResponseStatus, ApiWorkspace, HttpRequestSpec, import_api_json,
};
use ramag_domain::error::DomainError;
use ramag_domain::traits::ApiDriver;
use ramag_infra_api::HttpApiDriver;

fn docker_endpoint() -> Option<String> {
    match std::env::var("RAMAG_TEST_API_HTTP_URL") {
        Ok(value) if !value.trim().is_empty() => Some(value.trim_end_matches('/').to_string()),
        _ => {
            eprintln!(
                "跳过 API Docker 集成测试；请设置 RAMAG_TEST_API_HTTP_URL 或运行 scripts/api-test/api-test.ps1 test。"
            );
            None
        }
    }
}

fn cancellation() -> ApiCancellation {
    Arc::new(AtomicBool::new(false))
}

fn http_request(method: &str, url: String) -> ApiRequestSpec {
    ApiRequestSpec::Http(HttpRequestSpec::new(method, url))
}

fn body_text(body: &[u8]) -> Result<String, std::string::FromUtf8Error> {
    String::from_utf8(body.to_vec())
}

#[tokio::test]
async fn docker_http_fixture_covers_requests_errors_auth_timeout_and_cancellation()
-> Result<(), Box<dyn std::error::Error>> {
    let Some(endpoint) = docker_endpoint() else {
        return Ok(());
    };
    let driver = HttpApiDriver::new()?;

    let health = driver
        .execute(
            &http_request("GET", format!("{endpoint}/json")),
            &BTreeMap::new(),
            cancellation(),
        )
        .await?;
    assert_eq!(health.status, ApiResponseStatus::Http { code: 200 });
    assert!(
        health
            .headers
            .iter()
            .any(|header| header.name.eq_ignore_ascii_case("x-ramag-docker")
                && header.value == "api-http")
    );
    assert!(body_text(&health.body)?.contains("ramag-api-http-test"));

    let openapi = serde_json::json!({
        "openapi": "3.0.3",
        "info": {"title": "Docker OpenAPI", "version": "1.0.0"},
        "servers": [{"url": endpoint}],
        "paths": {
            "/json": {
                "get": {
                    "operationId": "dockerJson",
                    "responses": {"200": {"description": "ok"}}
                }
            }
        }
    });
    let bundle = import_api_json(&openapi.to_string())?;
    let mut workspace = ApiWorkspace::new("Docker OpenAPI");
    bundle.merge_into(&mut workspace)?;
    let imported = &workspace.collections[0].requests[0];
    let imported_response = driver
        .execute(&imported.request, &BTreeMap::new(), cancellation())
        .await?;
    assert_eq!(
        imported_response.status,
        ApiResponseStatus::Http { code: 200 }
    );
    assert!(body_text(&imported_response.body)?.contains("ramag-api-http-test"));

    let mut variables = BTreeMap::new();
    variables.insert("resource".into(), "orders".into());
    variables.insert("query".into(), "hello world".into());
    variables.insert("token".into(), "docker-header".into());
    let mut echo = HttpRequestSpec::new("POST", format!("{endpoint}/echo/{{{{resource}}}}"));
    echo.query.push(ApiParameter::new("q", "{{query}}", false));
    echo.headers
        .push(ApiParameter::new("X-Request-Token", "{{token}}", false));
    echo.auth = ApiAuth::Basic {
        username: "api-user".into(),
        password: "api-pass".into(),
    };
    echo.body = Some(ApiBody::text(
        r#"{"message":"docker"}"#,
        Some("application/json".into()),
    ));
    let echo_response = driver
        .execute(&ApiRequestSpec::Http(echo), &variables, cancellation())
        .await?;
    let echo_body = body_text(&echo_response.body)?;
    assert!(echo_body.contains("/echo/orders"));
    assert!(echo_body.contains("hello world"));
    assert!(echo_body.contains("docker-header"));
    assert!(echo_body.contains("authorization_received"));
    assert!(echo_body.contains("docker"));

    let file_path = std::env::temp_dir().join(format!(
        "ramag-api-docker-multipart-{}.bin",
        std::process::id()
    ));
    fs::write(&file_path, b"file-content")?;
    let mut multipart = HttpRequestSpec::new("POST", format!("{endpoint}/multipart"));
    multipart.body = Some(ApiBody::multipart(vec![
        ApiMultipartPart::text("title", "hello", false),
        ApiMultipartPart::file(
            "upload",
            file_path.to_string_lossy().into_owned(),
            None,
            Some("application/octet-stream".into()),
        ),
    ]));
    let multipart_response = driver
        .execute(
            &ApiRequestSpec::Http(multipart),
            &BTreeMap::new(),
            cancellation(),
        )
        .await;
    let _ = fs::remove_file(&file_path);
    let multipart_response = multipart_response?;
    assert_eq!(
        multipart_response.status,
        ApiResponseStatus::Http { code: 200 }
    );
    assert!(body_text(&multipart_response.body)?.contains("\"multipart\":true"));

    let error_response = driver
        .execute(
            &http_request("GET", format!("{endpoint}/error")),
            &BTreeMap::new(),
            cancellation(),
        )
        .await?;
    assert_eq!(error_response.status, ApiResponseStatus::Http { code: 418 });
    assert!(body_text(&error_response.body)?.contains("teapot"));

    let unauthorized = driver
        .execute(
            &http_request("GET", format!("{endpoint}/auth")),
            &BTreeMap::new(),
            cancellation(),
        )
        .await?;
    assert_eq!(unauthorized.status, ApiResponseStatus::Http { code: 401 });
    assert!(
        unauthorized
            .headers
            .iter()
            .any(|header| header.name.eq_ignore_ascii_case("www-authenticate"))
    );

    let mut authenticated = HttpRequestSpec::new("GET", format!("{endpoint}/auth"));
    authenticated.auth = ApiAuth::Basic {
        username: "api-user".into(),
        password: "api-pass".into(),
    };
    let authenticated = driver
        .execute(
            &ApiRequestSpec::Http(authenticated),
            &BTreeMap::new(),
            cancellation(),
        )
        .await?;
    assert_eq!(authenticated.status, ApiResponseStatus::Http { code: 200 });
    assert!(body_text(&authenticated.body)?.contains("authenticated"));

    let mut delayed = HttpRequestSpec::new("GET", format!("{endpoint}/delay?ms=250"));
    delayed.timeout_millis = 50;
    let timeout = driver
        .execute(
            &ApiRequestSpec::Http(delayed),
            &BTreeMap::new(),
            cancellation(),
        )
        .await;
    assert!(matches!(timeout, Err(DomainError::ConnectionFailed(_))));

    let cancelled = cancellation();
    let mut streaming = HttpRequestSpec::new("GET", format!("{endpoint}/stream"));
    streaming.timeout_millis = 5_000;
    let task = tokio::spawn({
        let cancelled = cancelled.clone();
        async move {
            driver
                .execute(
                    &ApiRequestSpec::Http(streaming),
                    &BTreeMap::new(),
                    cancelled,
                )
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    cancelled.store(true, Ordering::Relaxed);
    let joined = tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .map_err(|_| test_error("Docker streaming cancellation did not return promptly"))?
        .map_err(|_| test_error("Docker streaming task did not join"))?;
    assert!(matches!(joined, Err(DomainError::Cancelled(_))));
    Ok(())
}

fn test_error(message: &'static str) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::other(message))
}
