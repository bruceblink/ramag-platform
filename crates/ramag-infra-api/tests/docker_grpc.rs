//! 本机 Docker gRPC 服务集成测试；未配置端点时保持默认测试命令可运行并跳过。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ramag_domain::entities::{
    ApiCancellation, ApiGrpcDescriptor, ApiParameter, ApiRequestSpec, ApiResponseStatus,
    GrpcRequestSpec,
};
use ramag_domain::error::DomainError;
use ramag_domain::traits::ApiDriver;
use ramag_infra_api::GrpcApiDriver;

fn docker_endpoint() -> Option<String> {
    match std::env::var("RAMAG_TEST_API_GRPC_URL") {
        Ok(value) if !value.trim().is_empty() => Some(value.trim_end_matches('/').to_string()),
        _ => {
            eprintln!(
                "跳过 API gRPC Docker 集成测试；请设置 RAMAG_TEST_API_GRPC_URL 或运行 scripts/api-test/grpc-test.ps1 test。"
            );
            None
        }
    }
}

fn cancellation() -> ApiCancellation {
    Arc::new(AtomicBool::new(false))
}

fn grpc_request(endpoint: &str, message: &str) -> ApiRequestSpec {
    grpc_request_for(endpoint, "Unary", message)
}

fn grpc_request_for(endpoint: &str, method: &str, message: &str) -> ApiRequestSpec {
    let mut spec = GrpcRequestSpec::new(endpoint, "api.docker.Echo", method);
    spec.descriptor = ApiGrpcDescriptor::Reflection;
    spec.message = message.into();
    spec.metadata
        .push(ApiParameter::new("x-request", "{{client}}", false));
    ApiRequestSpec::Grpc(spec)
}

#[tokio::test]
async fn docker_grpc_fixture_covers_reflection_unary_metadata_status_and_cancellation()
-> Result<(), Box<dyn std::error::Error>> {
    let Some(endpoint) = docker_endpoint() else {
        return Ok(());
    };
    let driver = GrpcApiDriver::new()?;

    let services = driver
        .discover_services(
            &endpoint,
            &ApiGrpcDescriptor::Reflection,
            &Default::default(),
            5_000,
            &BTreeMap::new(),
            cancellation(),
        )
        .await?;
    assert!(services.iter().any(|service| {
        service.name == "api.docker.Echo"
            && service.methods.iter().any(|method| method.name == "Unary")
            && service.methods.iter().any(|method| {
                method.name == "ServerStream" && !method.client_streaming && method.server_streaming
            })
            && service.methods.iter().any(|method| {
                method.name == "ClientStream" && method.client_streaming && !method.server_streaming
            })
            && service.methods.iter().any(|method| {
                method.name == "BidiStream" && method.client_streaming && method.server_streaming
            })
    }));

    let mut variables = BTreeMap::new();
    variables.insert("client".into(), "docker".into());
    variables.insert("message".into(), "hello".into());
    let mut success = grpc_request(&endpoint, "{\"message\":\"{{message}}\"}");
    let response = driver.execute(&success, &variables, cancellation()).await?;
    assert_eq!(
        response.status,
        ApiResponseStatus::Grpc { code: "ok".into() }
    );
    assert!(String::from_utf8(response.body)?.contains("docker echo: hello"));
    assert!(
        response.metadata.iter().any(|parameter| {
            parameter.name == "x-response" && parameter.value == "docker-grpc"
        })
    );

    let server_stream = driver
        .execute(
            &grpc_request_for(&endpoint, "ServerStream", r#"{"message":"hello"}"#),
            &variables,
            cancellation(),
        )
        .await?;
    assert_eq!(
        server_stream.body,
        br#"[{"message":"docker server: hello:one"},{"message":"docker server: hello:two"}]"#
    );

    let client_stream = driver
        .execute(
            &grpc_request_for(
                &endpoint,
                "ClientStream",
                "{\"message\":\"first\"}\n{\"message\":\"second\"}",
            ),
            &variables,
            cancellation(),
        )
        .await?;
    assert_eq!(
        client_stream.body,
        br#"{"message":"docker client: first,second"}"#
    );

    let bidi_stream = driver
        .execute(
            &grpc_request_for(
                &endpoint,
                "BidiStream",
                "{\"message\":\"left\"}\n{\"message\":\"right\"}",
            ),
            &variables,
            cancellation(),
        )
        .await?;
    assert_eq!(
        bidi_stream.body,
        br#"[{"message":"docker bidi: left"},{"message":"docker bidi: right"}]"#
    );

    let error = driver
        .execute(
            &grpc_request(&endpoint, "{\"message\":\"error\"}"),
            &variables,
            cancellation(),
        )
        .await?;
    assert_eq!(
        error.status,
        ApiResponseStatus::Grpc {
            code: "FailedPrecondition".into()
        }
    );
    assert!(
        error
            .metadata
            .iter()
            .any(|parameter| { parameter.name == "x-error" && parameter.value == "docker" })
    );

    success = grpc_request(&endpoint, "{\"message\":\"delay:800\"}");
    let cancelled = cancellation();
    let task = tokio::spawn({
        let cancelled = cancelled.clone();
        async move { driver.execute(&success, &variables, cancelled).await }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    cancelled.store(true, Ordering::Relaxed);
    let joined = tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .map_err(|_| test_error("Docker gRPC cancellation did not return promptly"))?
        .map_err(|_| test_error("Docker gRPC task did not join"))?;
    assert!(matches!(joined, Err(DomainError::Cancelled(_))));
    Ok(())
}

fn test_error(message: &'static str) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::other(message))
}
