//! API 工作台应用服务。
//!
//! 该服务负责把请求交给对应协议驱动，并把工作区保存交给 Storage；UI 不直接持有
//! `reqwest`、`tonic` 或 redb 类型。请求执行使用调用方提供的变量和取消标记。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ramag_domain::entities::{
    ApiCancellation, ApiRequestSpec, ApiResponseSnapshot, ApiWorkspace, ApiWorkspaceId,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::{ApiDriver, Storage};

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
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use ramag_domain::entities::{
        ApiProtocol, ApiRequestSpec, ApiResponseSnapshot, ApiResponseSnapshotParts,
        ApiResponseStatus, ApiWorkspace, GrpcRequestSpec, HttpRequestSpec,
    };
    use ramag_domain::error::Result;
    use ramag_domain::traits::ApiDriver;
    use tempfile::tempdir;

    use super::{ApiService, new_api_cancellation};

    struct RecordingDriver {
        protocol: ApiProtocol,
        calls: AtomicUsize,
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
            vec![workspace]
        );
    }
}
