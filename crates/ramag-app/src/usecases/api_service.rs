//! API 工作台应用服务。
//!
//! 该服务负责把请求交给对应协议驱动，并把工作区保存交给 Storage；UI 不直接持有
//! `reqwest`、`tonic` 或 redb 类型。请求执行使用调用方提供的变量和取消标记。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ramag_domain::entities::{
    ApiCancellation, ApiEnvironment, ApiExecutionOutcome, ApiGrpcDiscoverySpec,
    ApiGrpcServiceSummary, ApiHistoryRecord, ApiRequestRecord, ApiRequestSpec, ApiResponseSnapshot,
    ApiWorkspace, ApiWorkspaceId,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::{ApiDriver, Storage};

#[path = "api_service_collection.rs"]
mod collection;
#[path = "api_service_import.rs"]
mod import;
#[path = "api_service_outcome.rs"]
mod outcome;
#[path = "api_service_request.rs"]
mod request;

/// API 工作台的协议执行与本地工作区编排服务。
pub struct ApiService {
    http_driver: Arc<dyn ApiDriver>,
    grpc_driver: Arc<dyn ApiDriver>,
    storage: Arc<dyn Storage>,
}

impl ApiService {
    /// 创建服务；两个驱动必须分别声明 HTTP 和 gRPC 协议，避免配置错误延迟到 UI 操作时才暴露。
    pub fn new(
        http_driver: Arc<dyn ApiDriver>,
        grpc_driver: Arc<dyn ApiDriver>,
        storage: Arc<dyn Storage>,
    ) -> Result<Self> {
        if http_driver.protocol() != ramag_domain::entities::ApiProtocol::Http {
            return Err(DomainError::InvalidConfig(
                "API HTTP 驱动协议声明不正确".into(),
            ));
        }
        if grpc_driver.protocol() != ramag_domain::entities::ApiProtocol::Grpc {
            return Err(DomainError::InvalidConfig(
                "API gRPC 驱动协议声明不正确".into(),
            ));
        }
        Ok(Self {
            http_driver,
            grpc_driver,
            storage,
        })
    }

    /// 按请求协议选择驱动并执行；请求校验和取消传播保持在应用边界。
    pub async fn execute(
        &self,
        request: &ApiRequestSpec,
        variables: &BTreeMap<String, String>,
        cancelled: ApiCancellation,
    ) -> Result<ApiResponseSnapshot> {
        request.validate().map_err(DomainError::InvalidConfig)?;
        let driver = self.driver_for(request).clone();
        let request = request.clone();
        let variables = variables.clone();
        // GPUI 的后台执行器不保证存在 Tokio reactor；HTTP/gRPC 驱动统一在受控 app worker
        // 中建立 current-thread runtime，避免 UI 发送时触发“no reactor running” panic。
        crate::run_blocking(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    DomainError::Other(format!("初始化 API Tokio runtime 失败：{error}"))
                })?;
            runtime.block_on(driver.execute(&request, &variables, cancelled))
        })
        .await
    }

    /// 通过 gRPC 驱动读取 Service/Method 目录；发现请求不写入工作区或执行历史。
    pub async fn discover_grpc_services(
        &self,
        request: &ApiGrpcDiscoverySpec,
        variables: &BTreeMap<String, String>,
        cancelled: ApiCancellation,
    ) -> Result<Vec<ApiGrpcServiceSummary>> {
        request.validate().map_err(DomainError::InvalidConfig)?;
        let driver = self.grpc_driver.clone();
        let request = request.clone();
        let variables = variables.clone();
        crate::run_blocking(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    DomainError::Other(format!("初始化 API Tokio runtime 失败：{error}"))
                })?;
            runtime.block_on(driver.discover_grpc_services(&request, &variables, cancelled))
        })
        .await
    }

    /// 展开环境变量、执行请求、评估断言并写入有界历史；用户级失败保留在 outcome 中。
    pub async fn execute_record(
        &self,
        workspace_id: &ApiWorkspaceId,
        record: &ApiRequestRecord,
        environment: &mut ApiEnvironment,
        cancelled: ApiCancellation,
    ) -> Result<ApiExecutionOutcome> {
        record.validate().map_err(DomainError::InvalidConfig)?;
        environment.validate().map_err(DomainError::InvalidConfig)?;

        let failure = if cancelled.load(Ordering::Relaxed) {
            Some(DomainError::Cancelled("API 请求已取消".into()))
        } else {
            None
        };
        let execution = match failure {
            Some(error) => Err(error),
            None => {
                match request::resolve_request(&record.request, &environment.execution_variables())
                {
                    Ok(request) => self.execute(&request, &BTreeMap::new(), cancelled).await,
                    Err(error) => Err(error),
                }
            }
        };

        let outcome = match execution {
            Ok(snapshot) => match outcome::build_success_outcome(record, environment, snapshot) {
                Ok(outcome) => outcome,
                Err(error) => {
                    outcome::failure_outcome(record, environment, &error.to_string(), false)
                }
            },
            Err(error) => {
                let cancelled = matches!(error, DomainError::Cancelled(_));
                let message = error.to_string();
                outcome::failure_outcome(record, environment, &message, cancelled)
            }
        };
        self.storage
            .append_api_history(workspace_id, &outcome.history)
            .await?;
        Ok(outcome)
    }

    /// 读取本地 API 工作区列表，Storage 负责解密和完整性校验。
    pub async fn list_workspaces(&self) -> Result<Vec<ApiWorkspace>> {
        self.storage.list_api_workspaces().await
    }

    /// 保存完整 API 工作区；保存前再次执行聚合校验，避免 UI 草稿绕过领域限制。
    pub async fn save_workspace(&self, workspace: &ApiWorkspace) -> Result<()> {
        workspace.validate().map_err(DomainError::InvalidConfig)?;
        self.storage.save_api_workspace(workspace).await
    }

    /// 删除一个本地 API 工作区。
    pub async fn delete_workspace(&self, id: &ApiWorkspaceId) -> Result<()> {
        self.storage.delete_api_workspace(id).await
    }

    /// 读取工作区最近执行摘要；正文和敏感值由 Storage 保持有界并加密。
    pub async fn list_history(
        &self,
        workspace_id: &ApiWorkspaceId,
        limit: usize,
    ) -> Result<Vec<ApiHistoryRecord>> {
        self.storage.list_api_history(workspace_id, limit).await
    }

    pub async fn clear_history(&self, workspace_id: &ApiWorkspaceId) -> Result<()> {
        self.storage.clear_api_history(workspace_id).await
    }

    fn driver_for(&self, request: &ApiRequestSpec) -> &Arc<dyn ApiDriver> {
        match request.protocol() {
            ramag_domain::entities::ApiProtocol::Http => &self.http_driver,
            ramag_domain::entities::ApiProtocol::Grpc => &self.grpc_driver,
        }
    }
}

/// 创建一个尚未取消的执行标记，供 UI 的发送操作和后续取消按钮共用。
pub fn new_api_cancellation() -> ApiCancellation {
    Arc::new(AtomicBool::new(false))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use ramag_domain::entities::{
        ApiAssertion, ApiBody, ApiBodyMode, ApiCollection, ApiEnvironment, ApiMultipartPart,
        ApiMultipartValue, ApiProtocol, ApiProxyConfig, ApiRequestRecord, ApiRequestSpec,
        ApiResponseSnapshot, ApiResponseSnapshotParts, ApiResponseStatus, ApiWorkspace,
        GrpcRequestSpec, HttpRequestSpec,
    };
    use ramag_domain::error::Result;
    use ramag_domain::traits::ApiDriver;
    use tempfile::tempdir;

    use super::{ApiService, new_api_cancellation};

    struct RecordingDriver {
        protocol: ApiProtocol,
        calls: AtomicUsize,
    }

    #[test]
    fn resolves_multipart_body_templates_without_downgrading_to_text()
    -> std::result::Result<(), String> {
        let mut spec = HttpRequestSpec::new("POST", "http://127.0.0.1/upload");
        spec.body = Some(ApiBody::multipart(vec![
            ApiMultipartPart {
                name: "{{field_name}}".into(),
                value: ApiMultipartValue::Text {
                    value: "{{field_value}}".into(),
                },
                content_type: Some("text/{{format}}".into()),
                sensitive: true,
            },
            ApiMultipartPart {
                name: "file".into(),
                value: ApiMultipartValue::File {
                    path: "{{file_path}}".into(),
                    file_name: Some("{{file_name}}".into()),
                },
                content_type: Some("application/octet-stream".into()),
                sensitive: false,
            },
        ]));
        let variables = BTreeMap::from([
            ("field_name".into(), "title".into()),
            ("field_value".into(), "private".into()),
            ("format".into(), "plain".into()),
            ("file_path".into(), "/tmp/upload.txt".into()),
            ("file_name".into(), "upload.txt".into()),
        ]);

        let resolved = super::request::resolve_http_request(&spec, &variables)
            .map_err(|error| error.to_string())?;
        let body = resolved
            .body
            .ok_or_else(|| "Multipart 正文不应丢失".to_string())?;
        assert_eq!(body.mode, ApiBodyMode::Multipart);
        assert_eq!(body.multipart.len(), 2);
        assert_eq!(body.multipart[0].name, "title");
        assert_eq!(
            body.multipart[0].content_type.as_deref(),
            Some("text/plain")
        );
        assert!(matches!(
            body.multipart[0].value,
            ApiMultipartValue::Text { ref value } if value == "private"
        ));
        assert!(matches!(
            body.multipart[1].value,
            ApiMultipartValue::File { ref path, ref file_name }
                if path == "/tmp/upload.txt" && file_name.as_deref() == Some("upload.txt")
        ));
        Ok(())
    }

    #[test]
    fn resolves_proxy_templates_and_rejects_unsafe_proxy_values() -> std::result::Result<(), String>
    {
        let mut spec = HttpRequestSpec::new("GET", "http://127.0.0.1/status");
        spec.proxy = ApiProxyConfig {
            url: Some("http://{{proxy_host}}:{{proxy_port}}".into()),
            username: Some("{{proxy_user}}".into()),
            password: Some("{{proxy_password}}".into()),
        };
        let variables = BTreeMap::from([
            ("proxy_host".into(), "127.0.0.1".into()),
            ("proxy_port".into(), "18093".into()),
            ("proxy_user".into(), "user".into()),
            ("proxy_password".into(), "secret".into()),
        ]);

        let resolved = super::request::resolve_http_request(&spec, &variables)
            .map_err(|error| error.to_string())?;
        assert_eq!(
            resolved.proxy.url.as_deref(),
            Some("http://127.0.0.1:18093")
        );
        assert_eq!(resolved.proxy.username.as_deref(), Some("user"));
        assert_eq!(resolved.proxy.password.as_deref(), Some("secret"));

        let unsafe_variables = BTreeMap::from([
            ("proxy_host".into(), "127.0.0.1".into()),
            ("proxy_port".into(), "18093/path".into()),
            ("proxy_user".into(), "user".into()),
            ("proxy_password".into(), "secret".into()),
        ]);
        assert!(super::request::resolve_http_request(&spec, &unsafe_variables).is_err());
        Ok(())
    }

    #[async_trait]
    impl ApiDriver for RecordingDriver {
        fn protocol(&self) -> ApiProtocol {
            self.protocol
        }

        async fn execute(
            &self,
            request: &ApiRequestSpec,
            _variables: &std::collections::BTreeMap<String, String>,
            _cancelled: ramag_domain::entities::ApiCancellation,
        ) -> Result<ApiResponseSnapshot> {
            assert_eq!(request.protocol(), self.protocol);
            self.calls.fetch_add(1, Ordering::Relaxed);
            ApiResponseSnapshot::new(ApiResponseSnapshotParts {
                protocol: self.protocol,
                status: match self.protocol {
                    ApiProtocol::Http => ApiResponseStatus::Http { code: 200 },
                    ApiProtocol::Grpc => ApiResponseStatus::Grpc { code: "ok".into() },
                },
                headers: Vec::new(),
                metadata: Vec::new(),
                body: b"recorded".to_vec(),
                elapsed_millis: 1,
                size_bytes: 8,
                truncated: false,
                error: None,
            })
            .map_err(ramag_domain::error::DomainError::InvalidConfig)
        }
    }

    #[tokio::test]
    async fn routes_both_protocols_and_persists_workspace() {
        let directory = tempdir().expect("创建 API 服务测试目录");
        let storage = ramag_infra_storage::RedbStorage::open_with_key(
            &directory.path().join("api.redb"),
            &[0x45; 32],
        )
        .expect("打开 API 服务测试存储");
        let http_driver = std::sync::Arc::new(RecordingDriver {
            protocol: ApiProtocol::Http,
            calls: AtomicUsize::new(0),
        });
        let grpc_driver = std::sync::Arc::new(RecordingDriver {
            protocol: ApiProtocol::Grpc,
            calls: AtomicUsize::new(0),
        });
        let service = ApiService::new(
            http_driver.clone(),
            grpc_driver.clone(),
            std::sync::Arc::new(storage),
        )
        .expect("API 服务应接受匹配的协议驱动");

        let http = service
            .execute(
                &ApiRequestSpec::Http(HttpRequestSpec::new("GET", "http://127.0.0.1")),
                &Default::default(),
                new_api_cancellation(),
            )
            .await
            .expect("HTTP 请求应路由到 HTTP 驱动");
        let grpc = service
            .execute(
                &ApiRequestSpec::Grpc(GrpcRequestSpec::new(
                    "http://127.0.0.1:18090",
                    "api.docker.Echo",
                    "Unary",
                )),
                &Default::default(),
                new_api_cancellation(),
            )
            .await
            .expect("gRPC 请求应路由到 gRPC 驱动");
        assert_eq!(http.status, ApiResponseStatus::Http { code: 200 });
        assert_eq!(grpc.status, ApiResponseStatus::Grpc { code: "ok".into() });
        assert_eq!(http_driver.calls.load(Ordering::Relaxed), 1);
        assert_eq!(grpc_driver.calls.load(Ordering::Relaxed), 1);

        let workspace = ApiWorkspace::new("API 测试工作区");
        service
            .save_workspace(&workspace)
            .await
            .expect("工作区应保存");
        assert_eq!(
            service.list_workspaces().await.expect("工作区应列出"),
            vec![workspace.clone()]
        );

        let mut imported = ApiWorkspace::new("Imported API");
        let mut collection = ApiCollection::new("Imported Collection");
        collection.requests.push(ApiRequestRecord::new_http(
            "Imported Health",
            HttpRequestSpec::new("GET", "http://127.0.0.1/health"),
        ));
        imported.collections.push(collection);
        let raw = serde_json::to_string(&imported).expect("导入工作区应序列化");
        let (merged, summary) = service
            .import_workspace_json(&workspace, &raw)
            .await
            .expect("导入工作区应保存");
        assert_eq!(summary.request_count, 1);
        assert_eq!(merged.collections[0].requests[0].name, "Imported Health");
        assert_eq!(
            service.list_workspaces().await.expect("导入后工作区应列出"),
            vec![merged]
        );
    }

    #[tokio::test]
    async fn execute_record_covers_assertion_failure_missing_variable_and_cancel() {
        let directory = tempdir().expect("创建 API 执行测试目录");
        let storage = ramag_infra_storage::RedbStorage::open_with_key(
            &directory.path().join("api-execution.redb"),
            &[0x46; 32],
        )
        .expect("打开 API 执行测试存储");
        let http_driver = std::sync::Arc::new(RecordingDriver {
            protocol: ApiProtocol::Http,
            calls: AtomicUsize::new(0),
        });
        let grpc_driver = std::sync::Arc::new(RecordingDriver {
            protocol: ApiProtocol::Grpc,
            calls: AtomicUsize::new(0),
        });
        let service = ApiService::new(
            http_driver.clone(),
            grpc_driver,
            std::sync::Arc::new(storage),
        )
        .expect("API 服务应创建");
        let workspace = ApiWorkspace::new("history");
        let mut environment = ApiEnvironment::new("local");

        let mut passing =
            ApiRequestRecord::new_http("passing", HttpRequestSpec::new("GET", "http://127.0.0.1"));
        passing.assertions.push(ApiAssertion::BodyContains {
            expected: "recorded".into(),
        });
        let passed = service
            .execute_record(
                &workspace.id,
                &passing,
                &mut environment,
                new_api_cancellation(),
            )
            .await
            .expect("成功执行应返回 outcome");
        assert!(passed.result.as_ref().is_some_and(|result| result.passed));
        assert!(passed.history.passed);

        let mut failing = passing.clone();
        failing.name = "failing".into();
        failing.assertions = vec![ApiAssertion::HttpStatus { expected: 201 }];
        let failed = service
            .execute_record(
                &workspace.id,
                &failing,
                &mut environment,
                new_api_cancellation(),
            )
            .await
            .expect("断言失败应返回 outcome");
        assert!(failed.result.is_some());
        assert!(!failed.result.expect("应有响应").passed);
        assert!(!failed.history.passed);

        let missing = ApiRequestRecord::new_http(
            "missing",
            HttpRequestSpec::new("GET", "{{missing_url}}/health"),
        );
        let missing_outcome = service
            .execute_record(
                &workspace.id,
                &missing,
                &mut environment,
                new_api_cancellation(),
            )
            .await
            .expect("变量缺失应返回 outcome");
        assert!(missing_outcome.result.is_none());
        assert!(
            missing_outcome
                .error
                .as_deref()
                .is_some_and(|error| error.contains("缺少 API 环境变量"))
        );

        let cancelled = new_api_cancellation();
        cancelled.store(true, Ordering::Relaxed);
        let cancelled_outcome = service
            .execute_record(&workspace.id, &passing, &mut environment, cancelled)
            .await
            .expect("取消应返回 outcome");
        assert!(
            cancelled_outcome
                .error
                .as_deref()
                .is_some_and(|error| error.contains("取消"))
        );
        assert!(cancelled_outcome.cancelled);
        assert_eq!(http_driver.calls.load(Ordering::Relaxed), 2);
        assert_eq!(
            service.list_history(&workspace.id, 10).await.unwrap().len(),
            4
        );
    }
}
