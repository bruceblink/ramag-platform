use super::*;

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
