//! Mutation regressions use synthetic history and a driver that never reads the OS clipboard.

use super::*;
use gpui_kit::test::TestWindowExt as _;
use ramag_domain::entities::{ClipId, ClipItem, ClipKind};
use std::sync::atomic::Ordering;
use std::time::Duration;

fn sample() -> Arc<ClipItem> {
    let now = chrono::Utc::now();
    Arc::new(ClipItem {
        id: ClipId::new(),
        kind: ClipKind::Text,
        text: Some("needle test entry".into()),
        rtf: None,
        image_path: None,
        thumb_path: None,
        image_dims: None,
        files: Vec::new(),
        preview: "needle test entry".into(),
        source: None,
        byte_size: 17,
        content_hash: "synthetic-needle".into(),
        created_at: now,
        last_used_at: now,
    })
}

fn window_with_entry(
    cx: &mut TestAppContext,
    storage: Arc<TestStorage>,
    item: Arc<ClipItem>,
) -> Option<(Entity<ClipboardView>, &mut VisualTestContext)> {
    cx.update(gpui_kit::component::init);
    let mut view = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let service = Arc::new(ClipboardService::new(Arc::new(TestClipboard), storage));
        let entity = cx.new(|cx| {
            let mut view = ClipboardView::new(service, window, cx);
            view.search
                .update(cx, |input, cx| input.set_value("needle", window, cx));
            view.search_results = vec![item.clone()];
            view.items = vec![item.clone()];
            view.selected = Some(item.id.clone());
            view.detail_text_cache = Some((item.id.clone(), "needle test entry".into()));
            view
        });
        view = Some(entity.clone());
        let host = cx.new(|_| ClipboardTestHost {
            view: entity.clone(),
        });
        gpui_kit::component::Root::new(host, window, cx)
    });
    cx.simulate_resize(gpui_kit::size(gpui_kit::px(1024.0), gpui_kit::px(768.0)));
    cx.run_until_parked();
    view.map(|view| (view, cx))
}

#[gpui_kit::test]
fn successful_delete_clears_search_and_detail_and_rejects_late_result(cx: &mut TestAppContext) {
    let item = sample();
    let storage = Arc::new(TestStorage::default());
    storage
        .rows
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(item.as_ref().clone());
    storage.hold_search.store(true, Ordering::SeqCst);
    let Some((view, cx)) = window_with_entry(cx, storage.clone(), item.clone()) else {
        return;
    };
    view.update(cx, |view, cx| view.schedule_search(cx));
    cx.executor().advance_clock(Duration::from_millis(251));
    cx.run_until_parked();
    assert!(storage.search_held.load(Ordering::SeqCst));
    let generation = view.read_with(cx, |view, _| view.search_gen);
    view.update(cx, |view, cx| view.delete_clip(item.clone(), cx));
    cx.run_until_parked();
    assert!(
        storage
            .rows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty()
    );
    assert!(view.read_with(cx, |view, _| view.search_gen > generation));
    assert!(view.read_with(cx, |view, _| view.selected.is_none()));
    assert!(view.read_with(cx, |view, _| view.detail_text_cache.is_none()));
    storage.release_held_search();
    cx.run_until_parked();
    assert!(view.read_with(cx, |view, app| view.visible_items(app).is_empty()));
    cx.executor().advance_clock(Duration::from_millis(251));
    cx.run_until_parked();
    assert!(view.read_with(cx, |view, _| view.search_results.is_empty()));
    assert!(storage.search_calls.load(Ordering::SeqCst) >= 2);
}

#[gpui_kit::test]
fn failed_delete_keeps_search_selection_and_detail(cx: &mut TestAppContext) {
    let item = sample();
    let storage = Arc::new(TestStorage::default());
    storage
        .rows
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(item.as_ref().clone());
    storage.fail_delete.store(true, Ordering::SeqCst);
    let Some((view, cx)) = window_with_entry(cx, storage.clone(), item.clone()) else {
        return;
    };
    let generation = view.read_with(cx, |view, _| view.search_gen);
    view.update(cx, |view, cx| view.delete_clip(item.clone(), cx));
    cx.run_until_parked();
    assert_eq!(
        storage
            .rows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len(),
        1
    );
    assert_eq!(view.read_with(cx, |view, _| view.search_gen), generation);
    assert_eq!(
        view.read_with(cx, |view, _| view.selected.clone()),
        Some(item.id.clone())
    );
    assert!(view.read_with(cx, |view, _| view.detail_text_cache.is_some()));
    assert_eq!(
        view.read_with(cx, |view, app| view.visible_items(app)[0].id.clone()),
        item.id
    );
}

#[gpui_kit::test]
fn undo_button_restores_entry_and_requeries_current_search(cx: &mut TestAppContext) {
    let item = sample();
    let storage = Arc::new(TestStorage::default());
    storage
        .rows
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(item.as_ref().clone());
    let Some((view, cx)) = window_with_entry(cx, storage.clone(), item.clone()) else {
        return;
    };
    view.update(cx, |view, cx| view.delete_clip(item.clone(), cx));
    cx.run_until_parked();
    assert!(
        storage
            .rows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty()
    );
    let generation = view.read_with(cx, |view, _| view.search_gen);
    // Toast buttons stay non-visible while the 400 ms entrance animation is running.
    std::thread::sleep(Duration::from_millis(410));
    let undo = cx.debug_bounds("clip-undo-delete");
    assert!(undo.is_some(), "删除成功后应渲染撤销按钮");
    if undo.is_none() {
        return;
    }
    cx.update(|window, app| window.click("clip-undo-delete", app));
    cx.run_until_parked();
    assert_eq!(storage.restore_calls.load(Ordering::SeqCst), 1);
    assert!(view.read_with(cx, |view, _| view.search_gen > generation));
    assert_eq!(
        view.read_with(cx, |view, app| view.search.read(app).value().to_string()),
        "needle"
    );
    cx.executor().advance_clock(Duration::from_millis(251));
    cx.run_until_parked();
    let (visible, items, results) = view.read_with(cx, |view, app| {
        (
            view.visible_items(app),
            view.items.clone(),
            view.search_results.clone(),
        )
    });
    assert!(
        visible.iter().any(|row| row.id == item.id),
        "撤销后当前查询应找到条目：visible={visible:?}, items={items:?}, results={results:?}"
    );
    assert!(
        results.iter().any(|row| row.id == item.id),
        "撤销后应重新执行当前查询：results={results:?}"
    );
}

#[gpui_kit::test]
fn undo_button_reports_restore_failure_without_resurrecting_entry(cx: &mut TestAppContext) {
    let item = sample();
    let storage = Arc::new(TestStorage::default());
    storage
        .rows
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(item.as_ref().clone());
    let Some((view, cx)) = window_with_entry(cx, storage.clone(), item.clone()) else {
        return;
    };
    view.update(cx, |view, cx| view.delete_clip(item.clone(), cx));
    cx.run_until_parked();
    storage.fail_restore.store(true, Ordering::SeqCst);
    // Toast buttons stay non-visible while the 400 ms entrance animation is running.
    std::thread::sleep(Duration::from_millis(410));
    let undo = cx.debug_bounds("clip-undo-delete");
    assert!(undo.is_some(), "删除成功后应渲染撤销按钮");
    if undo.is_none() {
        return;
    }
    cx.update(|window, app| window.click("clip-undo-delete", app));
    cx.run_until_parked();

    assert_eq!(storage.restore_calls.load(Ordering::SeqCst), 1);
    assert!(
        storage
            .rows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty(),
        "撤销失败时不能重新写入条目"
    );
    assert!(
        view.read_with(cx, |view, _| view.pending_notification.is_some())
            || cx.debug_bounds("clipboard-restore-error").is_some(),
        "撤销失败时应保留错误通知"
    );
}

#[gpui_kit::test]
fn current_search_failure_keeps_cached_rows_and_reports_error(cx: &mut TestAppContext) {
    let item = sample();
    let storage = Arc::new(TestStorage::default());
    storage
        .rows
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(item.as_ref().clone());
    storage.fail_search.store(true, Ordering::SeqCst);
    let Some((view, cx)) = window_with_entry(cx, storage.clone(), item.clone()) else {
        return;
    };
    view.update(cx, |view, cx| view.schedule_search(cx));
    let generation = view.read_with(cx, |view, _| view.search_gen);
    cx.executor().advance_clock(Duration::from_millis(251));
    cx.run_until_parked();

    assert_eq!(view.read_with(cx, |view, _| view.search_gen), generation);
    assert!(view.read_with(cx, |view, _| view.search_results.is_empty()));
    assert!(view.read_with(cx, |view, app| {
        view.visible_items(app).iter().any(|row| row.id == item.id)
    }));
    assert!(!storage.fail_search.load(Ordering::SeqCst));
    assert!(view.read_with(cx, |view, _| view.pending_notification.is_none()));
    assert_eq!(storage.search_calls.load(Ordering::SeqCst), 1);
}

#[gpui_kit::test]
fn stale_search_failure_does_not_replace_a_newer_query_result(cx: &mut TestAppContext) {
    let item = sample();
    let storage = Arc::new(TestStorage::default());
    storage
        .rows
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(item.clone().as_ref().clone());
    storage.fail_search.store(true, Ordering::SeqCst);
    storage.hold_search.store(true, Ordering::SeqCst);
    let Some((view, cx)) = window_with_entry(cx, storage.clone(), item.clone()) else {
        return;
    };
    view.update(cx, |view, cx| view.schedule_search(cx));
    cx.executor().advance_clock(Duration::from_millis(251));
    cx.run_until_parked();
    assert!(storage.search_held.load(Ordering::SeqCst));

    view.update(cx, |view, cx| view.schedule_search(cx));
    cx.executor().advance_clock(Duration::from_millis(251));
    cx.run_until_parked();
    storage.release_held_search();
    cx.run_until_parked();

    assert!(view.read_with(cx, |view, _| view.pending_notification.is_none()));
    assert!(view.read_with(cx, |view, _| {
        view.search_results.iter().any(|row| row.id == item.id)
    }));
    assert!(storage.search_calls.load(Ordering::SeqCst) >= 2);
}
