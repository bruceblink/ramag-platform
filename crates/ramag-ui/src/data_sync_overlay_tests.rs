#![allow(clippy::expect_used, clippy::type_complexity)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, Modifiers, MouseButton, ParentElement, Render,
    Styled, TestAppContext, VisualTestContext, Window, div, point, prelude::*, px, size,
};
use ramag_app::{DataSyncExecutionContext, DataSyncGate, DataSyncGatePhase};
use ramag_domain::entities::{DataSyncProgress, DataSyncSummary, DataSyncTaskId};

use super::{
    DataSyncOverlay,
    result::{format_count, format_elapsed_ms, render_result_summary},
};
use super::{render_sync_feedback_notice, sync_progress_percent};

struct OverlayTestHost {
    overlay: Entity<DataSyncOverlay>,
    background_mouse_events: Arc<AtomicUsize>,
    background_key_events: Arc<AtomicUsize>,
    render_count: Arc<AtomicUsize>,
}

impl Render for OverlayTestHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.render_count.fetch_add(1, Ordering::Relaxed);
        let mouse_events = self.background_mouse_events.clone();
        let key_events = self.background_key_events.clone();
        div()
            .id("overlay-test-host")
            .debug_selector(|| "overlay-test-host".into())
            .relative()
            .size_full()
            .on_key_down(move |_, _, _| {
                key_events.fetch_add(1, Ordering::Relaxed);
            })
            .child(
                div()
                    .id("overlay-behind-control")
                    .debug_selector(|| "overlay-behind-control".into())
                    .absolute()
                    .inset_0()
                    .on_mouse_down(MouseButton::Left, move |_, _, _| {
                        mouse_events.fetch_add(1, Ordering::Relaxed);
                    }),
            )
            .child(self.overlay.clone())
    }
}

fn context() -> DataSyncExecutionContext {
    DataSyncExecutionContext {
        source_connection: "source".into(),
        source_scope: "source-db".into(),
        target_connection: "target".into(),
        target_scope: "target-db".into(),
    }
}

fn add_overlay_window(
    cx: &mut TestAppContext,
    gate: Arc<DataSyncGate>,
) -> (
    Entity<OverlayTestHost>,
    Entity<DataSyncOverlay>,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    &mut VisualTestContext,
) {
    cx.update(gpui_kit::component::init);
    let mouse_events = Arc::new(AtomicUsize::new(0));
    let key_events = Arc::new(AtomicUsize::new(0));
    let render_count = Arc::new(AtomicUsize::new(0));
    let mut overlay_entity = None;
    let mouse_for_view = mouse_events.clone();
    let keys_for_view = key_events.clone();
    let renders_for_view = render_count.clone();
    let (host, visual_cx) = cx.add_window_view(|_window, cx| {
        let overlay = cx.new(|cx| DataSyncOverlay::new(gate, cx));
        overlay_entity = Some(overlay.clone());
        OverlayTestHost {
            overlay,
            background_mouse_events: mouse_for_view,
            background_key_events: keys_for_view,
            render_count: renders_for_view,
        }
    });
    (
        host,
        overlay_entity.expect("占屏实体应创建"),
        mouse_events,
        key_events,
        render_count,
        visual_cx,
    )
}

#[gpui_kit::test]
fn running_overlay_covers_window_and_blocks_background_input(cx: &mut TestAppContext) {
    let gate = Arc::new(DataSyncGate::default());
    let _permit = gate
        .begin(DataSyncTaskId::new(), context())
        .expect("门禁应开始");
    let (host_entity, _overlay, mouse_events, key_events, render_count, cx) =
        add_overlay_window(cx, gate);
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    host_entity.update(cx, |_, cx| cx.notify());
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(render_count.load(Ordering::Relaxed) > 0, "宿主必须发生渲染");

    let host = cx.debug_bounds("overlay-test-host").expect("宿主应渲染");
    let overlay = cx.debug_bounds("data-sync-overlay").expect("占屏应渲染");
    assert_eq!(overlay, host, "占屏必须覆盖完整主视图");
    assert!(cx.debug_bounds("data-sync-card").is_some());
    assert!(cx.debug_bounds("sync-progress-track").is_some());
    assert!(cx.debug_bounds("sync-progress-fill").is_none());

    let behind = cx
        .debug_bounds("overlay-behind-control")
        .expect("背后控件应参与布局");
    let click = point(behind.origin.x + px(8.0), behind.origin.y + px(8.0));
    cx.simulate_mouse_down(click, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(click, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_down(click, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(click, MouseButton::Right, Modifiers::default());
    cx.simulate_keystrokes("tab enter escape");
    assert_eq!(mouse_events.load(Ordering::Relaxed), 0);
    assert_eq!(key_events.load(Ordering::Relaxed), 0);
}

#[gpui_kit::test]
fn cancel_and_terminal_result_keep_overlay_until_acknowledged(cx: &mut TestAppContext) {
    let gate = Arc::new(DataSyncGate::default());
    let permit = gate
        .begin(DataSyncTaskId::new(), context())
        .expect("门禁应开始");
    let (host_entity, overlay_entity, _mouse_events, _key_events, _render_count, cx) =
        add_overlay_window(cx, gate.clone());
    host_entity.update(cx, |_, cx| cx.notify());
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();

    let cancel = cx.debug_bounds("sync-cancel").expect("取消按钮应渲染");
    let cancel_point = point(
        cancel.origin.x + cancel.size.width / 2.0,
        cancel.origin.y + cancel.size.height / 2.0,
    );
    cx.simulate_mouse_down(cancel_point, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(cancel_point, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    let confirmation_panel = cx
        .debug_bounds("sync-cancel-confirm-panel")
        .expect("取消确认面板应渲染");
    let confirmation_title = cx
        .debug_bounds("sync-cancel-confirm-title")
        .expect("取消确认标题应渲染");
    let confirmation_message = cx
        .debug_bounds("sync-cancel-confirm-message")
        .expect("取消确认说明应渲染");
    let card = cx.debug_bounds("data-sync-card").expect("同步卡片应渲染");
    assert!(confirmation_panel.origin.x >= card.origin.x);
    assert!(confirmation_panel.right() <= card.right());
    assert!(confirmation_title.origin.x >= confirmation_panel.origin.x);
    assert!(confirmation_message.right() <= confirmation_panel.right());
    let confirm = cx
        .debug_bounds("sync-cancel-confirm")
        .expect("二次取消确认应渲染");
    let confirm_point = point(
        confirm.origin.x + confirm.size.width / 2.0,
        confirm.origin.y + confirm.size.height / 2.0,
    );
    cx.simulate_mouse_down(confirm_point, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(confirm_point, MouseButton::Left, Modifiers::default());
    assert_eq!(
        gate.snapshot().map(|snapshot| snapshot.phase),
        Some(DataSyncGatePhase::Cancelling)
    );
    assert!(gate.is_blocking());

    assert!(gate.finish_cancelled(&permit, DataSyncSummary::default()));
    overlay_entity.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(cx.debug_bounds("data-sync-overlay").is_some());
    assert!(cx.debug_bounds("sync-result-summary").is_some());
    assert!(cx.debug_bounds("sync-running-progress").is_none());
    let acknowledge = cx
        .debug_bounds("sync-result-ack")
        .expect("终态确认按钮应渲染");
    let acknowledge_point = point(
        acknowledge.origin.x + acknowledge.size.width / 2.0,
        acknowledge.origin.y + acknowledge.size.height / 2.0,
    );
    cx.simulate_mouse_down(acknowledge_point, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(acknowledge_point, MouseButton::Left, Modifiers::default());
    assert!(!gate.is_blocking());
}

#[gpui_kit::test]
fn cancel_confirmation_fits_supported_widths(cx: &mut TestAppContext) {
    let gate = Arc::new(DataSyncGate::default());
    let _permit = gate
        .begin(DataSyncTaskId::new(), context())
        .expect("门禁应开始");
    let (host_entity, _overlay, _mouse_events, _key_events, _render_count, visual_cx) =
        add_overlay_window(cx, gate);
    host_entity.update(visual_cx, |_, cx| cx.notify());
    visual_cx.update(|window, _| window.refresh());
    visual_cx.run_until_parked();

    let cancel = visual_cx
        .debug_bounds("sync-cancel")
        .expect("取消按钮应渲染");
    let cancel_point = point(
        cancel.origin.x + cancel.size.width / 2.0,
        cancel.origin.y + cancel.size.height / 2.0,
    );
    visual_cx.simulate_mouse_down(cancel_point, MouseButton::Left, Modifiers::default());
    visual_cx.simulate_mouse_up(cancel_point, MouseButton::Left, Modifiers::default());
    visual_cx.run_until_parked();

    for mode in [crate::Mode::Light, crate::Mode::Dark] {
        visual_cx.update(|_, app| crate::apply_theme(mode, app));
        for width in [320.0, 640.0, 1024.0] {
            visual_cx.simulate_resize(size(px(width), px(360.0)));
            host_entity.update(visual_cx, |_, cx| cx.notify());
            visual_cx.run_until_parked();

            let card = visual_cx
                .debug_bounds("data-sync-card")
                .expect("同步卡片应渲染");
            let panel = visual_cx
                .debug_bounds("sync-cancel-confirm-panel")
                .expect("取消确认面板应渲染");
            let message = visual_cx
                .debug_bounds("sync-cancel-confirm-message")
                .expect("取消确认说明应渲染");
            assert!(panel.origin.x >= card.origin.x);
            assert!(panel.right() <= card.right());
            assert!(message.origin.x >= panel.origin.x);
            assert!(message.right() <= panel.right());
        }
    }
}

#[gpui_kit::test]
fn running_progress_bar_fits_supported_widths(cx: &mut TestAppContext) {
    let gate = Arc::new(DataSyncGate::default());
    let permit = gate
        .begin(DataSyncTaskId::new(), context())
        .expect("门禁应开始");
    assert!(gate.update_progress(
        &permit,
        DataSyncProgress {
            objects_done: 3,
            objects_total: Some(10),
            ..DataSyncProgress::default()
        }
    ));
    let (host_entity, _overlay, _mouse_events, _key_events, _render_count, visual_cx) =
        add_overlay_window(cx, gate);

    for mode in [crate::Mode::Light, crate::Mode::Dark] {
        visual_cx.update(|_, app| crate::apply_theme(mode, app));
        for width in [320.0, 640.0, 1024.0] {
            visual_cx.simulate_resize(size(px(width), px(420.0)));
            host_entity.update(visual_cx, |_, cx| cx.notify());
            visual_cx.run_until_parked();

            let card = visual_cx
                .debug_bounds("data-sync-card")
                .expect("同步卡片应渲染");
            let track = visual_cx
                .debug_bounds("sync-progress-track")
                .expect("同步进度轨道应渲染");
            let fill = visual_cx
                .debug_bounds("sync-progress-fill")
                .expect("同步进度填充应渲染");
            assert!(track.origin.x >= card.origin.x);
            assert!(track.right() <= card.right());
            assert!(fill.origin.x >= track.origin.x);
            assert!(fill.right() <= track.right());
            assert!(fill.size.width > px(0.0));
        }
    }
}

#[gpui_kit::test]
fn failed_result_remains_readable_inside_small_window(cx: &mut TestAppContext) {
    let gate = Arc::new(DataSyncGate::default());
    let permit = gate
        .begin(DataSyncTaskId::new(), context())
        .expect("门禁应开始");
    assert!(gate.finish_failed(
        &permit,
        DataSyncSummary::default(),
        "包含中文的长错误信息".repeat(2_000),
    ));
    let (host_entity, _overlay, _mouse_events, _key_events, _render_count, cx) =
        add_overlay_window(cx, gate);
    cx.simulate_resize(size(px(320.0), px(240.0)));
    host_entity.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    let overlay = cx.debug_bounds("data-sync-overlay").expect("占屏应渲染");
    let card = cx.debug_bounds("data-sync-card").expect("结果卡片应渲染");
    assert!(card.size.width <= overlay.size.width);
    assert!(card.size.height <= overlay.size.height);
    let error_notice = cx
        .debug_bounds("sync-error-notice")
        .expect("同步错误状态卡应渲染");
    let error_scroll = cx
        .debug_bounds("sync-error-scroll")
        .expect("同步错误原因滚动区应渲染");
    assert!(error_notice.origin.x >= card.origin.x);
    assert!(error_notice.right() <= card.right());
    assert!(error_scroll.origin.x >= error_notice.origin.x);
    assert!(error_scroll.right() <= error_notice.right());
    assert!(cx.debug_bounds("sync-result-ack").is_some());
}

struct SyncFeedbackHost;

impl Render for SyncFeedbackHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("sync-feedback-host")
            .debug_selector(|| "sync-feedback-host".into())
            .w_full()
            .child(render_sync_feedback_notice(
                "sync-feedback-error",
                "sync-feedback-error-message",
                "同步失败",
                "错误原因和恢复说明".repeat(20),
                cx.theme().danger,
                cx.theme().muted_foreground,
            ))
    }
}

struct SyncResultSummaryHost;

impl Render for SyncResultSummaryHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let summary = DataSyncSummary {
            objects: 18_446_744_073_709_551_615,
            scanned: 18_446_744_073_709_551_615,
            inserted: 18_446_744_073_709_551_615,
            skipped: 18_446_744_073_709_551_615,
            failed: 18_446_744_073_709_551_615,
            bytes: 18_446_744_073_709_551_615,
            elapsed_ms: 3_661_230,
            cancelled: false,
            warnings: Vec::new(),
            warnings_overflow: 18_446_744_073_709_551_615,
        };
        div()
            .id("sync-result-summary-host")
            .debug_selector(|| "sync-result-summary-host".into())
            .w_full()
            .child(render_result_summary(
                &summary,
                Some(summary.objects),
                theme.border,
                theme.muted_foreground,
                theme.foreground,
                theme.success,
                theme.warning,
                theme.danger,
            ))
    }
}

#[gpui_kit::test]
fn result_summary_fits_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual_cx) = cx.add_window_view(|_, _| SyncResultSummaryHost);

    for mode in [crate::Mode::Light, crate::Mode::Dark] {
        visual_cx.update(|_, app| crate::apply_theme(mode, app));
        for width in [280.0, 320.0, 640.0, 1024.0] {
            visual_cx.simulate_resize(size(px(width), px(420.0)));
            visual_cx.run_until_parked();

            let host = visual_cx
                .debug_bounds("sync-result-summary-host")
                .expect("结果摘要宿主应渲染");
            let summary = visual_cx
                .debug_bounds("sync-result-summary")
                .expect("结果摘要应渲染");
            assert!(summary.origin.x >= host.origin.x);
            assert!(summary.right() <= host.right());
            for id in [
                "sync-result-metrics-primary",
                "sync-result-metrics-secondary",
                "sync-result-metrics-details",
            ] {
                let row = visual_cx.debug_bounds(id).expect("结果指标行应渲染");
                assert!(row.origin.x >= summary.origin.x);
                assert!(row.right() <= summary.right());
            }
            for id in [
                "sync-result-metric-inserted",
                "sync-result-metric-scanned",
                "sync-result-metric-objects",
                "sync-result-metric-skipped",
                "sync-result-metric-failed",
                "sync-result-metric-warnings",
                "sync-result-metric-bytes",
                "sync-result-metric-elapsed",
            ] {
                let metric = visual_cx.debug_bounds(id).expect("结果指标应渲染");
                assert!(metric.origin.x >= summary.origin.x);
                assert!(metric.right() <= summary.right());
            }
        }
    }
}

#[gpui_kit::test]
fn sync_feedback_notice_fits_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual_cx) = cx.add_window_view(|_, _| SyncFeedbackHost);
    let visual_cx: &mut VisualTestContext = visual_cx;

    for mode in [crate::Mode::Light, crate::Mode::Dark] {
        visual_cx.update(|_, app| crate::apply_theme(mode, app));
        for width in [320.0, 640.0, 1024.0] {
            visual_cx.simulate_resize(size(px(width), px(280.0)));
            visual_cx.run_until_parked();

            let Some(host) = visual_cx.debug_bounds("sync-feedback-host") else {
                unreachable!("同步反馈测试宿主应渲染");
            };
            let Some(notice) = visual_cx.debug_bounds("sync-feedback-error") else {
                unreachable!("同步反馈状态卡应渲染");
            };
            let Some(message) = visual_cx.debug_bounds("sync-feedback-error-message") else {
                unreachable!("同步反馈原因区应渲染");
            };
            assert!(notice.origin.x >= host.origin.x);
            assert!(notice.right() <= host.right());
            assert!(notice.origin.y >= host.origin.y);
            assert!(notice.bottom() <= host.bottom());
            assert!(message.origin.x >= notice.origin.x);
            assert!(message.right() <= notice.right());
        }
    }
}

#[test]
fn result_numbers_and_elapsed_time_are_human_readable_at_boundaries() {
    assert_eq!(format_count(0), "0");
    assert_eq!(format_count(999), "999");
    assert_eq!(format_count(1_000), "1,000");
    assert_eq!(format_count(u64::MAX), "18,446,744,073,709,551,615");

    assert_eq!(format_elapsed_ms(0), "0.00 秒");
    assert_eq!(format_elapsed_ms(59_999), "59.99 秒");
    assert_eq!(format_elapsed_ms(60_000), "1 分 00.00 秒");
    assert_eq!(format_elapsed_ms(94_120), "1 分 34.12 秒");
    assert_eq!(format_elapsed_ms(3_661_230), "1 小时 1 分 01.23 秒");
}

#[test]
fn sync_progress_percent_handles_unknown_and_overflowed_counts() {
    assert_eq!(sync_progress_percent(0, None), None);
    assert_eq!(sync_progress_percent(0, Some(0)), None);
    assert_eq!(sync_progress_percent(0, Some(10)), Some(0));
    assert_eq!(sync_progress_percent(3, Some(10)), Some(30));
    assert_eq!(sync_progress_percent(10, Some(10)), Some(100));
    assert_eq!(sync_progress_percent(99, Some(10)), Some(100));
}
