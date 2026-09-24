use super::*;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gpui_kit::{Modifiers, TestAppContext, VisualTestContext, point, px, size};
use ramag_app::ApiService;
use ramag_domain::entities::{
    ApiEnvironment, ApiHistoryRecord, ApiRequestRecord, ApiWorkspace, ApiWorkspaceId,
    ConnectionConfig, ConnectionId, HttpRequestSpec, QueryRecord, QueryRecordId,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::{ApiDriver, Storage};
use ramag_infra_api::{GrpcApiDriver, HttpApiDriver};

struct WorkspaceTestStorage {
    fail_listing: AtomicBool,
    fail_history: AtomicBool,
    workspaces: Mutex<Vec<ApiWorkspace>>,
    save_calls: AtomicUsize,
    clear_history_calls: AtomicUsize,
}

impl WorkspaceTestStorage {
    fn new(fail_listing: bool, workspaces: Vec<ApiWorkspace>) -> Self {
        Self {
            fail_listing: AtomicBool::new(fail_listing),
            fail_history: AtomicBool::new(false),
            workspaces: Mutex::new(workspaces),
            save_calls: AtomicUsize::new(0),
            clear_history_calls: AtomicUsize::new(0),
        }
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
        self.workspaces
            .lock()
            .expect("锁定测试工作区")
            .push(workspace.clone());
        Ok(())
    }

    async fn clear_api_history(&self, _workspace_id: &ApiWorkspaceId) -> Result<()> {
        self.clear_history_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
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
        gpui_kit::component::Root::new(view, window, cx)
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
fn workspace_history_failure_is_visible_without_discarding_workspace(cx: &mut TestAppContext) {
    let saved_workspace = ApiWorkspace::new("History Retry");
    let storage = Arc::new(WorkspaceTestStorage::new(false, vec![saved_workspace]));
    storage.fail_history.store(true, Ordering::Relaxed);
    let (view, visual_cx) = add_view(cx, storage.clone());

    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::HistoryFailed
    );
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).workspace.name.clone()),
        "History Retry"
    );
    assert!(
        visual_cx
            .debug_bounds("api-workspace-load-status")
            .is_some()
    );
    assert!(visual_cx.debug_bounds("api-workspace-load-retry").is_some());
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).notice.clone()),
        Some(("工作区已加载，但执行历史读取失败；请点击重试".into(), true))
    );
    assert!(
        !ApiWorkspaceLoadState::HistoryFailed
            .status_message()
            .expect("历史读取失败应提供提示")
            .contains("history-secret-sentinel")
    );
    assert!(
        ApiWorkspaceLoadState::HistoryFailed
            .save_block_message()
            .is_none()
    );

    storage.fail_history.store(false, Ordering::Relaxed);
    click(visual_cx, "api-workspace-load-retry");
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
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
    assert!(visual_cx.debug_bounds("api-history-clear").is_some());
    assert!(visual_cx.update(|_, app| {
        let view = view.read(app);
        view.service.is_some()
            && !view.history.is_empty()
            && !view.clearing_history
            && view.workspace_load_state == ApiWorkspaceLoadState::Loaded
    }));

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| view.confirm_clear_history(window, cx));
    });
    visual_cx.run_until_parked();
    visual_cx.run_until_parked();
    assert!(visual_cx.update(|_, app| {
        let view = view.read(app);
        !view.history.is_empty() && !view.clearing_history
    }));

    // 共享确认弹窗的布局和按钮由 ramag-ui 专项测试覆盖；这里直接执行确认回调，
    // 验证 API 工作台的 Storage 调用、成功状态和本地列表清理。
    view.update(visual_cx, |view, cx| view.clear_history(cx));

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
}
