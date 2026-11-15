//! 隔离 API 窗口验收装配；只保存本机 Docker 测试服务的请求，不读取用户工作区。

use std::sync::Arc;

use ramag_app::ApiService;
use ramag_domain::entities::{
    ApiCollection, ApiEnvironment, ApiParameter, ApiRequestRecord, ApiVariableExtraction,
    ApiVariableSource, ApiWorkspace, GrpcRequestSpec, HttpRequestSpec,
};
use ramag_domain::traits::Storage;
use ramag_infra_api::{GrpcApiDriver, HttpApiDriver};

pub(super) fn service(storage: Arc<dyn Storage>) -> anyhow::Result<Arc<ApiService>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let service = Arc::new(ApiService::new(
        Arc::new(HttpApiDriver::new()?),
        Arc::new(GrpcApiDriver::new()?),
        storage,
    )?);
    let mut workspace = ApiWorkspace::new("本机 API 切换验收");
    let mut environment = ApiEnvironment::new("local");
    environment
        .variables
        .insert("switch_marker".into(), "baseline".into());
    workspace.default_environment_id = Some(environment.id.clone());
    workspace.environments.push(environment);
    let mut collection = ApiCollection::new("隔离请求");
    let mut slow_http = ApiRequestRecord::new_http(
        "A 延迟 HTTP",
        HttpRequestSpec::new("GET", "http://127.0.0.1:18089/delay?ms=5000"),
    );
    slow_http.response_variables.push(ApiVariableExtraction {
        name: "switch_marker".into(),
        source: ApiVariableSource::Body,
        sensitive: false,
    });
    collection.requests.push(slow_http);
    let mut normal_http = HttpRequestSpec::new("GET", "http://127.0.0.1:18089/json");
    normal_http.timeout_millis = 37_000;
    normal_http
        .query
        .push(ApiParameter::new("preview", "public", false));
    normal_http
        .query
        .push(ApiParameter::new("preview", "sensitive", true));
    normal_http
        .query
        .push(ApiParameter::new("source", "local-preview", false));
    normal_http
        .headers
        .push(ApiParameter::new("X-Preview-Mode", "local", false));
    normal_http
        .headers
        .push(ApiParameter::new("X-Preview-Sensitive", "local-only", true));
    collection
        .requests
        .push(ApiRequestRecord::new_http("B 正常 HTTP", normal_http));
    for (name, message, extracts_marker) in [
        ("C 延迟 gRPC", r#"{"message":"delay:5000"}"#, true),
        ("D 正常 gRPC", r#"{"message":"preview"}"#, false),
    ] {
        let mut spec = GrpcRequestSpec::new("http://127.0.0.1:18090", "api.docker.Echo", "Unary");
        spec.message = message.into();
        if !extracts_marker {
            spec.timeout_millis = 43_000;
        }
        spec.metadata
            .push(ApiParameter::new("x-request", "docker", !extracts_marker));
        if !extracts_marker {
            spec.metadata
                .push(ApiParameter::new("x-roundtrip", "first-public", false));
            spec.metadata
                .push(ApiParameter::new("x-roundtrip", "second-sensitive", true));
            spec.metadata
                .push(ApiParameter::new("x-tail", "preview-tail", false));
        }
        let mut record = ApiRequestRecord::new_grpc(name, spec);
        if extracts_marker {
            record.response_variables.push(ApiVariableExtraction {
                name: "switch_marker".into(),
                source: ApiVariableSource::Body,
                sensitive: false,
            });
        }
        collection.requests.push(record);
    }
    workspace.collections.push(collection);
    futures::executor::block_on(service.save_workspace(&workspace))?;
    Ok(service)
}
