use super::*;
use gpui_kit::{px, size};
use ramag_infra_system::Availability;

#[gpui_kit::test]
fn summary_long_memory_failure_reason_does_not_expand_the_metric_row(cx: &mut TestAppContext) {
    let (view, cx) = populated(cx);
    let reason = "cached memory could not be read by the platform memory provider ".repeat(8);
    let mut snapshot = fixture::snapshot(6);
    let cached = snapshot
        .readings
        .iter_mut()
        .find(|reading| reading.sensor_id == "memory:host/cache")
        .test_unwrap();
    cached.value = None;
    cached.availability = Availability::Unavailable;
    cached.reason = Some(reason);
    accept(&view, snapshot, cx);

    for (width, height) in [(360., 640.), (1024., 768.), (1440., 900.)] {
        cx.simulate_resize(size(px(width), px(height)));
        draw(cx);
        let metric = cx.debug_bounds("summary-memory:cache").test_unwrap();
        assert!(
            metric.left() >= px(0.) && metric.right() <= px(width),
            "long memory failure reason expanded the inline metric at {width}x{height}: {metric:?}"
        );
    }

    command(
        &view,
        crate::workspace::Command::SensorVisible("memory:host".into(), "memory:host/cache".into()),
        cx,
    );
    assert!(cx.debug_bounds("summary-memory:cache").is_none());
}
