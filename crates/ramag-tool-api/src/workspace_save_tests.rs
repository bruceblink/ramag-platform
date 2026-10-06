use super::*;

/// An unchanged draft must surface a save failure instead of silently discarding it.
#[gpui_kit::test]
fn unchanged_draft_save_failure_is_visible_and_keeps_the_editor(cx: &mut TestAppContext) {
    let storage = Arc::new(WorkspaceTestStorage::new(false, Vec::new()));
    let (view, visual_cx) = add_view(cx, storage.clone());
    assert_eq!(
        wait_for_workspace_load(visual_cx, &view),
        ApiWorkspaceLoadState::Empty
    );
    let (started, release) = storage.delay_next_save();
    visual_cx.update(|_, app| view.update(app, |view, cx| view.save(cx)));
    let mut save_started = false;
    for _ in 0..100 {
        visual_cx.run_until_parked();
        if started.try_recv().is_ok() {
            save_started = true;
            break;
        }
        std::thread::yield_now();
    }
    assert!(save_started);
    release.try_send(false).unwrap();
    for _ in 0..100 {
        visual_cx.run_until_parked();
        if !visual_cx.update(|_, app| view.read(app).saving) {
            break;
        }
        std::thread::yield_now();
    }
    visual_cx.update(|_, app| {
        let draft = view.read(app);
        assert!(!draft.saving);
        assert!(draft.workspace.collections.is_empty());
        assert!(draft.active_request_id.is_none());
        let (message, failed) = draft.notice.as_ref().expect("保存失败必须显示错误");
        assert!(*failed);
        assert!(!message.is_empty());
        assert!(!message.contains("save-secret-sentinel"));
        assert_eq!(draft.request_name.read(app).value().as_str(), "新请求");
    });
}
