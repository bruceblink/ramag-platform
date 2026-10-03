use super::*;
#[cfg(test)]
use crate::test_support::TestUnwrapExt;

#[gpui_kit::test]
fn summary_top_cpu_processes_follow_live_snapshots(cx: &mut TestAppContext) {
    let (view, cx) = populated(cx);
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert_eq!(
            data.processes.len(),
            2,
            "Summary needs its visible process rows"
        );
        assert_eq!(data.processes[0].cells[1], "compiler");
    });
    let compiler = cx
        .debug_bounds("summary-process:401:40100:name")
        .test_unwrap();
    let idle = cx
        .debug_bounds("summary-process:402:40200:name")
        .test_unwrap();
    assert!(
        compiler.origin.y < idle.origin.y,
        "Summary sorts by CPU usage"
    );
    command(&view, crate::workspace::Command::Screen(Screen::Cpu), cx);
    let mut next = fixture::snapshot(6);
    next.processes[0].identity.start_time_ticks += 1;
    next.processes[0].name = "replacement compiler".into();
    next.processes[0].cpu_percent.value = Some(0.);
    next.processes[1].cpu_percent.value = Some(80.);
    accept(&view, next, cx);
    assert!(cx.read(|cx| view.read(cx).shared.borrow().processes.is_empty()));
    command(
        &view,
        crate::workspace::Command::Screen(Screen::Summary),
        cx,
    );
    assert!(cx.debug_bounds("summary-process:401:40100:name").is_none());
    let compiler = cx
        .debug_bounds("summary-process:401:40101:name")
        .test_unwrap();
    let idle = cx
        .debug_bounds("summary-process:402:40200:name")
        .test_unwrap();
    assert!(
        idle.origin.y < compiler.origin.y,
        "Latest CPU changes the visible order"
    );
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert_eq!(data.processes[0].cells[1], "replacement compiler");
        assert_eq!(data.snapshot.as_ref().test_unwrap().sequence, 6);
    });
}

#[gpui_kit::test]
fn hidden_process_rows_are_prepared_from_the_latest_identity_on_display(cx: &mut TestAppContext) {
    let (view, cx) = harness(cx);
    command(&view, crate::workspace::Command::Screen(Screen::Cpu), cx);
    for sequence in 1..=5 {
        accept(&view, fixture::snapshot(sequence), cx);
    }
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert!(
            data.processes.is_empty(),
            "CPU charts do not need process table cells"
        );
        assert_eq!(data.process_count(), 2);
    });
    command(
        &view,
        crate::workspace::Command::Screen(Screen::Processes),
        cx,
    );
    let identity = cx.read(|cx| view.read(cx).shared.borrow().processes[0].identity.clone());
    cx.update(|_, cx| {
        view.read(cx)
            .processes
            .clone()
            .update(cx, |panel, _| panel.selected = Some(identity.clone()));
    });
    command(&view, crate::workspace::Command::Screen(Screen::Cpu), cx);
    accept(&view, fixture::snapshot(6), cx);
    cx.read(|cx| {
        assert_eq!(
            view.read(cx).processes.read(cx).selected.as_ref(),
            Some(&identity)
        );
        assert!(view.read(cx).shared.borrow().processes.is_empty());
    });
    let mut replacement = fixture::snapshot(7);
    replacement.processes.truncate(1);
    replacement.processes[0].identity.start_time_ticks += 1;
    replacement.processes[0].name = "replacement process".into();
    replacement.processes[0].cpu_percent.availability = ramag_infra_system::Availability::Failed;
    replacement.processes[0].cpu_percent.value = None;
    replacement.processes[0].cpu_percent.reason = Some("Permission denied".into());
    accept(&view, replacement, cx);
    cx.read(|cx| {
        let screen = view.read(cx);
        assert!(screen.processes.read(cx).selected.is_none());
        assert!(screen.shared.borrow().processes.is_empty());
        assert_eq!(screen.shared.borrow().process_count(), 1);
    });
    command(
        &view,
        crate::workspace::Command::Screen(Screen::Processes),
        cx,
    );
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert_eq!(data.processes.len(), 1);
        assert_eq!(data.processes[0].cells[1], "replacement process");
        assert!(data.processes[0].cells[2].contains("No access"));
        assert_ne!(data.processes[0].identity, identity);
    });
}

#[gpui_kit::test]
fn reopening_processes_uses_background_snapshots_and_retained_history(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut application = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let app = cx.new(|cx| ApplicationView::new_fixture(window, cx));
        application = Some(app.clone());
        gpui_kit::component::Root::new(app, window, cx)
    });
    let app = application.test_unwrap();
    let screen = cx.read(|cx| app.read(cx).screens.clone());
    accept(&screen, fixture::snapshot(1), cx);
    command(
        &screen,
        crate::workspace::Command::Screen(Screen::Processes),
        cx,
    );
    let shared = cx.read(|cx| screen.read(cx).shared.clone());
    let owner = shared.borrow().owner.clone().test_unwrap();
    cx.update(|_, cx| app.update(cx, |app, cx| app.detach_window(cx)));
    for sequence in 2..=5 {
        let mut snapshot = fixture::snapshot(sequence);
        snapshot.processes[0].name = format!("background {sequence}");
        cx.update(|_, cx| {
            owner
                .update(cx, |owner, cx| {
                    owner.accept_background_snapshot(snapshot, cx)
                })
                .test_unwrap()
        });
        assert!(shared.borrow().processes.is_empty());
    }
    let history_len = shared
        .borrow()
        .history
        .samples("cpu:host", "cpu:host/usage")
        .test_unwrap()
        .len();
    assert_eq!(history_len, 5);
    cx.update(|window, cx| app.update(cx, |app, cx| app.attach_window(window, cx)));
    draw(cx);
    let data = shared.borrow();
    assert_eq!(data.snapshot.as_ref().test_unwrap().sequence, 5);
    assert_eq!(data.processes[0].cells[1], "background 5");
    assert_eq!(
        data.history
            .samples("cpu:host", "cpu:host/usage")
            .test_unwrap()
            .len(),
        history_len
    );
}

#[gpui_kit::test]
fn summary_graphs_fill_rows_when_the_window_resizes(cx: &mut TestAppContext) {
    let (_, cx) = populated(cx);
    for (width, columns) in [
        (1800., 5),
        (2560., 5),
        (1280., 3),
        (960., 3),
        (1680., 5),
        (1679., 3),
    ] {
        cx.simulate_resize(gpui_kit::size(gpui_kit::px(width), gpui_kit::px(1200.)));
        draw(cx);
        let charts: Vec<_> = [
            "summary-history:disks",
            "summary-history:network",
            "summary-history:energy",
            "summary-history:gpu",
            "summary-history:thermals",
        ]
        .into_iter()
        .map(|selector| cx.debug_bounds(selector).test_unwrap())
        .collect();
        for row in charts.chunks(columns) {
            assert!(
                row.iter().all(|bounds| bounds.origin.y == row[0].origin.y),
                "Summary cards wrapped before the row was full at {width}px: {charts:?}"
            );
            assert_eq!(row[0].origin.x, charts[0].origin.x);
            let right = row.last().test_unwrap().right();
            assert!(
                (right.as_f32() - (width - 25.)).abs() <= 1.,
                "Summary row left unused space at {width}px: {charts:?}"
            );
            for pair in row.windows(2) {
                assert!(pair[0].right() < pair[1].origin.x);
            }
            assert!(row.iter().all(|bounds| bounds.size.width.as_f32() >= 280.));
        }
        if columns < charts.len() {
            assert!(charts[columns].origin.y > charts[0].bottom());
        }
    }
}

#[gpui_kit::test]
fn detached_workspace_keeps_cpu_history_and_preferences(cx: &mut TestAppContext) {
    let (view, cx) = populated(cx);
    command(&view, crate::workspace::Command::Screen(Screen::Memory), cx);
    let owner = cx.read(|cx| view.read(cx).shared.borrow().owner.clone().test_unwrap());
    cx.update(|_, cx| {
        owner
            .update(cx, |owner, cx| owner.detach_window(cx))
            .test_unwrap();
        for sequence in 6..=130 {
            owner
                .update(cx, |owner, cx| {
                    owner.accept_background_snapshot(fixture::snapshot(sequence), cx)
                })
                .test_unwrap();
        }
    });
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert_eq!(data.snapshot.as_ref().test_unwrap().sequence, 130);
        assert_eq!(
            data.history
                .samples("cpu:host", "cpu:host/usage")
                .test_unwrap()
                .len(),
            120
        );
        assert_eq!(data.session.workspace.screens.active, Screen::Memory);
    });
    cx.update(|window, cx| {
        owner
            .update(cx, |owner, cx| owner.attach_window(window, cx))
            .test_unwrap()
    });
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert_eq!(data.snapshot.as_ref().test_unwrap().sequence, 130);
        assert_eq!(data.session.workspace.screens.active, Screen::Memory);
        assert_eq!(
            data.history
                .samples("cpu:host", "cpu:host/usage")
                .test_unwrap()
                .len(),
            120
        );
    });
}

#[gpui_kit::test]
fn network_default_prefers_route_but_preserves_explicit_selection(cx: &mut TestAppContext) {
    let (view, cx) = populated(cx);
    let mut snapshot = fixture::snapshot(6);
    snapshot
        .monitors
        .push(ramag_infra_system::MonitorDescriptor {
            id: "network:docker0".into(),
            title: "Docker bridge".into(),
            kind: ramag_infra_system::MonitorKind::Network,
            summary_sensor_id: "network:docker0/rx".into(),
        });
    snapshot.preferred_network_monitor_id = Some("network:eth0".into());
    accept(&view, snapshot.clone(), cx);
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert_eq!(
            crate::screen_data::devices(&data, Screen::Network)[0].id,
            "network:docker0"
        );
        assert_eq!(
            crate::screen_data::selected_device(&data, Screen::Network).as_deref(),
            Some("network:eth0")
        );
    });
    command(
        &view,
        crate::workspace::Command::ScreenDevice(Screen::Network, "network:docker0".into()),
        cx,
    );
    snapshot.sequence = 7;
    snapshot.monitors.reverse();
    accept(&view, snapshot, cx);
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert_eq!(
            crate::screen_data::selected_device(&data, Screen::Network).as_deref(),
            Some("network:docker0")
        );
    });
}

#[gpui_kit::test]
fn every_tab_is_clickable_and_preserves_the_legacy_layout(cx: &mut TestAppContext) {
    let (view, cx) = harness(cx);
    let before = cx.read(|cx| view.read(cx).shared.borrow().session.workspace.dock.clone());
    for (screen, selector) in Screen::ALL.into_iter().zip([
        "screen-tab:summary",
        "screen-tab:cpu",
        "screen-tab:memory",
        "screen-tab:gpu",
        "screen-tab:disks",
        "screen-tab:network",
        "screen-tab:energy",
        "screen-tab:thermals",
        "screen-tab:processes",
    ]) {
        let bounds = cx
            .debug_bounds(selector)
            .test_unwrap_msg("tab is in production root");
        cx.simulate_click(bounds.center(), Modifiers::default());
        draw(cx);
        assert_eq!(active(&view, cx), screen);
        cx.read(|cx| assert_eq!(view.read(cx).shared.borrow().session.workspace.dock, before));
    }
    assert!(
        cx.debug_bounds("screen-tab:settings").is_none(),
        "the removed monitor Settings screen must not be navigable"
    );
}

#[gpui_kit::test]
fn arrows_home_end_and_control_tab_switch_screens(cx: &mut TestAppContext) {
    let (view, cx) = harness(cx);
    cx.update(|window, cx| {
        view.read(cx).focus[&Screen::Summary]
            .handle
            .clone()
            .focus(window, cx)
    });
    native_key("right", cx);
    draw(cx);
    assert_eq!(active(&view, cx), Screen::Cpu);
    native_key("end", cx);
    draw(cx);
    assert_eq!(active(&view, cx), Screen::Processes);
    native_key("ctrl-tab", cx);
    draw(cx);
    assert_eq!(active(&view, cx), Screen::Summary);
    native_key("ctrl-shift-tab", cx);
    draw(cx);
    assert_eq!(active(&view, cx), Screen::Processes);
    native_key("home", cx);
    draw(cx);
    assert_eq!(active(&view, cx), Screen::Summary);
}

#[gpui_kit::test]
fn keyboard_switching_works_immediately_after_application_creation(cx: &mut TestAppContext) {
    let (view, cx) = harness(cx);
    assert_eq!(active(&view, cx), Screen::Summary);
    native_key("ctrl-tab", cx);
    draw(cx);
    assert_eq!(active(&view, cx), Screen::Cpu);
}

#[gpui_kit::test]
fn keyboard_switching_survives_builtin_preset_from_settings(cx: &mut TestAppContext) {
    let (application, cx) = crate::native_tests::application_harness(cx);
    let view = cx.read(|cx| application.read(cx).screens.clone());
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.select(Screen::Settings, window, cx);
        })
    });
    draw(cx);
    assert_eq!(active(&view, cx), Screen::Settings);
    assert!(cx.debug_bounds("open-ramag-settings").is_some());
    draw(cx);
    command(
        &view,
        crate::workspace::Command::Preset(crate::workspace::presets::PresetCommand::Builtin(
            system_pulse_model::BuiltinPreset::Default,
        )),
        cx,
    );
    assert_eq!(active(&view, cx), Screen::Summary);
    native_key("ctrl-tab", cx);
    draw(cx);
    assert_eq!(active(&view, cx), Screen::Cpu);
}

#[gpui_kit::test]
fn keyboard_switching_survives_accepting_recovered_settings(cx: &mut TestAppContext) {
    let (view, cx) = harness(cx);
    cx.update(|window, cx| {
        let owner = view.read(cx).shared.borrow().owner.clone().test_unwrap();
        owner
            .update(cx, |owner, cx| {
                owner.restore("invalid saved workspace", window, cx)
            })
            .test_unwrap();
    });
    draw(cx);
    cx.read(|cx| assert!(view.read(cx).shared.borrow().session.rejected.is_some()));
    let recovery = cx.debug_bounds("accept-screen-recovery").test_unwrap();
    cx.simulate_click(recovery.center(), Modifiers::default());
    draw(cx);
    assert!(cx.debug_bounds("accept-screen-recovery").is_none());
    cx.read(|cx| assert!(view.read(cx).shared.borrow().session.rejected.is_none()));
    assert_eq!(active(&view, cx), Screen::Summary);
    // The focused recovery button has disappeared. Do not click a new focus target.
    native_key("ctrl-tab", cx);
    draw(cx);
    assert_eq!(active(&view, cx), Screen::Cpu);
}

#[gpui_kit::test]
fn collapsed_hidden_legacy_process_panel_cannot_hide_the_process_screen(cx: &mut TestAppContext) {
    let (view, cx) = harness(cx);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let shared = view.shared.clone();
            let mut data = shared.borrow_mut();
            let state = data.session.workspace.panel_mut("processes");
            state.visible = false;
            state.collapsed = true;
            drop(data);
            view.select(Screen::Processes, window, cx);
        })
    });
    draw(cx);
    assert!(cx.debug_bounds("process-table").is_some());
    assert_eq!(active(&view, cx), Screen::Processes);
}

#[gpui_kit::test]
fn tab_switching_preserves_process_identity_selection(cx: &mut TestAppContext) {
    let (view, cx) = harness(cx);
    let identity = cx.read(|cx| view.read(cx).shared.borrow().processes[0].identity.clone());
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.processes
                .update(cx, |panel, _| panel.selected = Some(identity.clone()));
            view.select(Screen::Memory, window, cx);
        })
    });
    draw(cx);
    cx.update(|window, cx| view.update(cx, |view, cx| view.select(Screen::Processes, window, cx)));
    draw(cx);
    cx.read(|cx| {
        assert_eq!(
            view.read(cx).processes.read(cx).selected.as_ref(),
            Some(&identity)
        )
    });
    assert!(cx.debug_bounds("process-details").is_some());
}
