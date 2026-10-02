use super::super::*;
use crate::{MonitorSnapshot, ReadingStatus, SensorSample, SystemMonitor};
use gpui_kit::component::Root;
use gpui_kit::{Context, TestAppContext, Window, px, size};
use ramag_infra_system::{
    Availability, MonitorDescriptor, MonitorKind, ProcessIdentity, ProcessRow, Reading,
    SensorDescriptor, SensorKind, Unit,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

#[path = "thermals_render_tests.rs"]
mod thermal_render_tests;

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
    let search =
        cx.new(|cx| ramag_ui::bounded_search_input(window, cx).placeholder("名称 / PID / 用户"));
    let subscription = cx.subscribe_in(&search, window, |_, _, event: &InputEvent, _, cx| {
        if matches!(event, InputEvent::Change) {
            cx.notify();
        }
    });
    SystemView {
        monitor: SystemMonitor::with_snapshot(snapshot),
        section: SystemSection::Summary,
        termination_request: None,
        termination_focus: cx.focus_handle(),
        termination_scroll: gpui_kit::ScrollHandle::new(),
        termination_focus_requested: false,
        termination_in_progress: false,
        notice: None,
        #[cfg(target_os = "windows")]
        cpu_temperature_request_in_flight: false,
        process_search: search,
        process_table_scroll: gpui_kit::ScrollHandle::new(),
        selected_process: None,
        process_detail_scroll: gpui_kit::ScrollHandle::new(),
        presentation: ramag_ui::MonitorPresentationSettings::default(),
        _settings_subscription: None,
        _search_subscription: Some(subscription),
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
        ("system-refresh-0.5s", crate::RefreshInterval::HalfSecond),
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
fn persisted_half_second_refresh_is_applied_when_monitor_starts(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    cx.update(|app| {
        ramag_ui::set_monitor_settings(
            ramag_ui::MonitorSettings {
                refresh_rate: ramag_ui::MonitorRefreshRate::HalfSecond,
            },
            app,
        );
    });
    let monitor = SystemMonitor::with_snapshot(MonitorSnapshot::default());
    cx.update(|app| apply_monitor_preferences(&monitor, app));
    assert_eq!(monitor.refresh_interval(), RefreshInterval::HalfSecond);
}

fn energy_snapshot() -> MonitorSnapshot {
    let sensor = |id: &str, title: &str, source: &str, scope: &str| SensorDescriptor {
        id: id.into(),
        monitor_id: "power".into(),
        title: title.into(),
        kind: SensorKind::Power,
        unit: Unit::Watts,
        source: source.into(),
        scope: scope.into(),
        scale: None,
    };
    let sample = |value: f64| SensorSample {
        at_seconds: 1.0,
        value: Some(value),
        total: None,
        status: ReadingStatus::Current,
        reason: None,
    };
    let sensors = vec![
        sensor(
            "cpu:host/power",
            "CPU package power",
            "windows-energy",
            "CPU package",
        ),
        sensor(
            "gpu:0/power",
            "NVIDIA GPU power",
            "windows-gpu",
            "GPU board",
        ),
    ];
    MonitorSnapshot {
        host: ramag_infra_system::Snapshot {
            sensors,
            ..Default::default()
        },
        histories: BTreeMap::from([
            ("cpu:host/power".into(), VecDeque::from([sample(65.0)])),
            ("gpu:0/power".into(), VecDeque::from([sample(14.0)])),
        ]),
        ..Default::default()
    }
}

#[test]
fn energy_selection_preserves_stable_ids_and_missing_saved_sensor() {
    let snapshot = energy_snapshot();
    let readings = snapshot.host.sensors.iter().collect::<Vec<_>>();
    assert_eq!(
        crate::view::pages::selected_power_sensor(&readings, None).map(|sensor| sensor.id.as_str()),
        Some("cpu:host/power")
    );
    assert_eq!(
        crate::view::pages::selected_power_sensor(&readings, Some("gpu:0/power"))
            .map(|sensor| sensor.id.as_str()),
        Some("gpu:0/power")
    );
    assert!(
        crate::view::pages::selected_power_sensor(&readings, Some("power:removed")).is_none(),
        "a missing saved sensor must not silently select another channel"
    );
    let reversed = readings.iter().copied().rev().collect::<Vec<_>>();
    assert_eq!(
        crate::view::pages::selected_power_sensor(&reversed, None).map(|sensor| sensor.id.as_str()),
        Some("cpu:host/power"),
        "CPU package power is the stable default when no preference exists"
    );
    let mut stale = snapshot.clone();
    stale.collection_stale = true;
    assert_eq!(
        crate::view::pages::displayed_sensor_state(&stale, stale.latest("cpu:host/power")),
        (ReadingStatus::Stale, None),
        "a stalled collection must not expose an old sample as live power"
    );
}

#[gpui_kit::test]
fn energy_page_renders_selected_primary_and_measured_channels(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let entity = Rc::new(RefCell::new(None));
    let entity_for_view = entity.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|view_cx| {
            let mut view = test_view(window, view_cx, energy_snapshot());
            view.section = SystemSection::Energy;
            view
        });
        *entity_for_view.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();
    for selector in [
        "system-page-energy",
        "system-energy-sensor-selector",
        "system-energy-primary-meter",
        "system-energy-primary-value",
        "system-energy-primary-chart",
        "system-energy-domain",
        "system-energy-measured-channels",
        "system-sensor-cpu-host-power",
        "system-sensor-gpu-0-power",
    ] {
        assert!(
            visual.debug_bounds(selector).is_some(),
            "missing {selector}"
        );
    }
    let selector = required!(
        visual.debug_bounds("system-energy-sensor-selector"),
        "energy sensor selector"
    );
    visual.simulate_click(selector.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    let option = required!(
        visual.debug_bounds("system-energy-sensor-option-gpu-0-power"),
        "GPU power option"
    );
    visual.simulate_click(option.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    let view = required!(entity.borrow().clone(), "system view missing");
    visual.update(|_, app| {
        assert_eq!(
            view.read(app)
                .presentation
                .selected_sensors
                .get("energy")
                .map(String::as_str),
            Some("gpu:0/power")
        );
        assert_eq!(
            ramag_ui::monitor_presentation_settings(app)
                .selected_sensors
                .get("energy")
                .map(String::as_str),
            Some("gpu:0/power")
        );
    });
    for width in [360.0, 1024.0, 1440.0] {
        visual.simulate_resize(size(px(width), px(768.0)));
        visual.run_until_parked();
        let page = required!(visual.debug_bounds("system-page-energy"), "energy page");
        let primary = required!(
            visual.debug_bounds("system-energy-primary-meter"),
            "energy primary meter"
        );
        let channels = required!(
            visual.debug_bounds("system-energy-measured-channels"),
            "energy measured channels"
        );
        assert!(
            primary.left() >= page.left() && primary.right() <= page.right(),
            "primary meter overflows {width}px Energy page"
        );
        assert!(
            channels.left() >= page.left() && channels.right() <= page.right(),
            "measured channels overflow {width}px Energy page"
        );
        for selector in ["system-sensor-cpu-host-power", "system-sensor-gpu-0-power"] {
            let card = required!(visual.debug_bounds(selector), "energy sensor card");
            assert!(
                card.left() >= channels.left() && card.right() <= channels.right(),
                "{selector} overflows {width}px measured channels"
            );
        }
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

#[path = "termination_tests.rs"]
mod termination_tests;

#[path = "process_tests.rs"]
mod process_tests;

#[path = "process_detail_tests.rs"]
mod process_detail_tests;
