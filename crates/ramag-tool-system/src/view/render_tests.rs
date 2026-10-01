use super::super::*;
use crate::{MonitorSnapshot, ReadingStatus, SensorSample, SystemMonitor};
use gpui_kit::component::Root;
use gpui_kit::{Context, TestAppContext, Window, px, size};
use ramag_infra_system::{
    Availability, MonitorDescriptor, MonitorKind, ProcessIdentity, ProcessRow, Reading,
    SensorDescriptor, SensorKind, Unit,
};
use std::cell::RefCell;
use std::rc::Rc;

macro_rules! required {
    ($value:expr, $message:literal) => {{
        let value = $value;
        assert!(value.is_some(), $message);
        match value {
            Some(value) => value,
            None => unreachable!("assertion above guarantees a value"),
        }
    }};
}

fn test_view(
    window: &mut Window,
    cx: &mut Context<SystemView>,
    snapshot: MonitorSnapshot,
) -> SystemView {
    let search = cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx));
    SystemView {
        monitor: SystemMonitor::with_snapshot(snapshot),
        section: SystemSection::Summary,
        termination_request: None,
        termination_focus: cx.focus_handle(),
        termination_focus_requested: false,
        termination_in_progress: false,
        notice: None,
        process_search: search,
        presentation: ramag_ui::MonitorPresentationSettings::default(),
        _settings_subscription: None,
        _search_subscription: None,
    }
}

fn process_snapshot() -> MonitorSnapshot {
    let reading = |sensor_id: &str, value: f64| Reading {
        sensor_id: sensor_id.into(),
        value: Some(value),
        total: None,
        availability: Availability::Available,
        reason: None,
        observations: Vec::new(),
    };
    MonitorSnapshot {
        host: ramag_infra_system::Snapshot {
            processes: vec![ProcessRow {
                identity: ProcessIdentity {
                    pid: 4242,
                    start_time_ticks: 918273,
                },
                name: "worker.exe".into(),
                user: Some("operator".into()),
                user_reason: None,
                cpu_percent: reading("process/4242/cpu", 12.5),
                memory_bytes: reading("process/4242/memory", 256.0 * 1024.0 * 1024.0),
                read_bytes_per_second: reading("process/4242/read", 0.0),
                write_bytes_per_second: reading("process/4242/write", 0.0),
                threads: reading("process/4242/threads", 5.0),
            }],
            ..Default::default()
        },
        ..Default::default()
    }
}

#[gpui_kit::test]
fn pulse_navigation_and_pages_fit_compact_and_desktop_windows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|view_cx| test_view(window, view_cx, MonitorSnapshot::default()));
        Root::new(view, window, cx)
    });
    for mode in [ramag_ui::Mode::Light, ramag_ui::Mode::Dark] {
        visual.update(|_, app| ramag_ui::apply_theme(mode, app));
        for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
            visual.simulate_resize(size(px(width), px(height)));
            for (section, selector) in [
                (SystemSection::Summary, "system-performance-body"),
                (SystemSection::Cpu, "system-page-cpu"),
                (SystemSection::Memory, "system-page-memory"),
                (SystemSection::Gpu, "system-page-gpu"),
                (SystemSection::Disks, "system-page-disks"),
                (SystemSection::Network, "system-page-network"),
                (SystemSection::Energy, "system-page-energy"),
                (SystemSection::Thermals, "system-page-thermals"),
                (SystemSection::Processes, "system-page-processes"),
                (SystemSection::Settings, "system-page-settings"),
            ] {
                visual.run_until_parked();
                assert!(visual.debug_bounds("system-root").is_some());
                for index in 0..SystemSection::ALL.len() {
                    assert!(
                        visual.debug_bounds(tab_selector(index)).is_some(),
                        "tab {index} missing at {width}x{height}"
                    );
                }
                let content = required!(visual.debug_bounds("system-content"), "content bounds");
                let header = required!(visual.debug_bounds("system-header"), "header bounds");
                assert!(content.left() >= px(0.0) && content.right() <= px(width));
                assert!(header.left() >= px(0.0) && header.right() <= px(width));
                let index = SystemSection::ALL
                    .iter()
                    .position(|entry| *entry == section)
                    .unwrap_or(usize::MAX);
                assert!(index < SystemSection::ALL.len(), "section index");
                let tab = visual.debug_bounds(tab_selector(index)).unwrap_or_default();
                visual.simulate_click(tab.center(), gpui_kit::Modifiers::default());
                visual.run_until_parked();
                assert!(
                    visual.debug_bounds(selector).is_some(),
                    "{} page did not open",
                    section.id()
                );
            }
        }
    }
}

fn tab_selector(index: usize) -> &'static str {
    match index {
        0 => "pulse-tab-0",
        1 => "pulse-tab-1",
        2 => "pulse-tab-2",
        3 => "pulse-tab-3",
        4 => "pulse-tab-4",
        5 => "pulse-tab-5",
        6 => "pulse-tab-6",
        7 => "pulse-tab-7",
        8 => "pulse-tab-8",
        9 => "pulse-tab-9",
        _ => "pulse-tab-invalid",
    }
}

#[gpui_kit::test]
fn missing_saved_device_stays_selected_and_explains_unavailability(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|view_cx| test_view(window, view_cx, MonitorSnapshot::default()));
        view.update(cx, |view, _| {
            view.section = SystemSection::Gpu;
            view.presentation
                .selected_devices
                .insert("gpu".into(), "gpu:removed-device".into());
        });
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    assert!(visual.debug_bounds("system-page-gpu").is_some());
    assert!(visual.debug_bounds("pulse-status-notice").is_some());
    assert!(visual.debug_bounds("pulse-device-selector").is_some());
}

#[gpui_kit::test]
fn current_zero_reading_and_unavailable_sensor_have_distinct_render_state(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let snapshot = MonitorSnapshot {
        host: ramag_infra_system::Snapshot {
            monitors: vec![MonitorDescriptor {
                id: "cpu-0".into(),
                title: "CPU".into(),
                kind: MonitorKind::Cpu,
                summary_sensor_id: "cpu.load".into(),
            }],
            sensors: vec![SensorDescriptor {
                id: "cpu.load".into(),
                monitor_id: "cpu-0".into(),
                title: "Utilization".into(),
                kind: SensorKind::Percentage,
                unit: Unit::Percent,
                source: "host".into(),
                scope: "All cores".into(),
                scale: Some(100.0),
            }],
            ..Default::default()
        },
        histories: std::collections::BTreeMap::from([(
            "cpu.load".into(),
            std::collections::VecDeque::from([SensorSample {
                at_seconds: 1.0,
                value: Some(0.0),
                total: None,
                status: ReadingStatus::Current,
                reason: None,
            }]),
        )]),
        ..Default::default()
    };
    assert_eq!(
        snapshot
            .latest("cpu.load")
            .and_then(SensorSample::chart_value),
        Some(0.0)
    );
    assert_eq!(
        snapshot
            .latest("absent")
            .and_then(SensorSample::chart_value),
        None
    );
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|view_cx| {
            let mut view = test_view(window, view_cx, snapshot);
            view.section = SystemSection::Cpu;
            view
        });
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    assert!(visual.debug_bounds("system-sensor-cpu-load").is_some());
}

#[gpui_kit::test]
fn device_selection_sensor_visibility_and_refresh_settings_apply_immediately(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::component::init);
    let gpu0 = MonitorDescriptor {
        id: "gpu0".into(),
        title: "Graphics 0".into(),
        kind: MonitorKind::Gpu,
        summary_sensor_id: "gpu0/usage".into(),
    };
    let gpu1 = MonitorDescriptor {
        id: "gpu1".into(),
        title: "Graphics 1".into(),
        kind: MonitorKind::Gpu,
        summary_sensor_id: "gpu1/usage".into(),
    };
    let snapshot = MonitorSnapshot {
        host: ramag_infra_system::Snapshot {
            monitors: vec![gpu0, gpu1],
            sensors: vec![
                SensorDescriptor {
                    id: "gpu0/usage".into(),
                    monitor_id: "gpu0".into(),
                    title: "Utilization".into(),
                    kind: SensorKind::Percentage,
                    unit: Unit::Percent,
                    source: "host".into(),
                    scope: "Device".into(),
                    scale: Some(100.0),
                },
                SensorDescriptor {
                    id: "gpu1/usage".into(),
                    monitor_id: "gpu1".into(),
                    title: "Utilization".into(),
                    kind: SensorKind::Percentage,
                    unit: Unit::Percent,
                    source: "host".into(),
                    scope: "Device".into(),
                    scale: Some(100.0),
                },
            ],
            ..Default::default()
        },
        ..Default::default()
    };
    let entity = Rc::new(RefCell::new(None));
    let entity_for_view = entity.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|view_cx| {
            let mut view = test_view(window, view_cx, snapshot);
            view.section = SystemSection::Gpu;
            view
        });
        *entity_for_view.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    let device = required!(visual.debug_bounds("pulse-device-1"), "second GPU selector");
    visual.simulate_click(device.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    let view = required!(entity.borrow().clone(), "system view missing");
    visual.update(|_, app| {
        assert_eq!(
            view.read(app)
                .presentation
                .selected_devices
                .get("gpu")
                .map(String::as_str),
            Some("gpu1")
        )
    });

    visual.update(|_, app| {
        view.update(app, |view, cx| {
            view.select_section(SystemSection::Settings, cx)
        })
    });
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();
    let checkbox = required!(
        visual.debug_bounds("system-sensor-visible-gpu1-usage"),
        "sensor visibility checkbox"
    );
    visual.simulate_click(checkbox.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    visual.update(|_, app| {
        assert!(
            view.read(app)
                .presentation
                .hidden_sensors
                .contains("gpu1/usage"),
            "view preference did not toggle"
        );
        assert!(
            ramag_ui::monitor_presentation_settings(app)
                .hidden_sensors
                .contains("gpu1/usage"),
            "preference was not saved globally"
        )
    });

    for (selector, interval) in [
        ("system-refresh-1s", crate::RefreshInterval::OneSecond),
        ("system-refresh-2s", crate::RefreshInterval::TwoSeconds),
        ("system-refresh-5s", crate::RefreshInterval::FiveSeconds),
    ] {
        let bounds = required!(visual.debug_bounds(selector), "refresh interval control");
        visual.simulate_click(bounds.center(), gpui_kit::Modifiers::default());
        visual.run_until_parked();
        visual.update(|_, app| assert_eq!(view.read(app).monitor.refresh_interval(), interval));
    }
}

#[gpui_kit::test]
fn termination_confirmation_keeps_the_captured_stable_identity(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|view_cx| {
            let mut view = test_view(window, view_cx, process_snapshot());
            view.section = SystemSection::Processes;
            view
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = required!(view_entity, "system view missing");
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();
    let kill = required!(
        visual.debug_bounds("system-kill-4242"),
        "process force-quit control"
    );
    visual.simulate_click(kill.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    visual.update(|_, app| {
        assert_eq!(
            view.read(app)
                .termination_request
                .as_ref()
                .map(|request| request.identity.clone()),
            Some(ProcessIdentity {
                pid: 4242,
                start_time_ticks: 918273
            })
        );
    });
    visual.update(|window, app| {
        assert!(view.read(app).termination_focus.is_focused(window));
    });
    visual.simulate_keystrokes("tab");
    visual.run_until_parked();
    visual.update(|window, app| {
        assert!(
            view.read(app)
                .termination_focus
                .contains_focused(window, app)
        );
    });
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    visual.update(|_, app| assert!(view.read(app).termination_request.is_none()));
}

#[gpui_kit::test]
fn confirmation_actions_fit_a_compact_360_by_240_view(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|view_cx| test_view(window, view_cx, MonitorSnapshot::default()));
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(360.0), px(240.0)));
    let view = required!(view_entity, "system view");
    visual.update(|_window, app| {
        let description = view.update(app, |this, cx| {
            this.prepare_termination(
                crate::StableProcessIdentity {
                    pid: 4242,
                    start_time_ticks: 1,
                },
                "long-running-worker-process".into(),
                cx,
            )
        });
        assert!(
            description.is_some(),
            "eligible process should open confirmation"
        );
    });
    visual.run_until_parked();
    let cancel = required!(
        visual.debug_bounds("system-kill-cancel"),
        "cancel button bounds"
    );
    let confirm = required!(
        visual.debug_bounds("system-kill-confirm"),
        "confirm button bounds"
    );
    let card = required!(
        visual.debug_bounds("system-termination-card"),
        "confirmation card bounds"
    );
    assert!(card.left() >= px(0.0) && card.right() <= px(360.0));
    assert!(card.top() >= px(0.0) && card.bottom() <= px(240.0));
    assert!(cancel.left() >= px(0.0) && cancel.right() <= px(360.0));
    assert!(confirm.left() >= px(0.0) && confirm.right() <= px(360.0));
    assert!(cancel.top() >= px(0.0) && cancel.bottom() <= px(240.0));
    assert!(confirm.top() >= px(0.0) && confirm.bottom() <= px(240.0));
    assert!(cancel.right() <= confirm.left());
    visual.update(|window, app| {
        assert!(view.read(app).termination_focus.is_focused(window));
    });
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    visual.update(|_, app| assert!(view.read(app).termination_request.is_none()));
}
