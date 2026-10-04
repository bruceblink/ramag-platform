use gpui_kit::component::Root;
use gpui_kit::{
    AppContext as _, Bounds, Modifiers, Pixels, TestAppContext, VisualTestContext, px, size,
};
use ramag_domain::entities::{ContainerPage, DockerContainerDetail, DockerContainerSummary};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use super::{ContainerSection, ContainerView, SelectedDetail};

fn bounds(visual: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    visual
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing container layout element: {selector}"))
}

fn assert_inside(parent: Bounds<Pixels>, child: Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label} must stay inside its parent: parent={parent:?}, child={child:?}"
    );
}

fn assert_disjoint(first: Bounds<Pixels>, second: Bounds<Pixels>, label: &str) {
    let separated = first.right() <= second.origin.x
        || second.right() <= first.origin.x
        || first.bottom() <= second.origin.y
        || second.bottom() <= first.origin.y;
    assert!(
        separated,
        "{label} must not overlap: first={first:?}, second={second:?}"
    );
}

fn assert_on_screen(element: Bounds<Pixels>, width: f32, height: f32, label: &str) {
    assert!(
        element.origin.x >= px(0.0)
            && element.origin.y >= px(0.0)
            && element.right() <= px(width)
            && element.bottom() <= px(height),
        "{label} must stay on screen at {width}x{height}: {element:?}"
    );
}

fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let center = bounds(visual, selector).center();
    visual.simulate_click(center, Modifiers::default());
    visual.run_until_parked();
}

fn container_page() -> ContainerPage<DockerContainerSummary> {
    ContainerPage {
        items: (0..40)
            .map(|index| DockerContainerSummary {
                id: format!("container-{index}"),
                names: vec![format!("/service-{index}")],
                image: Some("example/service:latest".into()),
                image_id: None,
                command: Some("service --serve".into()),
                created: Some(1_700_000_000),
                state: Some("running".into()),
                status: Some("Up 2 hours (healthy)".into()),
                health: Some("healthy".into()),
                ports: Vec::new(),
                networks: vec!["bridge".into()],
                labels: Vec::new(),
            })
            .collect(),
        page: 1,
        page_size: 100,
        total: 40,
        has_more: false,
    }
}

fn selected_container_detail() -> DockerContainerDetail {
    DockerContainerDetail {
        summary: DockerContainerSummary {
            id: "container-0".into(),
            names: vec!["/service-0".into()],
            image: Some("example/service:latest".into()),
            image_id: None,
            command: Some("service --serve".into()),
            created: Some(1_700_000_000),
            state: Some("running".into()),
            status: Some("Up 2 hours (healthy)".into()),
            health: Some("healthy".into()),
            ports: Vec::new(),
            networks: vec!["bridge".into()],
            labels: Vec::new(),
        },
        path: Some(format!("/srv/{}", "long-runtime-path-segment/".repeat(80))),
        args: vec!["--serve".into()],
        platform: Some("linux".into()),
        working_directory: Some("/srv/service".into()),
        entrypoint: vec!["/usr/bin/service".into()],
        command: vec!["service".into(), "--serve".into()],
        restart_policy: Some("unless-stopped".into()),
        env_keys: vec!["APP_ENV".into(), "LOG_LEVEL".into()],
        mounts: Vec::new(),
        networks: Vec::new(),
    }
}

/// Keeps a real long resource list and its selected detail visible in the same
/// content viewport across supported sizes and both application themes.
#[gpui_kit::test]
fn selected_container_detail_shares_the_viewport_and_closes_cleanly(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ContainerView::new(window, cx));
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = view_entity.expect("container view should be created");
    view.update(visual, |view, cx| {
        view.section = ContainerSection::Containers;
        view.loading = false;
        view.containers = Some(container_page());
        view.selected_detail = None;
        cx.notify();
    });

    for mode in [ramag_ui::Mode::Light, ramag_ui::Mode::Dark] {
        visual.update(|_, app| ramag_ui::apply_theme(mode, app));
        for (width, height) in [(1024.0, 768.0), (1440.0, 900.0), (360.0, 640.0)] {
            visual.simulate_resize(size(px(width), px(height)));
            view.update(visual, |view, cx| {
                // The detail represents the loaded result for the first real row.
                view.selected_detail = Some(SelectedDetail::Container(selected_container_detail()));
                cx.notify();
            });
            visual.run_until_parked();

            let workbench = bounds(visual, "container-view");
            let content = bounds(visual, "container-content");
            let panel = bounds(visual, "container-resource-panel");
            let table_frame = bounds(visual, "container-resource-table-frame");
            let detail = bounds(visual, "container-detail-panel");
            let detail_scroll = bounds(visual, "container-detail-scroll");
            let close = bounds(visual, "container-detail-close");
            let selected_row = bounds(visual, "container-resource-container-container-0");
            let row_count = visual.update(|_, app| {
                view.read(app)
                    .containers
                    .as_ref()
                    .expect("container rows should be loaded")
                    .items
                    .len()
            });

            assert_eq!(
                row_count, 40,
                "the layout fixture must retain forty real rows"
            );
            assert!(selected_row.size.width > px(0.0) && selected_row.size.height > px(0.0));
            for (label, element) in [
                ("resource panel", panel),
                ("table frame", table_frame),
                ("detail panel", detail),
                ("detail scroll viewport", detail_scroll),
                ("detail close control", close),
            ] {
                assert!(
                    element.size.width > px(0.0) && element.size.height > px(0.0),
                    "{label} must be visible at {width}x{height} in {mode:?}: {element:?}"
                );
                assert_inside(workbench, element, label);
                assert_on_screen(element, width, height, label);
            }
            assert_inside(content, panel, "resource panel in content area");
            assert_inside(panel, table_frame, "table frame in resource panel");
            assert_inside(panel, detail, "detail panel in resource panel");
            assert_inside(detail, detail_scroll, "independent detail scroll viewport");
            assert_inside(detail, close, "detail close control");
            assert_disjoint(table_frame, detail, "table and detail panes");

            let metrics_cancellation = Arc::new(AtomicBool::new(false));
            view.update(visual, |view, cx| {
                view.stats_loading = true;
                view.stats_cancellation = Some(metrics_cancellation.clone());
                cx.notify();
            });
            click(visual, "container-detail-close");
            assert!(
                visual.debug_bounds("container-detail-panel").is_none(),
                "closing details must remove the detail panel at {width}x{height} in {mode:?}"
            );
            assert!(visual.debug_bounds("container-detail-scroll").is_none());
            assert!(visual.debug_bounds("container-detail-close").is_none());
            assert!(
                metrics_cancellation.load(Ordering::Relaxed),
                "closing details must cancel an in-flight metrics request"
            );
            let (selected_detail, stats_loading, retained_rows) = visual.update(|_, app| {
                let view = view.read(app);
                (
                    view.selected_detail.is_some(),
                    view.stats_loading,
                    view.containers
                        .as_ref()
                        .expect("closing details must retain the resource page")
                        .items
                        .len(),
                )
            });
            assert!(!selected_detail, "closing details must clear the selection");
            assert!(
                !stats_loading,
                "closing details must clear the metrics loading state"
            );
            assert_eq!(retained_rows, 40, "closing details must not clear the list");

            let content = bounds(visual, "container-content");
            let panel = bounds(visual, "container-resource-panel");
            let table_frame = bounds(visual, "container-resource-table-frame");
            assert_inside(content, panel, "resource panel after closing details");
            assert_inside(
                panel,
                table_frame,
                "full-height table frame after closing details",
            );
            assert!(
                panel.bottom() >= content.bottom() - px(20.0),
                "resource panel should return to the content bottom: content={content:?}, panel={panel:?}"
            );
            assert!(
                table_frame.bottom() >= panel.bottom() - px(32.0),
                "table should refill the panel after closing details: panel={panel:?}, table={table_frame:?}"
            );
        }
    }
}
