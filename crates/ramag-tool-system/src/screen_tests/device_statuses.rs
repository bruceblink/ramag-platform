use super::*;

#[gpui_kit::test]
fn missing_device_statistics_render_clear_statuses(cx: &mut TestAppContext) {
    use gpui_kit::{px, size};

    let (view, cx) = harness(cx);
    let mut snapshot = fixture::snapshot(6);
    let unavailable = [
        "volume:root/iops",
        "volume:root/latency",
        "network:eth0/rx-total",
        "network:eth0/tx-total",
        "network:eth0/connections",
    ];
    snapshot
        .sensors
        .retain(|sensor| !unavailable.contains(&sensor.id.as_str()));
    snapshot
        .readings
        .retain(|reading| !unavailable.contains(&reading.sensor_id.as_str()));
    accept(&view, snapshot, cx);

    for (screen, device, stats, empty, absent_metric) in [
        (
            Screen::Disks,
            "volume:root",
            "device-stats:disks",
            "device-stats:disks:empty",
            "screen-stat:volume:root/iops",
        ),
        (
            Screen::Network,
            "network:eth0",
            "device-stats:network",
            "device-stats:network:empty",
            "screen-stat:network:eth0/rx-total",
        ),
    ] {
        command(
            &view,
            crate::workspace::Command::ScreenDevice(screen, device.into()),
            cx,
        );
        command(&view, crate::workspace::Command::Screen(screen), cx);
        for (width, height) in [(360., 640.), (1024., 768.), (1440., 900.)] {
            cx.simulate_resize(size(px(width), px(height)));
            draw(cx);
            let stats_bounds = cx.debug_bounds(stats).test_unwrap();
            let empty_bounds = cx.debug_bounds(empty).test_unwrap();
            assert!(empty_bounds.size.width > px(0.) && empty_bounds.size.height > px(0.));
            assert!(
                empty_bounds.origin.y >= stats_bounds.origin.y
                    && empty_bounds.bottom() <= stats_bounds.bottom(),
                "empty status must remain inside the statistics section: stats={stats_bounds:?}, status={empty_bounds:?}"
            );
            assert!(
                cx.debug_bounds(absent_metric).is_none(),
                "missing metrics must not be rendered as live values"
            );
        }
    }
}
