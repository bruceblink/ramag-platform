//! Memory Summary layout checks for long collector explanations.

use super::super::summary_activity::tests::{
    activity_snapshot, monitor, sample, sensor, test_view,
};
use crate::{MonitorSnapshot, ReadingStatus, SensorSample};
use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, TestAppContext, px, size};
use ramag_infra_system::{MonitorKind, SensorKind, Unit};
use std::cell::RefCell;
use std::collections::VecDeque;
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

/// Builds a memory snapshot whose cache reason is longer than its compact row.
fn memory_snapshot() -> MonitorSnapshot {
    let mut snapshot = activity_snapshot();
    snapshot.host.monitors.push(monitor(
        "memory:host",
        "Memory",
        MonitorKind::Memory,
        "memory:host/used",
    ));
    for (suffix, value, reason) in [
        ("used", Some(8_000_000_000.0), None),
        ("total", Some(16_000_000_000.0), None),
        ("available", Some(4_000_000_000.0), None),
        (
            "cache",
            None,
            Some("sysinfo does not expose this host memory field on this platform"),
        ),
        ("swap", Some(0.0), None),
    ] {
        let mut descriptor = sensor(
            "memory:host",
            suffix,
            SensorKind::Capacity,
            Unit::Bytes,
            None,
        );
        descriptor.title = suffix.into();
        snapshot.host.sensors.push(descriptor.clone());
        snapshot.histories.insert(
            descriptor.id,
            VecDeque::from([SensorSample {
                value,
                status: reason.map_or(ReadingStatus::Current, |_| ReadingStatus::Unavailable),
                reason: reason.map(str::to_owned),
                ..sample(11.0, 0.0, None)
            }]),
        );
    }
    snapshot
}

/// Checks detail bounds at desktop and scrollable widths in both themes.
#[gpui_kit::test]
fn memory_detail_layout_keeps_long_unavailable_reason_inside_its_region(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let snapshot_for_view = memory_snapshot();
    let view_slot = Rc::new(RefCell::new(None));
    let slot = view_slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|view_cx| test_view(window, view_cx, snapshot_for_view));
        *slot.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    let _view = required!(view_slot.borrow().as_ref().cloned(), "system view");
    for mode in [ramag_ui::Mode::Light, ramag_ui::Mode::Dark] {
        visual.update(|_, app| {
            ramag_ui::apply_theme(mode, app);
            gpui_kit::component::Theme::global_mut(app).font_size = px(14.0);
        });
        for (width, height) in [(1024.0, 768.0), (1440.0, 900.0), (360.0, 640.0)] {
            visual.simulate_resize(size(px(width), px(height)));
            visual.run_until_parked();
            let panel = required!(visual.debug_bounds("system-summary-memory"), "memory panel");
            let details = [
                "system-summary-memory-available",
                "system-summary-memory-cache",
                "system-summary-memory-swap",
            ]
            .map(|selector| required!(visual.debug_bounds(selector), "memory detail"));
            for detail in details {
                assert!(detail.left() >= panel.left() && detail.right() <= panel.right());
                assert!(detail.top() >= panel.top() && detail.bottom() <= panel.bottom());
                assert!(detail.size.width > px(0.0) && detail.size.height > px(0.0));
            }
            for (first, left) in details.iter().enumerate() {
                for right in details.iter().skip(first + 1) {
                    assert!(
                        left.left() >= right.right()
                            || right.left() >= left.right()
                            || left.top() >= right.bottom()
                            || right.top() >= left.bottom(),
                        "memory details overlap at width {width} in {mode:?}"
                    );
                }
            }
        }
    }
}
