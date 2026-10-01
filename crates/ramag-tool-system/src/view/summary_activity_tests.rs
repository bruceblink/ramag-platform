use super::*;
use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, Context, TestAppContext, Window, px, size};
use ramag_infra_system::{MonitorDescriptor, SensorKind, Snapshot};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
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

pub(crate) fn sample(at_seconds: f64, value: f64, total: Option<f64>) -> SensorSample {
    SensorSample {
        at_seconds,
        value: Some(value),
        total,
        status: ReadingStatus::Current,
        reason: None,
    }
}

pub(crate) fn monitor(
    id: &str,
    title: &str,
    kind: MonitorKind,
    summary_sensor_id: &str,
) -> MonitorDescriptor {
    MonitorDescriptor {
        id: id.into(),
        title: title.into(),
        kind,
        summary_sensor_id: summary_sensor_id.into(),
    }
}

pub(crate) fn sensor(
    monitor_id: &str,
    suffix: &str,
    kind: SensorKind,
    unit: Unit,
    scale: Option<f64>,
) -> SensorDescriptor {
    SensorDescriptor {
        id: format!("{monitor_id}/{suffix}"),
        monitor_id: monitor_id.into(),
        title: suffix.into(),
        kind,
        unit,
        source: "system-test-source".into(),
        scope: format!("{monitor_id} scope"),
        scale,
    }
}

pub(crate) fn activity_snapshot() -> MonitorSnapshot {
    let mut sensors = Vec::new();
    let mut histories = BTreeMap::new();
    for (id, read, write) in [("volume-a", 4.0, 7.0), ("volume-b", 40.0, 70.0)] {
        sensors.push(sensor(
            id,
            "read",
            SensorKind::Rate,
            Unit::BytesPerSecond,
            None,
        ));
        sensors.push(sensor(
            id,
            "write",
            SensorKind::Rate,
            Unit::BytesPerSecond,
            None,
        ));
        sensors.push(sensor(
            id,
            "capacity",
            SensorKind::Capacity,
            Unit::Bytes,
            Some(1_000_000.0),
        ));
        histories.insert(
            format!("{id}/read"),
            VecDeque::from([
                sample(10.0, read / 2.0, Some(9_000.0)),
                sample(11.0, read, None),
            ]),
        );
        histories.insert(
            format!("{id}/write"),
            VecDeque::from([
                sample(10.0, write / 2.0, Some(8_000.0)),
                sample(11.0, write, None),
            ]),
        );
        histories.insert(
            format!("{id}/capacity"),
            VecDeque::from([sample(11.0, 250_000.0, Some(1_000_000.0))]),
        );
    }
    for (id, rx, tx) in [("network-a", 2.0, 3.0), ("network-b", 90.0, 100.0)] {
        sensors.push(sensor(
            id,
            "rx",
            SensorKind::Rate,
            Unit::BytesPerSecond,
            None,
        ));
        sensors.push(sensor(
            id,
            "tx",
            SensorKind::Rate,
            Unit::BytesPerSecond,
            None,
        ));
        histories.insert(
            format!("{id}/rx"),
            VecDeque::from([
                sample(10.0, rx / 2.0, Some(100_000.0)),
                sample(11.0, rx, None),
            ]),
        );
        histories.insert(
            format!("{id}/tx"),
            VecDeque::from([
                sample(10.0, tx / 2.0, Some(200_000.0)),
                sample(11.0, tx, None),
            ]),
        );
    }
    MonitorSnapshot {
        host: Snapshot {
            monitors: vec![
                monitor(
                    "volume-a",
                    "Disk A",
                    MonitorKind::Volume,
                    "volume-a/capacity",
                ),
                monitor(
                    "volume-b",
                    "Disk B",
                    MonitorKind::Volume,
                    "volume-b/capacity",
                ),
                monitor(
                    "network-a",
                    "Network A",
                    MonitorKind::Network,
                    "network-a/rx",
                ),
                monitor(
                    "network-b",
                    "Network B - long interface name for wrapping validation",
                    MonitorKind::Network,
                    "network-b/rx",
                ),
                monitor("gpu-a", "Graphics", MonitorKind::Gpu, "gpu-a/usage"),
            ],
            sensors,
            preferred_network_monitor_id: Some("network-b".into()),
            ..Default::default()
        },
        histories,
        ..Default::default()
    }
}

pub(crate) fn test_view(
    window: &mut Window,
    cx: &mut Context<SystemView>,
    snapshot: MonitorSnapshot,
) -> SystemView {
    let process_search =
        cx.new(|cx| ramag_ui::bounded_search_input(window, cx).placeholder("名称 / PID / 用户"));
    let mut presentation = ramag_ui::MonitorPresentationSettings::default();
    presentation
        .selected_devices
        .insert("disks".into(), "volume-b".into());
    presentation
        .selected_devices
        .insert("network".into(), "network-b".into());
    SystemView {
        monitor: crate::SystemMonitor::with_snapshot(snapshot),
        section: SystemSection::Summary,
        termination_request: None,
        termination_focus: cx.focus_handle(),
        termination_scroll: gpui_kit::ScrollHandle::new(),
        termination_focus_requested: false,
        termination_in_progress: false,
        notice: None,
        process_search,
        process_table_scroll: gpui_kit::ScrollHandle::new(),
        selected_process: None,
        process_detail_scroll: gpui_kit::ScrollHandle::new(),
        presentation,
        _search_subscription: None,
        _settings_subscription: None,
    }
}

#[test]
fn rate_projection_keeps_selected_source_and_physical_range() {
    let snapshot = activity_snapshot();
    let network_b = required!(
        snapshot
            .host
            .monitors
            .iter()
            .find(|monitor| monitor.id == "network-b"),
        "network-b monitor"
    );
    let rx = required!(
        activity_sensor(&snapshot, Some(network_b), "rx"),
        "network rx sensor"
    );
    let tx = required!(
        activity_sensor(&snapshot, Some(network_b), "tx"),
        "network tx sensor"
    );
    assert_eq!(rx.id, "network-b/rx");
    assert_eq!(tx.id, "network-b/tx");
    assert_eq!(
        rate_source_label(Some(rx)),
        "rx · system-test-source · network-b scope"
    );
    assert_eq!(current_rate(snapshot.latest(&rx.id), false), Some(90.0));
    assert_eq!(current_rate(snapshot.latest(&tx.id), false), Some(100.0));
    assert!((shared_rate_maximum(&snapshot, Some(rx), Some(tx)) - 115.0).abs() < 1e-9);

    let disk_b = required!(
        snapshot
            .host
            .monitors
            .iter()
            .find(|monitor| monitor.id == "volume-b"),
        "volume-b monitor"
    );
    let read = required!(
        activity_sensor(&snapshot, Some(disk_b), "read"),
        "disk read sensor"
    );
    let write = required!(
        activity_sensor(&snapshot, Some(disk_b), "write"),
        "disk write sensor"
    );
    assert_eq!(read.id, "volume-b/read");
    assert_eq!(current_rate(snapshot.latest(&read.id), false), Some(40.0));
    assert_eq!(current_rate(snapshot.latest(&write.id), false), Some(70.0));
    assert_eq!(
        capacity_detail(&snapshot, Some(disk_b)).as_deref(),
        Some("已用 244.1 KiB / 976.6 KiB")
    );
}

#[test]
fn rates_never_turn_stale_missing_nonfinite_or_failed_samples_into_current_values() {
    let warming = SensorSample {
        status: ReadingStatus::WarmingUp,
        reason: Some("等待第二次读取".into()),
        ..sample(2.0, 20.0, None)
    };
    let failed = SensorSample {
        status: ReadingStatus::Failed,
        reason: Some("接口读取失败".into()),
        ..sample(3.0, 20.0, None)
    };
    let stale = SensorSample {
        status: ReadingStatus::Stale,
        ..sample(4.0, 20.0, None)
    };
    let invalid_current = sample(5.0, f64::NAN, None);
    for reading in [
        Some(&warming),
        Some(&failed),
        Some(&stale),
        Some(&invalid_current),
        None,
    ] {
        assert_eq!(current_rate(reading, false), None);
    }
    assert_eq!(current_rate(Some(&sample(6.0, 0.0, None)), true), None);
    assert_eq!(
        rate_status(Some(&invalid_current), false, None),
        ReadingStatus::Failed
    );
    assert_eq!(
        rate_status(Some(&warming), false, None),
        ReadingStatus::WarmingUp
    );
    assert_eq!(
        rate_status(Some(&failed), false, None),
        ReadingStatus::Failed
    );
    assert_eq!(rate_status(Some(&stale), false, None), ReadingStatus::Stale);
    assert_eq!(rate_status(None, false, None), ReadingStatus::Unavailable);
    assert_eq!(
        rate_detail(Some(&failed), false, ReadingStatus::Failed),
        "接口读取失败"
    );
    assert_eq!(
        rate_detail(Some(&warming), false, ReadingStatus::WarmingUp),
        "等待第二次读取"
    );
    assert_eq!(
        rate_detail(Some(&stale), false, ReadingStatus::Stale),
        "已过期"
    );
    assert_eq!(rate_source_label(None), "传感器不可用");
    assert_eq!(current_rate(Some(&sample(7.0, -1.0, None)), false), None);

    let mut stale_snapshot = activity_snapshot();
    stale_snapshot.collection_stale = true;
    let network = required!(
        stale_snapshot
            .host
            .monitors
            .iter()
            .find(|monitor| monitor.id == "network-b"),
        "stale network-b monitor"
    );
    let rx = required!(
        activity_sensor(&stale_snapshot, Some(network), "rx"),
        "stale network rx sensor"
    );
    let points = rate_points(&stale_snapshot, Some(rx), 3.0);
    assert_eq!(points[0].value, Some(45.0));
    assert_eq!(
        points[1].value, None,
        "a stalled latest point has no current marker"
    );

    let mut gap_snapshot = activity_snapshot();
    let gap_series = required!(
        gap_snapshot.histories.get_mut("network-b/rx"),
        "network rx history"
    );
    gap_series.insert(
        1,
        SensorSample {
            status: ReadingStatus::Failed,
            reason: Some("临时读取失败".into()),
            ..sample(10.5, 50.0, None)
        },
    );
    let network = required!(
        gap_snapshot
            .host
            .monitors
            .iter()
            .find(|monitor| monitor.id == "network-b"),
        "gap network-b monitor"
    );
    let rx = required!(
        activity_sensor(&gap_snapshot, Some(network), "rx"),
        "gap network rx sensor"
    );
    let points = rate_points(&gap_snapshot, Some(rx), 3.0);
    assert_eq!(
        points.iter().map(|point| point.value).collect::<Vec<_>>(),
        [Some(45.0), None, Some(90.0)]
    );

    let empty = MonitorSnapshot::default();
    assert_eq!(shared_rate_maximum(&empty, None, None), 1.0);
}

#[gpui_kit::test]
fn activity_cards_follow_device_selection_preserve_gaps_and_navigate(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let snapshot = activity_snapshot();
    let snapshot_for_view = snapshot.clone();
    let view_slot = Rc::new(RefCell::new(None));
    let slot = view_slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|view_cx| test_view(window, view_cx, snapshot_for_view));
        *slot.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = required!(view_slot.borrow().as_ref().cloned(), "system view");
    for mode in [ramag_ui::Mode::Light, ramag_ui::Mode::Dark] {
        for font_size in [16.0, 18.0] {
            visual.update(|_, app| {
                ramag_ui::apply_theme(mode, app);
                gpui_kit::component::Theme::global_mut(app).font_size = px(font_size);
            });
            for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
                visual.simulate_resize(size(px(width), px(height)));
                visual.run_until_parked();
                let activity = required!(
                    visual.debug_bounds("system-summary-activity-network-network-b"),
                    "selected network activity card"
                );
                let body = required!(
                    visual.debug_bounds("system-performance-body"),
                    "performance body"
                );
                assert!(activity.left() >= body.left() && activity.right() <= body.right());
                assert!(activity.size.width > px(0.0) && activity.size.height > px(0.0));
            }
        }
    }
    visual.update(|_, app| {
        ramag_ui::apply_theme(ramag_ui::Mode::Dark, app);
        gpui_kit::component::Theme::global_mut(app).font_size = px(14.0);
    });
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();
    assert!(
        visual
            .debug_bounds("system-summary-activity-disks-volume-b")
            .is_some()
    );
    assert!(
        visual
            .debug_bounds("system-summary-activity-network-network-b")
            .is_some()
    );
    assert!(
        visual
            .debug_bounds("system-summary-activity-disks-volume-a")
            .is_none()
    );
    assert!(
        visual
            .debug_bounds("system-summary-activity-network-network-a")
            .is_none()
    );
    assert!(
        visual
            .debug_bounds("system-summary-activity-disks-capacity")
            .is_some()
    );
    assert!(
        visual
            .debug_bounds("system-summary-activity-network-rx-value")
            .is_some()
    );
    assert!(
        visual
            .debug_bounds("system-summary-activity-network-tx-value")
            .is_some()
    );
    assert!(visual.debug_bounds("system-summary-open-network").is_some());
    assert!(visual.debug_bounds("system-subsystem-gpu").is_some());
    assert!(visual.debug_bounds("system-subsystem-disks").is_none());
    assert!(visual.debug_bounds("system-subsystem-network").is_none());

    let open = required!(
        visual.debug_bounds("system-summary-open-network"),
        "open network action"
    );
    visual.simulate_click(open.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    visual.update(|_, app| assert_eq!(view.read(app).section, SystemSection::Network));
    visual.update(|_, app| {
        view.update(app, |state, cx| {
            state.select_section(SystemSection::Summary, cx)
        });
    });
    visual.run_until_parked();
    let gpu = required!(
        visual.debug_bounds("system-subsystem-gpu"),
        "GPU navigation action"
    );
    visual.simulate_click(gpu.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    visual.update(|_, app| assert_eq!(view.read(app).section, SystemSection::Gpu));
    visual.update(|_, app| {
        view.update(app, |state, cx| {
            state.select_section(SystemSection::Summary, cx)
        });
    });
    visual.run_until_parked();

    visual.update(|_, app| {
        let state = view.read(app);
        let (monitor, _) = selected_monitor(&snapshot, state, ActivityKind::NETWORK);
        assert_eq!(
            monitor.map(|monitor| monitor.id.as_str()),
            Some("network-b")
        );
        let rx = required!(
            activity_sensor(&snapshot, monitor, "rx"),
            "selected network rx sensor"
        );
        assert_eq!(current_rate(snapshot.latest(&rx.id), false), Some(90.0));
        let (disk, _) = selected_monitor(&snapshot, state, ActivityKind::DISK);
        let read = required!(
            activity_sensor(&snapshot, disk, "read"),
            "selected disk read sensor"
        );
        assert_eq!(current_rate(snapshot.latest(&read.id), false), Some(40.0));
    });

    visual.update(|_, app| {
        view.update(app, |state, cx| {
            state
                .presentation
                .selected_devices
                .insert("network".into(), "network-a".into());
            cx.notify();
        });
    });
    visual.run_until_parked();
    assert!(
        visual
            .debug_bounds("system-summary-activity-network-network-a")
            .is_some()
    );
    assert!(
        visual
            .debug_bounds("system-summary-activity-network-network-b")
            .is_none()
    );

    visual.update(|_, app| {
        view.update(app, |state, cx| {
            state.presentation.selected_devices.remove("network");
            cx.notify();
        });
    });
    visual.run_until_parked();
    assert!(
        visual
            .debug_bounds("system-summary-activity-network-network-b")
            .is_some()
    );

    visual.update(|_, app| {
        view.update(app, |state, cx| {
            state
                .presentation
                .selected_devices
                .insert("network".into(), "network-offline".into());
            cx.notify();
        });
    });
    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    let panel = required!(
        visual.debug_bounds("system-summary-activity-network-missing"),
        "missing network card"
    );
    let body = required!(
        visual.debug_bounds("system-performance-body"),
        "performance body"
    );
    assert!(panel.left() >= body.left());
    assert!(panel.right() <= body.right());
    assert!(
        visual
            .debug_bounds("system-summary-activity-network-device")
            .is_some()
    );
    assert!(
        visual
            .debug_bounds("system-summary-activity-network-rx-status")
            .is_some()
    );
    let chart = required!(
        visual.debug_bounds("system-summary-activity-network-chart"),
        "network chart"
    );
    assert!(chart.left() >= panel.left());
    assert!(chart.right() <= panel.right());
}
