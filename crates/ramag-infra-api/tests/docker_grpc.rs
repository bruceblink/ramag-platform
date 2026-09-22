//! 本机 Docker gRPC 服务集成测试；未配置端点时保持默认测试命令可运行并跳过。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ramag_domain::entities::{
    ApiAuth, ApiCancellation, ApiGrpcDescriptor, ApiGrpcDiscoverySpec, ApiOAuth2Config,
    ApiParameter, ApiProxyConfig, ApiRequestSpec, ApiResponseStatus, ApiTlsConfig, ApiTlsVerify,
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

fn docker_mtls_config() -> Option<ApiTlsConfig> {
    let directory = std::env::var("RAMAG_TEST_API_TLS_DIRECTORY").ok()?;
    let path = |name: &str| std::path::Path::new(&directory).join(name);
    Some(ApiTlsConfig {
        verify: ApiTlsVerify::Ca,
        ca_cert_path: Some(path("ca.cert.pem").to_string_lossy().into_owned()),
        client_cert_path: Some(path("client.cert.pem").to_string_lossy().into_owned()),
        client_key_path: Some(path("client.key.pem").to_string_lossy().into_owned()),
    })
}

fn docker_proxy_config() -> Option<ApiProxyConfig> {
    Some(ApiProxyConfig {
        url: Some(std::env::var("RAMAG_TEST_API_GRPC_PROXY_URL").ok()?),
        username: Some(std::env::var("RAMAG_TEST_API_PROXY_USERNAME").ok()?),
        password: Some(std::env::var("RAMAG_TEST_API_PROXY_PASSWORD").ok()?),
    })
}

fn docker_oauth2_auth() -> Option<ApiAuth> {
    Some(ApiAuth::OAuth2 {
        config: ApiOAuth2Config {
            token_url: std::env::var("RAMAG_TEST_API_OAUTH2_TOKEN_URL").ok()?,
            client_id: std::env::var("RAMAG_TEST_API_OAUTH2_CLIENT_ID").ok()?,
            client_secret: std::env::var("RAMAG_TEST_API_OAUTH2_CLIENT_SECRET").ok()?,
            scope: Some(std::env::var("RAMAG_TEST_API_OAUTH2_SCOPE").ok()?),
        },
    })
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
    let discovery = ApiGrpcDiscoverySpec::new(&endpoint);

    let services = driver
        .discover_services(&discovery, &BTreeMap::new(), cancellation())
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

    if let Some(auth) = docker_oauth2_auth() {
        let mut oauth = grpc_request(&endpoint, r#"{"message":"oauth"}"#);
        if let ApiRequestSpec::Grpc(spec) = &mut oauth {
            spec.auth = auth;
        }
        let oauth_response = driver.execute(&oauth, &variables, cancellation()).await?;
        assert_eq!(
            oauth_response.status,
            ApiResponseStatus::Grpc { code: "ok".into() }
        );
    } else {
        eprintln!("跳过 gRPC Docker OAuth2 回归；缺少 Token Endpoint 环境变量");
    }

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
    let cancellation_driver = driver.clone();
    let cancellation_variables = variables.clone();
    let task = tokio::spawn({
        let cancelled = cancelled.clone();
        async move {
            cancellation_driver
                .execute(&success, &cancellation_variables, cancelled)
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    cancelled.store(true, Ordering::Relaxed);
    let joined = tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .map_err(|_| test_error("Docker gRPC cancellation did not return promptly"))?
        .map_err(|_| test_error("Docker gRPC task did not join"))?;
    assert!(matches!(joined, Err(DomainError::Cancelled(_))));

    if let Some(mtls_endpoint) = std::env::var("RAMAG_TEST_API_GRPC_MTLS_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        let mut mtls = grpc_request(&mtls_endpoint, r#"{"message":"mtls"}"#);
        mtls = match mtls {
            ApiRequestSpec::Grpc(mut spec) => {
                spec.tls = docker_mtls_config().ok_or("mTLS 测试缺少证书目录")?;
                ApiRequestSpec::Grpc(spec)
            }
            ApiRequestSpec::Http(_) => unreachable!(),
        };
        let response = driver.execute(&mtls, &variables, cancellation()).await?;
        assert_eq!(
            response.status,
            ApiResponseStatus::Grpc { code: "ok".into() }
        );
        assert!(String::from_utf8(response.body)?.contains("docker echo: mtls"));

        let mut missing_client = match grpc_request(&mtls_endpoint, r#"{"message":"mtls"}"#) {
            ApiRequestSpec::Grpc(spec) => spec,
            ApiRequestSpec::Http(_) => unreachable!(),
        };
        let mut tls = docker_mtls_config().ok_or("mTLS 测试缺少证书目录")?;
        tls.client_cert_path = None;
        tls.client_key_path = None;
        missing_client.tls = tls;
        let error = driver
            .execute(
                &ApiRequestSpec::Grpc(missing_client),
                &variables,
                cancellation(),
            )
            .await;
        let error = match error {
            Ok(_) => return Err("缺少客户端证书时 gRPC mTLS 应握手失败".into()),
            Err(error) => error,
        };
        assert!(matches!(error, DomainError::ConnectionFailed(_)));
    }

    if let Some(proxy) = docker_proxy_config() {
        let mut proxy_variables = variables.clone();
        proxy_variables.insert(
            "proxy_url".into(),
            proxy.url.clone().ok_or("代理测试缺少代理 URL")?,
        );
        proxy_variables.insert(
            "proxy_username".into(),
            proxy.username.clone().ok_or("代理测试缺少代理用户名")?,
        );
        proxy_variables.insert(
            "proxy_password".into(),
            proxy.password.clone().ok_or("代理测试缺少代理密码")?,
        );
        let template_proxy = ApiProxyConfig {
            url: Some("{{proxy_url}}".into()),
            username: Some("{{proxy_username}}".into()),
            password: Some("{{proxy_password}}".into()),
        };

        let mut proxied_discovery = ApiGrpcDiscoverySpec::new("http://localhost:18090");
        proxied_discovery.proxy = template_proxy.clone();
        let services = driver
            .discover_services(&proxied_discovery, &proxy_variables, cancellation())
            .await?;
        assert!(
            services
                .iter()
                .any(|service| service.name == "api.docker.Echo")
        );

        let mut proxied = grpc_request("http://localhost:18090", r#"{"message":"proxy"}"#);
        proxied = match proxied {
            ApiRequestSpec::Grpc(mut spec) => {
                spec.proxy = template_proxy.clone();
                ApiRequestSpec::Grpc(spec)
            }
            ApiRequestSpec::Http(_) => unreachable!(),
        };
        let response = driver
            .execute(&proxied, &proxy_variables, cancellation())
            .await?;
        assert_eq!(
            response.status,
            ApiResponseStatus::Grpc { code: "ok".into() }
        );
        assert!(String::from_utf8(response.body)?.contains("docker echo: proxy"));

        let mut proxied_mtls =
            grpc_request("https://localhost:18092", r#"{"message":"proxy-mtls"}"#);
        proxied_mtls = match proxied_mtls {
            ApiRequestSpec::Grpc(mut spec) => {
                spec.tls = docker_mtls_config().ok_or("mTLS 测试缺少证书目录")?;
                spec.proxy = template_proxy.clone();
                ApiRequestSpec::Grpc(spec)
            }
            ApiRequestSpec::Http(_) => unreachable!(),
        };
        let response = driver
            .execute(&proxied_mtls, &proxy_variables, cancellation())
            .await?;
        assert_eq!(
            response.status,
            ApiResponseStatus::Grpc { code: "ok".into() }
        );

        let proxied_cancelled =
            match grpc_request("http://localhost:18090", r#"{"message":"delay:800"}"#) {
                ApiRequestSpec::Grpc(mut spec) => {
                    spec.proxy = template_proxy.clone();
                    ApiRequestSpec::Grpc(spec)
                }
                ApiRequestSpec::Http(_) => unreachable!(),
            };
        let cancelled = cancellation();
        let cancellation_driver = driver.clone();
        let cancellation_variables = proxy_variables.clone();
        let task = tokio::spawn({
            let cancelled = cancelled.clone();
            async move {
                cancellation_driver
                    .execute(&proxied_cancelled, &cancellation_variables, cancelled)
                    .await
            }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancelled.store(true, Ordering::Relaxed);
        let joined = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .map_err(|_| test_error("代理 gRPC cancellation did not return promptly"))?
            .map_err(|_| test_error("代理 gRPC task did not join"))?;
        assert!(matches!(joined, Err(DomainError::Cancelled(_))));

        let mut unauthorized = grpc_request("http://localhost:18090", r#"{"message":"proxy"}"#);
        unauthorized = match unauthorized {
            ApiRequestSpec::Grpc(mut spec) => {
                let mut bad_proxy = template_proxy;
                bad_proxy.password = Some("wrong-password".into());
                spec.proxy = bad_proxy;
                ApiRequestSpec::Grpc(spec)
            }
            ApiRequestSpec::Http(_) => unreachable!(),
        };
        let error = driver
            .execute(&unauthorized, &proxy_variables, cancellation())
            .await;
        assert!(matches!(error, Err(DomainError::ConnectionFailed(_))));
    }
    Ok(())
}

fn test_error(message: &'static str) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::other(message))
}
