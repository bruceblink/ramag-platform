use super::*;

#[gpui_kit::test]
fn request_execution_waits_for_delayed_workspace_load_and_history_failure_is_recoverable(
    cx: &mut TestAppContext,
) {
    let saved_workspace = ApiWorkspace::new("Delayed Load");
    let storage = Arc::new(WorkspaceTestStorage::new(false, vec![saved_workspace]));
    let (started, release) = storage.delay_next_listing();
    let (view, visual_cx) = add_view(cx, storage);

    for _ in 0..100 {
        visual_cx.run_until_parked();
        if started.try_recv().is_ok() {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).workspace_load_state),
        ApiWorkspaceLoadState::Loading
    );

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| view.send(window, cx));
    });
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).notice.clone()),
        Some((
            "本地 API 工作区仍在读取，请等待完成后再执行请求".into(),
            true,
        ))
    );
    assert!(!visual_cx.update(|_, app| view.read(app).loading));
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).request_generation),
        0
    );

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| view.run_collection(window, cx));
    });
    assert_eq!(
        visual_cx.update(|_, app| view.read(app).notice.clone()),
        Some((
            "本地 API 工作区仍在读取，请等待完成后再执行请求".into(),
            true,
        ))
    );
    release.try_send(true).expect("应允许完成延迟工作区读取");
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Loaded
    );

    assert!(
        ApiWorkspaceLoadState::HistoryFailed
            .execution_block_message()
            .is_none(),
        "历史读取失败时工作区身份仍可信，应允许执行请求"
    );
}
