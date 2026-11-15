//! 平台、页面和端点切换后的迟到回调回归。

use super::*;
use gpui_kit::TestAppContext;
use ramag_domain::entities::ContainerPlatform;

#[path = "view_context_test_support.rs"]
mod support;
use support::*;

fn verify_overview_switch(cx: &mut TestAppContext, fail: bool) {
    let mut h = setup(cx);
    let release = h.driver.delay(Operation::Overview, fail);
    click(h.visual, "container-refresh");
    let old = h.generation();
    click(h.visual, "container-platform-kubernetes");
    let calls = h.driver.calls.lock().expect("调用锁").len();
    click(h.visual, "container-resource-containers");
    h.release_and_wait(release, old);
    h.visual.update(|_, app| {
        let view = h.view.read(app);
        assert_eq!(view.platform, ContainerPlatform::Kubernetes);
        assert_eq!(view.section, ContainerSection::Containers);
        assert!(view.overview.is_none());
        assert!(view.connection.is_none());
        assert!(
            view.error
                .as_deref()
                .is_some_and(|error| error.contains("Kubernetes"))
        );
        assert!(!view.loading);
    });
    assert_eq!(h.driver.calls.lock().expect("调用锁").len(), calls);

    click(h.visual, "container-platform-docker");
    h.visual.update(|_, app| {
        let view = h.view.read(app);
        assert!(view.overview.is_some(), "切回后仍应允许刷新");
        assert!(view.error.is_none());
        assert!(!view.loading);
    });
}

#[gpui_kit::test]
fn delayed_overview_cannot_restore_docker_after_platform_change(cx: &mut TestAppContext) {
    verify_overview_switch(cx, false);
}

#[gpui_kit::test]
fn delayed_overview_error_cannot_replace_kubernetes_notice(cx: &mut TestAppContext) {
    verify_overview_switch(cx, true);
}

#[gpui_kit::test]
fn delayed_list_cannot_fill_logs_page_and_navigation_can_retry(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let release = h.driver.delay(Operation::List, false);
    click(h.visual, "container-resource-containers");
    let old = h.generation();
    click(h.visual, "container-resource-logs");
    h.release_and_wait(release, old);
    h.visual.update(|_, app| {
        let view = h.view.read(app);
        assert_eq!(view.section, ContainerSection::Logs);
        assert!(view.containers.is_none());
        assert!(!view.loading);
    });
    click(h.visual, "container-resource-containers");
    h.visual.update(|_, app| {
        let view = h.view.read(app);
        assert!(view.containers.is_some());
        assert!(!view.loading);
    });
}

fn verify_detail_switch(cx: &mut TestAppContext, fail: bool) {
    let mut h = setup(cx);
    click(h.visual, "container-resource-containers");
    let release = h.driver.delay(Operation::Detail, fail);
    h.view.update(h.visual, |view, cx| {
        view.load_detail("context-container".into(), cx)
    });
    h.visual.run_until_parked();
    let old = h.generation();
    click(h.visual, "container-resource-logs");
    h.release_and_wait(release, old);
    h.visual.update(|_, app| {
        let view = h.view.read(app);
        assert_eq!(view.section, ContainerSection::Logs);
        assert!(view.selected_detail.is_none());
        assert!(view.error.is_none());
        assert!(!view.detail_loading);
    });
}

#[gpui_kit::test]
fn delayed_detail_cannot_reopen_after_logs_navigation(cx: &mut TestAppContext) {
    verify_detail_switch(cx, false);
}

#[gpui_kit::test]
fn delayed_detail_error_cannot_update_logs_page(cx: &mut TestAppContext) {
    verify_detail_switch(cx, true);
}

#[gpui_kit::test]
fn registry_endpoint_change_rejects_old_repositories_and_preserves_validation_error(
    cx: &mut TestAppContext,
) {
    let mut h = setup(cx);
    let release = h.driver.delay(Operation::Repositories, false);
    click(h.visual, "container-resource-registry");
    let old = h.generation();
    h.view.update(h.visual, |view, cx| {
        // Remove only the test input binding so refresh uses the changed value.
        view.registry_endpoint_input = None;
        assert!(view.apply_registry_configuration("not-a-registry".into(), false));
        view.refresh_registry(cx);
    });
    h.release_and_wait(release, old);
    h.visual.update(|_, app| {
        let view = h.view.read(app);
        assert!(view.registry_repositories.is_none());
        assert!(view.registry_tags.is_none());
        assert!(view.registry_error.is_some());
        assert!(!view.registry_loading);
    });
}

#[gpui_kit::test]
fn registry_policy_change_and_page_navigation_reject_old_tags(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    click(h.visual, "container-resource-registry");
    let release = h.driver.delay(Operation::Tags, false);
    h.view.update(h.visual, |view, cx| {
        view.load_registry_tags("test/app".into(), cx)
    });
    h.visual.run_until_parked();
    let old = h.generation();
    h.view.update(h.visual, |view, _| {
        assert!(view.apply_registry_configuration(view.registry_endpoint.clone(), true));
        assert!(!view.registry_loading);
        assert!(view.selected_registry_repository.is_none());
    });
    click(h.visual, "container-resource-logs");
    h.release_and_wait(release, old);
    h.visual.update(|_, app| {
        let view = h.view.read(app);
        assert!(view.registry_tags.is_none());
        assert!(view.registry_error.is_none());
        assert!(!view.registry_loading);
    });
    click(h.visual, "container-resource-registry");
    h.visual.update(|_, app| {
        let view = h.view.read(app);
        assert!(view.registry_repositories.is_some());
        assert!(!view.registry_loading);
    });
}

#[gpui_kit::test]
fn opening_logs_cancels_pending_stats_and_platform_change_cancels_logs(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    click(h.visual, "container-resource-containers");
    let stats_release = h.driver.delay(Operation::Stats, false);
    h.view.update(h.visual, |view, cx| {
        view.selected_detail = Some(SelectedDetail::Container(detail("context-container")));
        view.load_container_stats("context-container".into(), cx);
    });
    h.visual.run_until_parked();
    let stats_id = h.generation();
    let stats_cancel = h
        .visual
        .update(|_, app| h.view.read(app).stats_cancellation.clone())
        .expect("指标取消标记");
    let logs_release = h.driver.delay(Operation::Logs, false);
    click(h.visual, "container-open-logs");
    let logs_id = h.generation();
    assert!(stats_cancel.load(Ordering::Relaxed));
    h.release_and_wait(stats_release, stats_id);
    let logs_cancel = h
        .visual
        .update(|_, app| h.view.read(app).log_cancellation.clone())
        .expect("日志取消标记");
    click(h.visual, "container-platform-kubernetes");
    assert!(logs_cancel.load(Ordering::Relaxed));
    h.release_and_wait(logs_release, logs_id);
    h.visual.update(|_, app| {
        let view = h.view.read(app);
        assert!(view.container_stats.is_none());
        assert!(view.logs.is_none());
        assert!(!view.logs_loading);
        assert!(!view.stats_loading);
    });
}

#[gpui_kit::test]
fn kubernetes_notice_does_not_offer_windows_docker_connection_guidance(cx: &mut TestAppContext) {
    let h = setup(cx);
    h.view.update(h.visual, |view, cx| {
        view.docker_endpoint = "npipe:////./pipe/docker_engine".into();
        view.select_platform(ContainerPlatform::Kubernetes, cx);
    });
    h.visual.run_until_parked();
    assert!(h.visual.debug_bounds("container-error").is_some());
    assert!(h.visual.debug_bounds("container-wsl-guidance").is_none());
}
