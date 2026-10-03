use super::*;
#[cfg(test)]
use crate::test_support::TestUnwrapExt;

#[gpui_kit::test]
fn process_table_fills_resized_windows_and_keeps_columns_aligned(cx: &mut TestAppContext) {
    use gpui_kit::{px, size};
    let (view, cx) = populated(cx);
    command(
        &view,
        crate::workspace::Command::Screen(Screen::Processes),
        cx,
    );
    let mut name_widths = Vec::new();
    for width in [1280., 1800., 2560., 960., 1280.] {
        cx.simulate_resize(size(px(width), px(1000.)));
        draw(cx);
        let viewport = cx.debug_bounds("process-table").test_unwrap();
        let last = cx.debug_bounds("process-sort:7").test_unwrap();
        if width >= 1280. {
            assert!(
                (last.right() + px(8.) - (viewport.right() - px(1.))).abs() <= px(1.),
                "table must fill the available width at {width}: {last:?}, {viewport:?}"
            );
        }
        for (column, (heading, cell)) in [
            ("process-sort:0", "process:401:40100:cell:0:text"),
            ("process-sort:1", "process:401:40100:cell:1:text"),
            ("process-sort:2", "process:401:40100:cell:2:text"),
            ("process-sort:3", "process:401:40100:cell:3:text"),
            ("process-sort:4", "process:401:40100:cell:4:text"),
            ("process-sort:5", "process:401:40100:cell:5:text"),
            ("process-sort:6", "process:401:40100:cell:6:text"),
            ("process-sort:7", "process:401:40100:cell:7:text"),
        ]
        .into_iter()
        .enumerate()
        {
            if !crate::processes::VISIBLE_PROCESS_COLUMNS.contains(&column) {
                assert!(cx.debug_bounds(heading).is_none());
                assert!(cx.debug_bounds(cell).is_none());
                continue;
            }
            let heading = cx.debug_bounds(heading).test_unwrap();
            let cell = cx.debug_bounds(cell).test_unwrap();
            assert!(
                (heading.left() - cell.left()).abs() <= px(1.),
                "left edge at {width}, column {column}"
            );
            assert!(
                (heading.right() - cell.right()).abs() <= px(1.),
                "right edge at {width}, column {column}"
            );
        }
        name_widths.push(cx.debug_bounds("process-sort:1").test_unwrap().size.width);
    }
    assert!(name_widths[1] > name_widths[0]);
    assert!(name_widths[2] > name_widths[1]);
    assert_eq!(name_widths[0], name_widths[4]);
}

#[gpui_kit::test]
fn process_search_stays_compact_when_the_window_grows(cx: &mut TestAppContext) {
    use gpui_kit::{px, size};
    let (view, cx) = populated(cx);
    command(
        &view,
        crate::workspace::Command::Screen(Screen::Processes),
        cx,
    );
    for width in [1280., 1800., 2560., 960., 1280.] {
        cx.simulate_resize(size(px(width), px(640.)));
        draw(cx);
        let search = cx.debug_bounds("process-search").test_unwrap();
        assert_eq!(search.size.width, px(280.), "search width at {width}");
        assert!(search.left() >= px(0.) && search.right() <= px(width));
    }
}

#[gpui_kit::test]
fn environmental_screens_hide_unavailable_sensors(cx: &mut TestAppContext) {
    use crate::workspace::Command;
    let (view, cx) = populated(cx);
    let mut snapshot = fixture::snapshot(6);
    let ids: Vec<_> = snapshot
        .sensors
        .iter()
        .filter(|sensor| {
            matches!(
                sensor.kind,
                ramag_infra_system::SensorKind::Power | ramag_infra_system::SensorKind::Temperature
            )
        })
        .map(|sensor| sensor.id.clone())
        .collect();
    for reading in &mut snapshot.readings {
        if ids.contains(&reading.sensor_id) {
            reading.value = None;
            reading.total = None;
            reading.availability = ramag_infra_system::Availability::Unavailable;
            reading.reason = Some("Sensor access has not been enabled".into());
        }
    }
    accept(&view, snapshot, cx);
    for (screen, id, stat, reason) in [
        (
            Screen::Thermals,
            "cpu:host/temperature",
            "screen-stat:cpu:host/temperature",
            "sensor-availability:cpu:host/temperature",
        ),
        (
            Screen::Energy,
            "cpu:host/power",
            "screen-stat:cpu:host/power",
            "sensor-availability:cpu:host/power",
        ),
    ] {
        command(&view, Command::ScreenDevice(screen, id.into()), cx);
        command(&view, Command::Screen(screen), cx);
        assert!(cx.debug_bounds(stat).is_none());
        assert!(cx.debug_bounds(reason).is_none());
        cx.read(|cx| {
            let data = view.read(cx).shared.borrow();
            assert!(crate::screen_data::selected_channel(&data, screen).is_none());
        });
    }
}
