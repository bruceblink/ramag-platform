use std::sync::atomic::Ordering;

use ramag_domain::entities::{
    ApiCancellation, ApiCollection, ApiCollectionRunResult, ApiEnvironment, ApiWorkspaceId,
};
use ramag_domain::error::{DomainError, Result};

use super::ApiService;

impl ApiService {
    /// 按保存顺序串行执行 Collection；取消会阻止下一个请求启动并保留已完成结果。
    pub async fn run_collection(
        &self,
        workspace_id: &ApiWorkspaceId,
        collection: &ApiCollection,
        environment: &mut ApiEnvironment,
        cancelled: ApiCancellation,
    ) -> Result<ApiCollectionRunResult> {
        collection.validate().map_err(DomainError::InvalidConfig)?;
        environment.validate().map_err(DomainError::InvalidConfig)?;
        let mut summary = ApiCollectionRunResult::empty(collection);
        for record in &collection.requests {
            if cancelled.load(Ordering::Relaxed) {
                summary.stopped = true;
                break;
            }
            let outcome = self
                .execute_record(workspace_id, record, environment, cancelled.clone())
                .await?;
            let was_cancelled = outcome.cancelled;
            let history_persist_failed = !outcome.history_persisted;
            summary.push(outcome);
            if history_persist_failed {
                summary.history_persist_failed = true;
                summary.stopped = true;
                break;
            }
            if was_cancelled {
                summary.stopped = true;
                break;
            }
        }
        if cancelled.load(Ordering::Relaxed) && summary.outcomes.len() < collection.requests.len() {
            summary.stopped = true;
        }
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use ramag_domain::entities::{
        ApiCancellation, ApiCollection, ApiEnvironment, ApiHistoryRecord, ApiProtocol,
        ApiRequestRecord, ApiRequestSpec, ApiResponseSnapshot, ApiResponseSnapshotParts,
        ApiResponseStatus, ApiWorkspace, ConnectionConfig, ConnectionId, HttpRequestSpec,
        QueryRecord, QueryRecordId,
    };
    use ramag_domain::error::{DomainError, Result};
    use ramag_domain::traits::{ApiDriver, Storage};
    use tempfile::{TempDir, tempdir};

    use super::super::new_api_cancellation;
    use super::ApiService;

    struct HistoryStorage {
        fail_after: usize,
        calls: std::sync::atomic::AtomicUsize,
        records: Arc<Mutex<Vec<ApiHistoryRecord>>>,
    }

    #[async_trait]
    impl Storage for HistoryStorage {
        async fn list_connections(&self) -> Result<Vec<ConnectionConfig>> {
            Ok(Vec::new())
        }

        async fn get_connection(&self, _id: &ConnectionId) -> Result<Option<ConnectionConfig>> {
            Ok(None)
        }

        async fn save_connection(&self, _config: &ConnectionConfig) -> Result<()> {
            Ok(())
        }

        async fn delete_connection(&self, _id: &ConnectionId) -> Result<()> {
            Ok(())
        }

        async fn append_api_history(
            &self,
            _workspace_id: &ramag_domain::entities::ApiWorkspaceId,
            record: &ApiHistoryRecord,
        ) -> Result<()> {
            let call = self.calls.fetch_add(1, Ordering::Relaxed);
            if call >= self.fail_after {
                return Err(DomainError::Storage("history-secret-sentinel".into()));
            }
            self.records
                .lock()
                .expect("记录测试历史")
                .push(record.clone());
            Ok(())
        }

        async fn list_api_history(
            &self,
            _workspace_id: &ramag_domain::entities::ApiWorkspaceId,
            _limit: usize,
        ) -> Result<Vec<ApiHistoryRecord>> {
            Ok(self.records.lock().expect("读取测试历史").clone())
        }

        async fn append_history(&self, _record: &QueryRecord) -> Result<()> {
            Ok(())
        }

        async fn list_history(
            &self,
            _connection_id: Option<&ConnectionId>,
            _limit: usize,
        ) -> Result<Vec<QueryRecord>> {
            Ok(Vec::new())
        }

        async fn delete_history(&self, _id: &QueryRecordId) -> Result<()> {
            Ok(())
        }

        async fn clear_history(&self, _connection_id: Option<&ConnectionId>) -> Result<()> {
            Ok(())
        }

        async fn get_preference(&self, _key: &str) -> Result<Option<String>> {
            Ok(None)
        }

        async fn set_preference(&self, _key: &str, _value: &str) -> Result<()> {
            Ok(())
        }
    }

    struct OrderedDriver {
        protocol: ApiProtocol,
        calls: Arc<Mutex<Vec<String>>>,
        cancel_after_first: bool,
        active_cancellation: Arc<Mutex<Option<ApiCancellation>>>,
    }

    #[async_trait]
    impl ApiDriver for OrderedDriver {
        fn protocol(&self) -> ApiProtocol {
            self.protocol
        }

        async fn execute(
            &self,
            request: &ApiRequestSpec,
            _variables: &std::collections::BTreeMap<String, String>,
            _cancelled: ApiCancellation,
        ) -> Result<ApiResponseSnapshot> {
            let ApiRequestSpec::Http(spec) = request else {
                return Err(DomainError::InvalidConfig(
                    "测试驱动收到非 HTTP 请求".into(),
                ));
            };
            let call_count = {
                let mut calls = self.calls.lock().expect("读取 Collection 调用顺序");
                calls.push(spec.url_template.clone());
                calls.len()
            };
            if self.cancel_after_first
                && call_count == 1
                && let Some(cancelled) = self
                    .active_cancellation
                    .lock()
                    .expect("读取 Collection 取消标记")
                    .clone()
            {
                cancelled.store(true, Ordering::Relaxed);
            }
            ApiResponseSnapshot::new(ApiResponseSnapshotParts {
                protocol: ApiProtocol::Http,
                status: ApiResponseStatus::Http { code: 200 },
                headers: Vec::new(),
                metadata: Vec::new(),
                body: spec.url_template.as_bytes().to_vec(),
                elapsed_millis: 1,
                size_bytes: spec.url_template.len() as u64,
                truncated: false,
                error: None,
            })
            .map_err(DomainError::InvalidConfig)
        }
    }

    type CollectionTestService = (
        ApiService,
        Arc<Mutex<Vec<String>>>,
        Arc<Mutex<Option<ApiCancellation>>>,
        TempDir,
    );
    type CollectionServiceState = (
        ApiService,
        Arc<Mutex<Vec<String>>>,
        Arc<Mutex<Option<ApiCancellation>>>,
    );

    fn build_service(cancel_after_first: bool) -> CollectionTestService {
        let directory = tempdir().expect("创建 Collection 测试目录");
        let storage = ramag_infra_storage::RedbStorage::open_with_key(
            &directory.path().join("api-collection.redb"),
            &[0x49; 32],
        )
        .expect("打开 Collection 测试存储");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let active_cancellation = Arc::new(Mutex::new(None));
        let http_driver: Arc<dyn ApiDriver> = Arc::new(OrderedDriver {
            protocol: ApiProtocol::Http,
            calls: calls.clone(),
            cancel_after_first,
            active_cancellation: active_cancellation.clone(),
        });
        let grpc_driver: Arc<dyn ApiDriver> = Arc::new(OrderedDriver {
            protocol: ApiProtocol::Grpc,
            calls: Arc::new(Mutex::new(Vec::new())),
            cancel_after_first: false,
            active_cancellation: Arc::new(Mutex::new(None)),
        });
        let service = ApiService::new(http_driver, grpc_driver, Arc::new(storage))
            .expect("创建 Collection 测试服务");
        (service, calls, active_cancellation, directory)
    }

    fn build_service_with_storage(
        cancel_after_first: bool,
        storage: Arc<dyn Storage>,
    ) -> CollectionServiceState {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let active_cancellation = Arc::new(Mutex::new(None));
        let http_driver: Arc<dyn ApiDriver> = Arc::new(OrderedDriver {
            protocol: ApiProtocol::Http,
            calls: calls.clone(),
            cancel_after_first,
            active_cancellation: active_cancellation.clone(),
        });
        let grpc_driver: Arc<dyn ApiDriver> = Arc::new(OrderedDriver {
            protocol: ApiProtocol::Grpc,
            calls: Arc::new(Mutex::new(Vec::new())),
            cancel_after_first: false,
            active_cancellation: Arc::new(Mutex::new(None)),
        });
        let service = ApiService::new(http_driver, grpc_driver, storage)
            .expect("创建自定义 Storage 的 Collection 测试服务");
        (service, calls, active_cancellation)
    }

    fn collection_with_requests() -> ApiCollection {
        let mut collection = ApiCollection::new("顺序 Collection");
        collection.requests = ["first", "second", "third"]
            .into_iter()
            .map(|name| {
                ApiRequestRecord::new_http(
                    name,
                    HttpRequestSpec::new("GET", format!("http://{name}")),
                )
            })
            .collect();
        collection
    }

    #[tokio::test]
    async fn runs_collection_in_order_and_persists_each_history_record() {
        let (service, calls, active_cancellation, _directory) = build_service(false);
        let cancellation = new_api_cancellation();
        *active_cancellation
            .lock()
            .expect("设置 Collection 取消标记") = Some(cancellation.clone());
        let workspace = ApiWorkspace::new("Collection 工作区");
        let mut environment = ApiEnvironment::new("local");
        let summary = service
            .run_collection(
                &workspace.id,
                &collection_with_requests(),
                &mut environment,
                cancellation,
            )
            .await
            .expect("Collection 应执行");

        assert_eq!(
            *calls.lock().expect("读取 Collection 调用顺序"),
            vec![
                "http://first".to_string(),
                "http://second".to_string(),
                "http://third".to_string()
            ]
        );
        assert_eq!(summary.outcomes.len(), 3);
        assert_eq!(summary.passed, 3);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.cancelled, 0);
        assert!(!summary.stopped);
        assert_eq!(
            service
                .list_history(&workspace.id, 10)
                .await
                .expect("读取 Collection 历史")
                .len(),
            3
        );
    }

    #[tokio::test]
    async fn stops_collection_before_starting_the_next_request_after_cancellation() {
        let (service, calls, active_cancellation, _directory) = build_service(true);
        let cancellation = new_api_cancellation();
        *active_cancellation
            .lock()
            .expect("设置 Collection 取消标记") = Some(cancellation.clone());
        let workspace = ApiWorkspace::new("取消 Collection");
        let mut environment = ApiEnvironment::new("local");
        let summary = service
            .run_collection(
                &workspace.id,
                &collection_with_requests(),
                &mut environment,
                cancellation,
            )
            .await
            .expect("取消后的 Collection 应返回部分结果");

        assert_eq!(summary.outcomes.len(), 1);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.cancelled, 0);
        assert!(summary.stopped);
        assert_eq!(calls.lock().expect("读取取消后的调用顺序").len(), 1);
    }

    #[tokio::test]
    async fn returns_stopped_summary_when_collection_is_already_cancelled() {
        let (service, calls, _active_cancellation, _directory) = build_service(false);
        let cancellation = new_api_cancellation();
        cancellation.store(true, Ordering::Relaxed);
        let workspace = ApiWorkspace::new("预取消 Collection");
        let mut environment = ApiEnvironment::new("local");
        let summary = service
            .run_collection(
                &workspace.id,
                &collection_with_requests(),
                &mut environment,
                cancellation,
            )
            .await
            .expect("预取消 Collection 应返回汇总");

        assert!(summary.outcomes.is_empty());
        assert!(summary.stopped);
        assert!(calls.lock().expect("读取预取消调用次数").is_empty());
    }

    #[tokio::test]
    async fn preserves_completed_outcome_when_history_persistence_fails() {
        let records = Arc::new(Mutex::new(Vec::new()));
        let storage: Arc<dyn Storage> = Arc::new(HistoryStorage {
            fail_after: 1,
            calls: std::sync::atomic::AtomicUsize::new(0),
            records: records.clone(),
        });
        let (service, calls, _active_cancellation) = build_service_with_storage(false, storage);
        let workspace = ApiWorkspace::new("历史失败 Collection");
        let mut environment = ApiEnvironment::new("local");
        let summary = service
            .run_collection(
                &workspace.id,
                &collection_with_requests(),
                &mut environment,
                new_api_cancellation(),
            )
            .await
            .expect("历史写入失败仍应返回部分汇总");

        assert_eq!(calls.lock().expect("读取历史失败调用顺序").len(), 2);
        assert_eq!(summary.outcomes.len(), 2);
        assert_eq!(summary.passed, 2);
        assert_eq!(summary.failed, 0);
        assert!(summary.stopped);
        assert!(summary.history_persist_failed);
        assert!(summary.outcomes[0].history_persisted);
        assert!(!summary.outcomes[1].history_persisted);
        assert_eq!(records.lock().expect("读取已保存历史").len(), 1);
    }
}
