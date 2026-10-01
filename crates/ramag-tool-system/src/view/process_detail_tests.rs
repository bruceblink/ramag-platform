use super::*;
use gpui_kit::{Bounds, Pixels, VisualTestContext};
use ramag_infra_system::{Availability, ProcessIdentity};

fn two_process_snapshot() -> MonitorSnapshot {
    let mut snapshot = process_snapshot();
    let mut other = snapshot.host.processes[0].clone();
    other.identity = ProcessIdentity {
        pid: 4343,
        start_time_ticks: 112233,
    };
    other.name = "second-worker.exe".into();
    snapshot.host.processes.push(other);
    snapshot
}

fn process_row_selector(pid: u32) -> &'static str {
    match pid {
        4242 => "system-process-row-4242",
        4343 => "system-process-row-4343",
        _ => unreachable!("known test PID"),
    }
}

fn metric_selectors() -> [&'static str; 6] {
    [
        "system-process-detail-cpu",
        "system-process-detail-memory",
        "system-process-detail-read",
        "system-process-detail-write",
        "system-process-detail-threads",
        "system-process-detail-user",
    ]
}

fn contained(child: Bounds<Pixels>, parent: Bounds<Pixels>) {
    assert!(child.left() >= parent.left() && child.right() <= parent.right());
    assert!(child.top() >= parent.top() && child.bottom() <= parent.bottom());
}

fn open_details(visual: &mut VisualTestContext, pid: u32) {
    let row = required!(
        visual.debug_bounds(process_row_selector(pid)),
        "process row"
    );
    visual.simulate_click(row.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
}

/// Selecting a real row exposes its captured identity and close control without requesting termination.
#[gpui_kit::test]
fn clicking_a_process_row_opens_identity_bound_details(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let mut view = test_view(window, cx, process_snapshot());
            view.section = SystemSection::Processes;
            view
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = required!(view_entity, "system view");
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();

    open_details(visual, 4242);
    assert!(
        visual
            .debug_bounds("system-process-details-header")
            .is_some()
    );
    assert!(
        visual
            .debug_bounds("system-process-details-close")
            .is_some()
    );
    for selector in [
        "system-process-detail-name",
        "system-process-detail-pid",
        "system-process-detail-user",
        "system-process-detail-sample-time",
    ] {
        assert!(
            visual.debug_bounds(selector).is_some(),
            "missing {selector}"
        );
    }
    for selector in metric_selectors() {
        assert!(
            visual.debug_bounds(selector).is_some(),
            "missing {selector}"
        );
    }
    visual.update(|_, app| {
        let view = view.read(app);
        let selected = required!(view.selected_process.as_ref(), "selected identity");
        assert_eq!(selected.identity.pid, 4242);
        assert_eq!(selected.identity.start_time_ticks, 918273);
        assert!(view.termination_request.is_none());
    });

    let close = required!(
        visual.debug_bounds("system-process-details-close"),
        "close details"
    );
    visual.simulate_click(close.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    assert!(visual.debug_bounds("system-process-details").is_none());
    visual.update(|_, app| assert!(view.read(app).selected_process.is_none()));
}

/// Sorting, filtering, and navigation preserve selection; a force-quit click on another row does not replace it.
#[gpui_kit::test]
fn selection_survives_process_list_changes_and_unrelated_row_actions(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let mut view = test_view(window, cx, two_process_snapshot());
            view.section = SystemSection::Processes;
            view
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = required!(view_entity, "system view");
    visual.simulate_resize(size(px(1024.0), px(1200.0)));
    visual.run_until_parked();
    open_details(visual, 4242);

    let sort = required!(visual.debug_bounds("system-process-sort-name"), "name sort");
    visual.simulate_click(sort.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    visual.update(|_, app| {
        assert_eq!(
            view.read(app)
                .selected_process
                .as_ref()
                .map(|item| item.identity.pid),
            Some(4242)
        )
    });

    visual.update(|window, app| {
        view.update(app, |this, cx| {
            this.process_search.update(cx, |state, cx| {
                state.set_value("no-matching-process", window, cx);
                cx.emit(gpui_kit::component::input::InputEvent::Change);
            });
        });
    });
    visual.run_until_parked();
    assert!(visual.debug_bounds("system-process-row-4242").is_none());
    assert!(visual.debug_bounds("system-process-detail-pid").is_some());
    visual.update(|_, app| {
        assert_eq!(
            view.read(app)
                .selected_process
                .as_ref()
                .map(|item| item.identity.pid),
            Some(4242)
        )
    });

    visual.update(|window, app| {
        view.update(app, |this, cx| {
            this.process_search.update(cx, |state, cx| {
                state.set_value("", window, cx);
                cx.emit(gpui_kit::component::input::InputEvent::Change);
            });
        });
    });
    visual.run_until_parked();
    for (index, expected) in [(0, SystemSection::Summary), (8, SystemSection::Processes)] {
        let tab = required!(visual.debug_bounds(tab_selector(index)), "navigation tab");
        visual.simulate_click(tab.center(), gpui_kit::Modifiers::default());
        visual.run_until_parked();
        visual.update(|_, app| assert_eq!(view.read(app).section, expected));
    }
    assert!(visual.debug_bounds("system-process-detail-pid").is_some());

    let kill = required!(visual.debug_bounds("system-kill-4343"), "other row action");
    visual.simulate_click(kill.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    visual.update(|_, app| {
        let view = view.read(app);
        assert_eq!(
            view.selected_process.as_ref().map(|item| item.identity.pid),
            Some(4242)
        );
        assert_eq!(
            view.termination_request
                .as_ref()
                .map(|request| request.identity.pid),
            Some(4343)
        );
    });
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    visual.update(|_, app| {
        assert_eq!(
            view.read(app)
                .selected_process
                .as_ref()
                .map(|item| item.identity.pid),
            Some(4242)
        )
    });
}

/// Re-resolving after monitor updates distinguishes current, missing, reused, and stale identities.
#[gpui_kit::test]
fn detail_state_tracks_latest_snapshot_without_reassigning_pid_reuse(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let mut view = test_view(window, cx, process_snapshot());
            view.section = SystemSection::Processes;
            view
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = required!(view_entity, "system view");
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();
    open_details(visual, 4242);

    let mut updated = process_snapshot();
    updated.host.processes[0].cpu_percent.value = Some(73.0);
    visual.update(|_, app| {
        view.update(app, |this, cx| {
            this.monitor = SystemMonitor::with_snapshot(updated);
            cx.notify();
        });
    });
    visual.run_until_parked();
    visual.update(|_, app| {
        let view = view.read(app);
        let selected = required!(view.selected_process.as_ref(), "selected identity");
        assert_eq!(selected.identity.start_time_ticks, 918273);
        assert_eq!(
            view.monitor.snapshot().host.processes[0].cpu_percent.value,
            Some(73.0)
        );
    });
    assert!(visual.debug_bounds("system-process-detail-cpu").is_some());

    let mut missing = MonitorSnapshot::default();
    visual.update(|_, app| {
        view.update(app, |this, cx| {
            this.monitor = SystemMonitor::with_snapshot(missing.clone());
            cx.notify();
        });
    });
    visual.run_until_parked();
    assert!(
        visual
            .debug_bounds("system-process-detail-missing")
            .is_some()
    );
    for selector in ["system-process-detail-name", "system-process-detail-pid"] {
        assert!(
            visual.debug_bounds(selector).is_some(),
            "captured {selector}"
        );
    }
    for selector in metric_selectors() {
        assert!(
            visual.debug_bounds(selector).is_none(),
            "stale metric {selector}"
        );
    }

    missing = process_snapshot();
    missing.host.processes[0].identity.start_time_ticks += 1;
    visual.update(|_, app| {
        view.update(app, |this, cx| {
            this.monitor = SystemMonitor::with_snapshot(missing);
            cx.notify();
        });
    });
    visual.run_until_parked();
    assert!(
        visual
            .debug_bounds("system-process-detail-reused")
            .is_some()
    );
    for selector in ["system-process-detail-name", "system-process-detail-pid"] {
        assert!(
            visual.debug_bounds(selector).is_some(),
            "captured {selector}"
        );
    }
    for selector in metric_selectors() {
        assert!(
            visual.debug_bounds(selector).is_none(),
            "reused metric {selector}"
        );
    }
    visual.update(|_, app| {
        let selected = required!(
            view.read(app).selected_process.as_ref(),
            "original identity"
        );
        assert_eq!(selected.identity.start_time_ticks, 918273);
    });

    let mut stale = process_snapshot();
    stale.collection_stale = true;
    visual.update(|_, app| {
        view.update(app, |this, cx| {
            this.monitor = SystemMonitor::with_snapshot(stale);
            cx.notify();
        });
    });
    visual.run_until_parked();
    assert!(visual.debug_bounds("system-process-detail-stale").is_some());
    assert!(
        visual
            .debug_bounds("system-process-detail-reason-cpu")
            .is_some()
    );
}

/// A long diagnostic payload proves the detail viewport stays bounded and can scroll in every supported layout.
#[gpui_kit::test]
fn long_process_details_remain_reachable_in_light_dark_and_large_text(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut snapshot = process_snapshot();
    let process = &mut snapshot.host.processes[0];
    process.name =
        "worker-with-a-very-long-unbroken-process-name-abcdefghijklmnopqrstuvwxyz.exe".into();
    process.user = None;
    process.user_reason = Some("user lookup failed: source identity was not available".into());
    process.cpu_percent.value = None;
    process.cpu_percent.availability = Availability::Failed;
    process.cpu_percent.reason = Some("counter read failed: source remained unavailable".into());
    let mut view_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let mut view = test_view(window, cx, snapshot);
            view.section = SystemSection::Processes;
            view
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = required!(view_entity, "system view");

    for mode in [ramag_ui::Mode::Light, ramag_ui::Mode::Dark] {
        for text_size in [
            ramag_ui::InterfaceTextSize::Standard,
            ramag_ui::InterfaceTextSize::Large,
        ] {
            visual.update(|_, app| {
                ramag_ui::apply_theme(mode, app);
                ramag_ui::set_system_settings(
                    ramag_ui::SystemSettings {
                        text_size,
                        ..Default::default()
                    },
                    app,
                );
            });
            for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
                visual.simulate_resize(size(px(width), px(height)));
                visual.run_until_parked();
                if visual.debug_bounds("system-process-details").is_none() {
                    open_details(visual, 4242);
                }
                let section = required!(
                    visual.debug_bounds("system-page-processes"),
                    "process section"
                );
                let header = required!(
                    visual.debug_bounds("system-process-details-header"),
                    "fixed details header"
                );
                let close = required!(
                    visual.debug_bounds("system-process-details-close"),
                    "close details"
                );
                let body = required!(
                    visual.debug_bounds("system-process-details-body"),
                    "scroll body"
                );
                contained(header, section);
                contained(close, header);
                assert!(section.top() <= header.top() && header.bottom() <= section.bottom());
                assert!(section.left() <= close.left() && close.right() <= section.right());
                assert!(body.size.height <= px((height * 0.4).min(260.0) + 1.0));
                assert!(body.size.height > px(0.0));
                assert!(
                    visual
                        .debug_bounds("system-process-detail-reason-user")
                        .is_some()
                );
                assert!(
                    visual
                        .debug_bounds("system-process-detail-reason-cpu")
                        .is_some()
                );

                visual.simulate_event(gpui_kit::ScrollWheelEvent {
                    position: body.center(),
                    delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.0), px(-180.0))),
                    modifiers: gpui_kit::Modifiers::default(),
                    touch_phase: gpui_kit::TouchPhase::Moved,
                });
                visual.run_until_parked();
                visual.update(|_, app| {
                    assert!(
                        view.read(app).process_detail_scroll.offset().y < px(0.0),
                        "detail body should scroll at {width}x{height}"
                    );
                });
                assert!(
                    visual
                        .debug_bounds("system-process-details-header")
                        .is_some()
                );
                assert!(
                    visual
                        .debug_bounds("system-process-details-close")
                        .is_some()
                );
                let close = required!(
                    visual.debug_bounds("system-process-details-close"),
                    "close after scrolling"
                );
                visual.simulate_click(close.center(), gpui_kit::Modifiers::default());
                visual.run_until_parked();
            }
        }
    }
}
