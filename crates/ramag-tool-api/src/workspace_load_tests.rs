use super::*;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gpui_kit::{
    IntoElement, Modifiers, Render, TestAppContext, VisualTestContext, Window, div, point, px, size,
};
use ramag_app::ApiService;
use ramag_domain::entities::{
    ApiCollection, ApiEnvironment, ApiHistoryRecord, ApiRequestRecord, ApiWorkspace,
    ApiWorkspaceId, ConnectionConfig, ConnectionId, HttpRequestSpec, QueryRecord, QueryRecordId,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::{ApiDriver, Storage};
use ramag_infra_api::{GrpcApiDriver, HttpApiDriver};

#[path = "workspace_history_tests.rs"]
mod workspace_history_tests;

struct WorkspaceTestStorage {
    fail_listing: AtomicBool,
    fail_history: AtomicBool,
    fail_clear_history: AtomicBool,
    workspaces: Mutex<Vec<ApiWorkspace>>,
    save_gate: Mutex<Option<WorkspaceSaveGate>>,
    save_calls: AtomicUsize,
    clear_history_calls: AtomicUsize,
}

struct ApiViewTestHost {
    view: Entity<ApiView>,
}

impl Render for ApiViewTestHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = gpui_kit::component::Root::render_dialog_layer(window, cx);
        div()
            .relative()
            .size_full()
            .child(self.view.clone())
            .children(dialog_layer)
    }
}

struct WorkspaceSaveGate {
    started: async_channel::Sender<()>,
    release: async_channel::Receiver<bool>,
}

impl WorkspaceTestStorage {
    fn new(fail_listing: bool, workspaces: Vec<ApiWorkspace>) -> Self {
        Self {
            fail_listing: AtomicBool::new(fail_listing),
            fail_history: AtomicBool::new(false),
            fail_clear_history: AtomicBool::new(false),
            workspaces: Mutex::new(workspaces),
            save_gate: Mutex::new(None),
            save_calls: AtomicUsize::new(0),
            clear_history_calls: AtomicUsize::new(0),
        }
    }

    fn delay_next_save(&self) -> (async_channel::Receiver<()>, async_channel::Sender<bool>) {
        let (started_sender, started_receiver) = async_channel::bounded(1);
        let (release_sender, release_receiver) = async_channel::bounded(1);
        *self.save_gate.lock().expect("锁定保存测试控制") = Some(WorkspaceSaveGate {
            started: started_sender,
            release: release_receiver,
        });
        (started_receiver, release_sender)
    }
}

#[async_trait::async_trait]
impl Storage for WorkspaceTestStorage {
    async fn list_api_workspaces(&self) -> Result<Vec<ApiWorkspace>> {
        if self.fail_listing.load(Ordering::Relaxed) {
            return Err(DomainError::Storage(
                "secret-token and request-body sentinel".into(),
            ));
        }
        Ok(self.workspaces.lock().expect("锁定测试工作区").clone())
    }

    async fn list_api_history(
        &self,
        _workspace_id: &ApiWorkspaceId,
        _limit: usize,
    ) -> Result<Vec<ApiHistoryRecord>> {
        if self.fail_history.load(Ordering::Relaxed) {
            return Err(DomainError::Storage("history-secret-sentinel".into()));
        }
        Ok(Vec::new())
    }

    async fn save_api_workspace(&self, workspace: &ApiWorkspace) -> Result<()> {
        self.save_calls.fetch_add(1, Ordering::Relaxed);
        let gate = self.save_gate.lock().expect("锁定保存测试控制").take();
        if let Some(gate) = gate {
            gate.started
                .send(())
                .await
                .map_err(|error| DomainError::Storage(error.to_string()))?;
            if !gate
                .release
                .recv()
                .await
                .map_err(|error| DomainError::Storage(error.to_string()))?
            {
                return Err(DomainError::Storage("save-secret-sentinel".into()));
            }
        }
        let mut workspaces = self.workspaces.lock().expect("锁定测试工作区");
        if let Some(existing) = workspaces
            .iter_mut()
            .find(|existing| existing.id == workspace.id)
        {
            *existing = workspace.clone();
        } else {
            workspaces.push(workspace.clone());
        }
        Ok(())
    }

    async fn clear_api_history(&self, _workspace_id: &ApiWorkspaceId) -> Result<()> {
        self.clear_history_calls.fetch_add(1, Ordering::Relaxed);
        if self.fail_clear_history.load(Ordering::Relaxed) {
            Err(DomainError::Storage("clear-history-secret-sentinel".into()))
        } else {
            Ok(())
        }
    }

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

fn click(cx: &mut VisualTestContext, selector: &'static str) {
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

fn click_dialog_button(cx: &mut VisualTestContext, selector: &'static str) {
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(300));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("确认对话框按钮应渲染: {selector}"));
    let center = point(
        bounds.origin.x + bounds.size.width / 2.0,
        bounds.origin.y + bounds.size.height / 2.0,
    );
    cx.simulate_mouse_move(center, None, Modifiers::default());
    let bounds = cx.debug_bounds(selector).unwrap_or(bounds);
    let center = point(
        bounds.origin.x + bounds.size.width / 2.0,
        bounds.origin.y + bounds.size.height / 2.0,
    );
    cx.simulate_mouse_down(center, gpui_kit::MouseButton::Left, Modifiers::default());
    let bounds = cx.debug_bounds(selector).unwrap_or(bounds);
    let center = point(
        bounds.origin.x + bounds.size.width / 2.0,
        bounds.origin.y + bounds.size.height / 2.0,
    );
    cx.simulate_mouse_up(center, gpui_kit::MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
}

/// Verifies that a delayed save cannot replace a request selected after the save began.
fn verify_delayed_save_preserves_newer_view(cx: &mut TestAppContext, succeed: bool) {
    let first = ApiRequestRecord::new_http(
        "First request",
        HttpRequestSpec::new("GET", "http://127.0.0.1/first"),
    );
    let first_id = first.id.clone();
    let second = ApiRequestRecord::new_http(
        "Second request",
        HttpRequestSpec::new("GET", "http://127.0.0.1/second"),
    );
    let second_id = second.id.clone();
    let mut workspace = ApiWorkspace::new("Delayed Save");
    let mut collection = ApiCollection::new("Requests");
    collection.requests.extend([first, second]);
    workspace.collections.push(collection);
    let storage = Arc::new(WorkspaceTestStorage::new(false, vec![workspace]));
    let (started, release) = storage.delay_next_save();
    let (view, visual_cx) = add_view(cx, storage.clone());
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).active_request_id.clone()),
        Some(first_id)
    );

    visual_cx.update(|_, app| view.update(app, |view, cx| view.save(cx)));
    for _ in 0..100 {
        visual_cx.run_until_parked();
        if started.try_recv().is_ok() {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(storage.save_calls.load(Ordering::Relaxed), 1);

    visual_cx.update(|_, app| view.update(app, |view, cx| view.save(cx)));
    visual_cx.update(|window, app| view.update(app, |view, cx| view.import(window, cx)));
    let load_generation = visual_cx.update(|_, app| view.read(app).workspace_load_generation);
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| view.load_saved_workspace(window, cx));
    });
    assert_eq!(storage.save_calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).workspace_load_generation),
        load_generation,
        "保存期间重试读取不应并发启动"
    );
    assert!(!visual_cx.update(|_, app| view.read(app).importing));

    click(visual_cx, "api-request-item-1");
    visual_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.notice = Some(("较新的界面提示".into(), false));
            cx.notify();
        });
    });
    release.try_send(succeed).expect("应允许完成延迟保存");

    for _ in 0..100 {
        visual_cx.run_until_parked();
        if !visual_cx.update(|_, app| view.read(app).saving) {
            break;
        }
        std::thread::yield_now();
    }
    assert!(!visual_cx.update(|_, app| view.read(app).saving));
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).active_request_id.clone()),
        Some(second_id)
    );
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).notice.clone()),
        Some(("较新的界面提示".into(), false))
    );
    assert!(!visual_cx.update(|_, app| {
        view.read(app)
            .notice
            .as_ref()
            .is_some_and(|(message, _)| message.contains("save-secret-sentinel"))
    }));
    assert_eq!(
        storage.workspaces.lock().expect("锁定测试工作区").len(),
        1,
        "保存应更新现有工作区而不是重复创建"
    );
}

fn add_view(
    cx: &mut TestAppContext,
    storage: Arc<WorkspaceTestStorage>,
) -> (Entity<ApiView>, &mut VisualTestContext) {
    cx.update(gpui_kit::component::init);
    cx.executor().allow_parking();
    let http_driver: Arc<dyn ApiDriver> =
        Arc::new(HttpApiDriver::new().expect("创建测试 HTTP 驱动"));
    let grpc_driver: Arc<dyn ApiDriver> =
        Arc::new(GrpcApiDriver::new().expect("创建测试 gRPC 驱动"));
    let service =
        Arc::new(ApiService::new(http_driver, grpc_driver, storage).expect("创建测试 API 服务"));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let service = service.clone();
        let view = cx.new(|cx| ApiView::with_service(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| ApiViewTestHost { view });
        gpui_kit::component::Root::new(host, window, cx)
    });
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.run_until_parked();
    (view_entity.expect("API 视图应初始化"), visual_cx)
}

fn wait_for_workspace_load(
    cx: &mut VisualTestContext,
    view: &Entity<ApiView>,
) -> ApiWorkspaceLoadState {
    for _ in 0..100 {
        cx.run_until_parked();
        let state = cx.update(|_, app| view.read(app).workspace_load_state);
        if state != ApiWorkspaceLoadState::Loading {
            return state;
        }
        std::thread::yield_now();
    }
    panic!("API 工作区读取未完成");
}

#[gpui_kit::test]
fn workspace_load_failure_is_redacted_blocks_save_and_can_retry(cx: &mut TestAppContext) {
    let saved_workspace = ApiWorkspace::new("Previously Saved");
    let storage = Arc::new(WorkspaceTestStorage::new(
        true,
        vec![saved_workspace.clone()],
    ));
    let (view, visual_cx) = add_view(cx, storage.clone());

    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Failed
    );
    assert!(
        visual_cx
            .debug_bounds("api-workspace-load-status")
            .is_some()
    );
    assert!(visual_cx.debug_bounds("api-workspace-load-retry").is_some());
    assert!(
        !ApiWorkspaceLoadState::Failed
            .status_message()
            .expect("失败状态应提供提示")
            .contains("secret-token")
    );

    visual_cx.update(|_, app| {
        view.update(app, |view, cx| view.save(cx));
    });
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| view.import(window, cx));
    });
    assert_eq!(storage.save_calls.load(Ordering::Relaxed), 0);
    assert!(!visual_cx.update(|_, app| view.read(app).importing));
    assert!(!visual_cx.update(|_, app| {
        view.read(app)
            .notice
            .as_ref()
            .is_some_and(|(message, _)| message.contains("secret-token"))
    }));

    storage.fail_listing.store(false, Ordering::Relaxed);
    click(visual_cx, "api-workspace-load-retry");
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).workspace.name.clone()),
        "Previously Saved"
    );
    assert!(
        visual_cx
            .debug_bounds("api-workspace-load-status")
            .is_none()
    );
}

#[gpui_kit::test]
fn empty_workspace_list_is_distinct_and_allows_creating_a_workspace(cx: &mut TestAppContext) {
    let storage = Arc::new(WorkspaceTestStorage::new(false, Vec::new()));
    let (view, visual_cx) = add_view(cx, storage.clone());

    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Empty
    );
    assert!(
        visual_cx
            .debug_bounds("api-workspace-load-status")
            .is_some()
    );
    assert!(ApiWorkspaceLoadState::Empty.save_block_message().is_none());

    visual_cx.update(|_, app| {
        view.update(app, |view, cx| view.save(cx));
    });
    for _ in 0..100 {
        visual_cx.run_until_parked();
        if storage.save_calls.load(Ordering::Relaxed) > 0 {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(storage.save_calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).workspace_load_state),
        ApiWorkspaceLoadState::Loaded
    );
    assert!(
        visual_cx
            .debug_bounds("api-workspace-load-status")
            .is_none()
    );
}

#[gpui_kit::test]
fn api_history_can_be_cleared_from_the_sidebar_after_confirmation(cx: &mut TestAppContext) {
    let saved_workspace = ApiWorkspace::new("History Clear");
    let storage = Arc::new(WorkspaceTestStorage::new(false, vec![saved_workspace]));
    let (view, visual_cx) = add_view(cx, storage.clone());

    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );
    view.update(visual_cx, |view, cx| {
        let request = ApiRequestRecord::new_http(
            "清理测试",
            HttpRequestSpec::new("GET", "http://127.0.0.1/history"),
        );
        view.history.push(ApiHistoryRecord::from_error(
            &request,
            "测试历史",
            &ApiEnvironment::new("测试环境"),
        ));
        cx.notify();
    });
    visual_cx.run_until_parked();
    let clear_button = visual_cx
        .debug_bounds("api-history-clear")
        .expect("有执行历史时应显示清理按钮");
    assert!(
        clear_button.size.width >= px(24.0) && clear_button.size.height >= px(24.0),
        "清理按钮应有可点击尺寸：{clear_button:?}"
    );
    assert!(visual_cx.update(|_, app| {
        let view = view.read(app);
        view.service.is_some()
            && !view.history.is_empty()
            && !view.clearing_history
            && view.workspace_load_state == ApiWorkspaceLoadState::Loaded
    }));

    assert_eq!(storage.clear_history_calls.load(Ordering::Relaxed), 0);
    click(visual_cx, "api-history-clear");
    visual_cx.run_until_parked();
    assert!(
        visual_cx.update(|window, app| {
            gpui_kit::component::WindowExt::has_active_dialog(window, app)
        })
    );
    assert!(visual_cx.debug_bounds("ramag-confirm-ok").is_some());
    click_dialog_button(visual_cx, "ramag-confirm-ok");
    assert!(
        !visual_cx.update(|window, app| {
            gpui_kit::component::WindowExt::has_active_dialog(window, app)
        })
    );

    for _ in 0..100 {
        visual_cx.run_until_parked();
        if storage.clear_history_calls.load(Ordering::Relaxed) > 0 {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(storage.clear_history_calls.load(Ordering::Relaxed), 1);
    assert!(visual_cx.update(|_, app| view.read(app).history.is_empty()));
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).notice.clone()),
        Some(("执行历史已清空".into(), false))
    );

    let retained_request = ApiRequestRecord::new_http(
        "失败后保留",
        HttpRequestSpec::new("GET", "http://127.0.0.1/history-retained"),
    );
    view.update(visual_cx, |view, cx| {
        view.history.push(ApiHistoryRecord::from_error(
            &retained_request,
            "保留测试历史",
            &ApiEnvironment::new("测试环境"),
        ));
        cx.notify();
    });
    storage.fail_clear_history.store(true, Ordering::Relaxed);
    click(visual_cx, "api-history-clear");
    visual_cx.run_until_parked();
    assert!(
        visual_cx.update(|window, app| {
            gpui_kit::component::WindowExt::has_active_dialog(window, app)
        })
    );
    click_dialog_button(visual_cx, "ramag-confirm-ok");
    for _ in 0..100 {
        visual_cx.run_until_parked();
        if !view.read_with(visual_cx, |view, _| view.clearing_history) {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).history.len()),
        1,
        "持久化失败时必须保留界面中的历史"
    );
    let notice = visual_cx.update(|_, app| view.read(app).notice.clone());
    assert_eq!(
        notice,
        Some(("清空执行历史失败；请检查本地存储后重试".into(), true))
    );
    assert!(
        !notice
            .expect("清理失败应显示提示")
            .0
            .contains("clear-history-secret-sentinel")
    );
}

#[gpui_kit::test]
fn delayed_save_success_preserves_newer_request_selection(cx: &mut TestAppContext) {
    verify_delayed_save_preserves_newer_view(cx, true);
}

#[gpui_kit::test]
fn delayed_save_failure_preserves_newer_notice(cx: &mut TestAppContext) {
    verify_delayed_save_preserves_newer_view(cx, false);
}
