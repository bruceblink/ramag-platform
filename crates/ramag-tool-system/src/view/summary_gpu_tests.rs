use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, TestAppContext, px};

use super::super::summary_activity::tests::{activity_snapshot, sample, test_view};
use super::{current_usage, gpu_sensor, usage_points, usage_status};
use crate::{ReadingStatus, SensorSample};

fn require<T>(value: Option<T>, message: &str) -> T {
    assert!(value.is_some(), "{message}");
    match value {
        Some(value) => value,
        None => unreachable!("assertion above guarantees a value"),
    }
}

#[test]
fn gpu_summary_keeps_zero_and_marks_invalid_or_stale_usage() {
    let snapshot = activity_snapshot();
    let gpu = require(
        snapshot
            .host
            .monitors
            .iter()
            .find(|monitor| monitor.id == "gpu-a"),
        "GPU monitor",
    );
    let usage = require(gpu_sensor(&snapshot, Some(gpu)), "GPU usage sensor");
    assert_eq!(current_usage(snapshot.latest(&usage.id), false), Some(0.0));
    assert_eq!(
        usage_status(snapshot.latest(&usage.id), false, Some(0.0)),
        ReadingStatus::Current
    );

    let invalid = SensorSample {
        value: Some(101.0),
        ..sample(12.0, 0.0, None)
    };
    assert_eq!(current_usage(Some(&invalid), false), None);
    assert_eq!(
        usage_status(Some(&invalid), false, None),
        ReadingStatus::Failed
    );

    let mut stale = snapshot.clone();
    stale.collection_stale = true;
    assert_eq!(current_usage(stale.latest(&usage.id), true), None);
    assert_eq!(
        usage_status(stale.latest(&usage.id), true, None),
        ReadingStatus::Stale
    );
    let points = usage_points(&snapshot, Some(usage), 3.0);
    assert_eq!(
        points.iter().map(|point| point.value).collect::<Vec<_>>(),
        [Some(0.0), Some(0.0)]
    );
}

#[gpui_kit::test]
fn gpu_summary_selection_and_missing_identity_are_rendered(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let snapshot = activity_snapshot();
    let snapshot_for_view = snapshot.clone();
    let view_slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let slot = view_slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|view_cx| test_view(window, view_cx, snapshot_for_view));
        *slot.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = require(view_slot.borrow().as_ref().cloned(), "system view");

    visual.simulate_resize(gpui_kit::size(px(1024.0), px(768.0)));
    visual.run_until_parked();
    assert!(visual.debug_bounds("system-summary-gpu-gpu-a").is_some());

    visual.update(|_, app| {
        view.update(app, |state, cx| {
            state
                .presentation
                .selected_devices
                .insert("gpu".into(), "gpu-b".into());
            cx.notify();
        });
    });
    visual.run_until_parked();
    assert!(visual.debug_bounds("system-summary-gpu-gpu-b").is_some());
    assert!(visual.debug_bounds("system-summary-gpu-gpu-a").is_none());
    assert!(visual.debug_bounds("system-summary-gpu-value").is_some());

    visual.update(|_, app| {
        view.update(app, |state, cx| {
            state
                .presentation
                .selected_devices
                .insert("gpu".into(), "gpu-missing".into());
            cx.notify();
        });
    });
    visual.run_until_parked();
    assert!(visual.debug_bounds("system-summary-gpu-missing").is_some());
    assert!(visual.debug_bounds("system-summary-gpu-status").is_some());
}
