use super::*;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use gpui_kit::{
    IntoElement, Modifiers, Render, TestAppContext, VisualTestContext, Window, div, point, px, size,
};
use ramag_app::ApiService;
use ramag_domain::entities::{
    ApiCancellation, ApiCollection, ApiEnvironment, ApiGrpcDiscoverySpec, ApiGrpcMethodSummary,
    ApiGrpcServiceSummary, ApiProtocol, ApiRequestRecord, ApiRequestSpec, ApiResponseSnapshot,
    ApiResponseSnapshotParts, ApiResponseStatus, ApiVariableExtraction, ApiVariableSource,
    ApiWorkspace, GrpcRequestSpec, HttpRequestSpec,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::{ApiDriver, Storage};
use ramag_infra_storage::RedbStorage;
use tempfile::TempDir;

struct ApiViewTestHost {
    view: Entity<ApiView>,
}

impl Render for ApiViewTestHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().relative().size_full().child(self.view.clone())
    }
}

pub(super) struct TestStorage {
    pub(super) service: Arc<ApiService>,
    _directory: TempDir,
}

pub(super) struct DriverControl {
    pub(super) slow_started: async_channel::Receiver<String>,
    pub(super) release_slow: async_channel::Sender<()>,
    pub(super) slow_finished: async_channel::Receiver<String>,
    pub(super) discovery_started: async_channel::Receiver<()>,
    pub(super) release_discovery: async_channel::Sender<()>,
}

struct ControlledApiDriver {
    protocol: ApiProtocol,
    slow_started: async_channel::Sender<String>,
    release_slow: async_channel::Receiver<()>,
    slow_finished: async_channel::Sender<String>,
    discovery_started: async_channel::Sender<()>,
    release_discovery: async_channel::Receiver<()>,
    delay_next_slow: AtomicBool,
    sequence: AtomicUsize,
}

fn controlled_driver(protocol: ApiProtocol) -> (Arc<ControlledApiDriver>, DriverControl) {
    let (slow_started_tx, slow_started_rx) = async_channel::bounded(2);
    let (release_slow_tx, release_slow_rx) = async_channel::bounded(2);
    let (slow_finished_tx, slow_finished_rx) = async_channel::bounded(2);
    let (discovery_started_tx, discovery_started_rx) = async_channel::bounded(1);
    let (release_discovery_tx, release_discovery_rx) = async_channel::bounded(1);
    (
        Arc::new(ControlledApiDriver {
            protocol,
            slow_started: slow_started_tx,
            release_slow: release_slow_rx,
            slow_finished: slow_finished_tx,
            discovery_started: discovery_started_tx,
            release_discovery: release_discovery_rx,
            delay_next_slow: AtomicBool::new(true),
            sequence: AtomicUsize::new(0),
        }),
        DriverControl {
            slow_started: slow_started_rx,
            release_slow: release_slow_tx,
            slow_finished: slow_finished_rx,
            discovery_started: discovery_started_rx,
            release_discovery: release_discovery_tx,
        },
    )
}

#[async_trait]
impl ApiDriver for ControlledApiDriver {
    fn protocol(&self) -> ApiProtocol {
        self.protocol
    }

    async fn execute(
        &self,
        request: &ApiRequestSpec,
        _variables: &BTreeMap<String, String>,
        _cancelled: ApiCancellation,
    ) -> Result<ApiResponseSnapshot> {
        let label = match request {
            ApiRequestSpec::Http(spec) if self.protocol == ApiProtocol::Http => {
                spec.url_template.as_str()
            }
            ApiRequestSpec::Grpc(spec) if self.protocol == ApiProtocol::Grpc => {
                spec.message.as_str()
            }
            _ => {
                return Err(DomainError::InvalidConfig(
                    "受控 API 驱动收到错误协议".into(),
                ));
            }
        };
        let slow = label.contains("slow");
        if slow && self.delay_next_slow.swap(false, Ordering::Relaxed) {
            let _ = self.slow_started.send(label.to_string()).await;
            self.release_slow
                .recv()
                .await
                .map_err(|error| DomainError::Other(error.to_string()))?;
            let _ = self.slow_finished.send(label.to_string()).await;
        }
        if label.contains("slow-error") {
            return Err(DomainError::Other("受控延迟请求失败".into()));
        }

        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let marker = format!(
            "{}-{sequence}",
            if slow {
                match self.protocol {
                    ApiProtocol::Http => "slow-http",
                    ApiProtocol::Grpc => "slow-grpc",
                }
            } else {
                match self.protocol {
                    ApiProtocol::Http => "fast-http",
                    ApiProtocol::Grpc => "fast-grpc",
                }
            }
        );
        let body = serde_json::json!({ "marker": marker })
            .to_string()
            .into_bytes();
        ApiResponseSnapshot::new(ApiResponseSnapshotParts {
            protocol: self.protocol,
            status: match self.protocol {
                ApiProtocol::Http => ApiResponseStatus::Http { code: 200 },
                ApiProtocol::Grpc => ApiResponseStatus::Grpc { code: "ok".into() },
            },
            headers: Vec::new(),
            metadata: Vec::new(),
            size_bytes: body.len() as u64,
            body,
            elapsed_millis: 1,
            truncated: false,
            error: None,
        })
        .map_err(DomainError::InvalidConfig)
    }

    async fn discover_grpc_services(
        &self,
        _request: &ApiGrpcDiscoverySpec,
        _variables: &BTreeMap<String, String>,
        _cancelled: ApiCancellation,
    ) -> Result<Vec<ApiGrpcServiceSummary>> {
        if self.protocol != ApiProtocol::Grpc {
            return Err(DomainError::InvalidConfig(
                "HTTP 驱动不支持 gRPC Service 发现".into(),
            ));
        }
        let _ = self.discovery_started.send(()).await;
        self.release_discovery
            .recv()
            .await
            .map_err(|error| DomainError::Other(error.to_string()))?;
        Ok(vec![ApiGrpcServiceSummary {
            name: "delayed.Service".into(),
            methods: vec![ApiGrpcMethodSummary {
                name: "DelayedMethod".into(),
                client_streaming: false,
                server_streaming: false,
            }],
        }])
    }
}

pub(super) fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("控件应参与布局: {selector}"));
    let center = point(
        bounds.origin.x + bounds.size.width / 2.0,
        bounds.origin.y + bounds.size.height / 2.0,
    );
    cx.simulate_mouse_move(center, None, Modifiers::default());
    cx.simulate_mouse_down(center, gpui_kit::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(center, gpui_kit::MouseButton::Left, Modifiers::default());
}

fn add_response_variable(record: &mut ApiRequestRecord) {
    record.response_variables.push(ApiVariableExtraction {
        name: "marker".into(),
        source: ApiVariableSource::JsonPath {
            path: "$.marker".into(),
        },
        sensitive: false,
    });
}

fn test_workspace() -> ApiWorkspace {
    let mut workspace = ApiWorkspace::new("Request switch tests");
    let mut environment = ApiEnvironment::new("local");
    environment
        .variables
        .insert("marker".into(), "baseline".into());
    workspace.default_environment_id = Some(environment.id.clone());
    workspace.environments.push(environment);

    let mut collection = ApiCollection::new("Saved requests");
    let mut slow_http = ApiRequestRecord::new_http(
        "Slow HTTP",
        HttpRequestSpec::new("GET", "http://controlled/slow-http"),
    );
    add_response_variable(&mut slow_http);
    let mut fast_http = ApiRequestRecord::new_http(
        "Fast HTTP",
        HttpRequestSpec::new("GET", "http://controlled/fast-http"),
    );
    add_response_variable(&mut fast_http);
    let mut slow_grpc = ApiRequestRecord::new_grpc(
        "Slow gRPC",
        GrpcRequestSpec::new("http://controlled", "api.Test", "Unary")
            .with_message(r#"{"message":"slow-grpc"}"#.into()),
    );
    add_response_variable(&mut slow_grpc);
    let mut fast_grpc = ApiRequestRecord::new_grpc(
        "Fast gRPC",
        GrpcRequestSpec::new("http://controlled", "api.Test", "Unary")
            .with_message(r#"{"message":"fast-grpc"}"#.into()),
    );
    add_response_variable(&mut fast_grpc);
    let slow_error = ApiRequestRecord::new_http(
        "Slow error",
        HttpRequestSpec::new("GET", "http://controlled/slow-error"),
    );
    collection
        .requests
        .extend([slow_http, fast_http, slow_grpc, fast_grpc, slow_error]);
    workspace.collections.push(collection);
    workspace
}

pub(super) fn add_view(
    cx: &mut TestAppContext,
) -> (
    Entity<ApiView>,
    &mut VisualTestContext,
    TestStorage,
    DriverControl,
    DriverControl,
) {
    cx.update(gpui_kit::component::init);
    cx.executor().allow_parking();
    let directory = tempfile::tempdir().expect("创建切换测试目录");
    let storage: Arc<dyn Storage> = Arc::new(
        RedbStorage::open_with_key(&directory.path().join("api-switch.redb"), &[0x52; 32])
            .expect("打开切换测试存储"),
    );
    let (http_driver, http_control) = controlled_driver(ApiProtocol::Http);
    let (grpc_driver, grpc_control) = controlled_driver(ApiProtocol::Grpc);
    let service =
        Arc::new(ApiService::new(http_driver, grpc_driver, storage).expect("创建真实 API 服务"));
    futures::executor::block_on(service.save_workspace(&test_workspace()))
        .expect("保存 API 切换测试工作区");

    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApiView::with_service(service.clone(), window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| ApiViewTestHost { view });
        gpui_kit::component::Root::new(host, window, cx)
    });
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    let view = view_entity.expect("API 切换测试视图应创建");
    wait_for_workspace(&mut *visual_cx, &view);
    (
        view,
        visual_cx,
        TestStorage {
            service,
            _directory: directory,
        },
        http_control,
        grpc_control,
    )
}

pub(super) fn wait_for_workspace(cx: &mut VisualTestContext, view: &Entity<ApiView>) {
    for _ in 0..200 {
        cx.run_until_parked();
        if cx.update(|_, app| view.read(app).workspace_load_state) == ApiWorkspaceLoadState::Loaded
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("API 测试工作区未加载");
}

pub(super) fn wait_for_response(
    cx: &mut VisualTestContext,
    view: &Entity<ApiView>,
) -> ApiResponseSnapshot {
    for _ in 0..200 {
        cx.run_until_parked();
        if let Some(response) = cx.update(|_, app| view.read(app).response.clone()) {
            return response;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("API 测试响应未完成");
}

pub(super) fn wait_for_driver_event(
    cx: &mut VisualTestContext,
    event: &async_channel::Receiver<String>,
) -> String {
    for _ in 0..200 {
        cx.run_until_parked();
        if let Ok(value) = event.try_recv() {
            return value;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("受控 API 驱动未开始延迟请求");
}

pub(super) fn wait_for_discovery(cx: &mut VisualTestContext, event: &async_channel::Receiver<()>) {
    for _ in 0..200 {
        cx.run_until_parked();
        if event.try_recv().is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("受控 gRPC 驱动未开始发现");
}

pub(super) fn callback_watch(
    cx: &mut VisualTestContext,
    view: &Entity<ApiView>,
    kind: ApiViewCallbackKind,
) -> (u64, async_channel::Receiver<ApiViewCallbackCompletion>) {
    cx.update(|_, app| {
        let view = view.read(app);
        let generation = match kind {
            ApiViewCallbackKind::Request | ApiViewCallbackKind::Collection => {
                view.request_generation
            }
            ApiViewCallbackKind::GrpcDiscovery => view.grpc_discovery_generation,
        };
        (generation, view.callback_completion_receiver.clone())
    })
}

pub(super) fn wait_for_stale_callback(
    cx: &mut VisualTestContext,
    receiver: &async_channel::Receiver<ApiViewCallbackCompletion>,
    kind: ApiViewCallbackKind,
    generation: u64,
) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        cx.run_until_parked();
        while let Ok(completion) = receiver.try_recv() {
            if completion.kind == kind && completion.generation == generation {
                assert!(
                    !completion.accepted,
                    "旧回调不应被接受: {kind:?}, generation={generation}"
                );
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "未收到旧回调完成通知: {kind:?}, generation={generation}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(super) fn wait_for_history(
    cx: &mut VisualTestContext,
    view: &Entity<ApiView>,
    storage: &TestStorage,
    request_name: &str,
) {
    let workspace_id = cx.update(|_, app| view.read(app).workspace.id.clone());
    for _ in 0..200 {
        cx.run_until_parked();
        let history = futures::executor::block_on(storage.service.list_history(&workspace_id, 20))
            .expect("读取 API 测试历史");
        if history
            .iter()
            .any(|record| record.request_name == request_name)
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("旧请求未完成历史写入: {request_name}");
}

pub(super) fn marker(snapshot: &ApiResponseSnapshot) -> String {
    serde_json::from_slice::<serde_json::Value>(&snapshot.body)
        .expect("受控驱动返回 JSON")
        .get("marker")
        .and_then(serde_json::Value::as_str)
        .expect("受控响应包含 marker")
        .to_string()
}
