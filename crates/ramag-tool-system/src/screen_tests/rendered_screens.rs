use super::*;
#[cfg(test)]
use crate::test_support::TestUnwrapExt;

#[gpui_kit::test]
fn all_nine_navigable_screens_render_populated_collector_data(cx: &mut TestAppContext) {
    let (view, cx) = populated(cx);
    for (screen, selector) in [
        (Screen::Summary, "history:cpu:host/usage"),
        (Screen::Cpu, "history:cpu:host/core-0-usage"),
        (Screen::Memory, "history:memory:host/used"),
        (Screen::Gpu, "history:gpu:pci:0000:01:00.0/usage"),
        (Screen::Disks, "device-history:disks"),
        (Screen::Network, "device-history:network"),
        (Screen::Energy, "history:cpu:host/power"),
        (Screen::Thermals, "history:cpu:host/temperature"),
        (Screen::Processes, "process-table"),
    ] {
        command(&view, crate::workspace::Command::Screen(screen), cx);
        let bounds = cx.debug_bounds(selector).unwrap_or_else(|| {
            std::panic::resume_unwind(Box::new(format!(
                "missing populated {screen:?} content: {selector}"
            )))
        });
        assert!(bounds.size.width > gpui_kit::px(0.) && bounds.size.height > gpui_kit::px(0.));
        assert_eq!(active(&view, cx), screen);
    }
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert_eq!(data.processes.len(), 2);
        assert_eq!(data.processes[0].identity.pid, 401);
        assert_eq!(crate::screen_data::cpu_cores(&data).len(), 4);
        for monitor in &data.snapshot.as_ref().test_unwrap().monitors {
            assert!(
                data.history
                    .latest(&monitor.id, &monitor.summary_sensor_id)
                    .test_unwrap()
                    .chart_value()
                    .is_some(),
                "missing current summary for {}",
                monitor.id
            );
        }
    });
}

#[gpui_kit::test]
fn summary_metric_labels_follow_latest_cpu_and_memory_samples(cx: &mut TestAppContext) {
    let (view, cx) = populated(cx);
    command(
        &view,
        crate::workspace::Command::Screen(Screen::Summary),
        cx,
    );

    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        let cpu = crate::screen_data::find(&data, "cpu:host", "usage").test_unwrap();
        let memory = crate::screen_data::find(&data, "memory:host", "used").test_unwrap();
        assert_eq!(cpu.value(&data), "33.0 %");
        assert_eq!(memory.value(&data), "12.0 / 32.0 GiB");
        assert_ne!(cpu.value(&data), "0 %");
        assert_ne!(memory.value(&data), "0 B");
    });
}

#[gpui_kit::test]
fn energy_primary_and_sensor_grid_render_zero_values_and_capture_gaps(cx: &mut TestAppContext) {
    let (view, cx) = populated(cx);
    command(
        &view,
        crate::workspace::Command::ScreenDevice(Screen::Energy, "cpu:host/power".into()),
        cx,
    );
    command(&view, crate::workspace::Command::Screen(Screen::Energy), cx);
    let primary = cx.debug_bounds("selected-channel-history").test_unwrap();
    let grid = cx.debug_bounds("history:cpu:host/power").test_unwrap();
    assert_eq!(primary.size.height, gpui_kit::px(260.));
    assert_eq!(grid.size.height, gpui_kit::px(95.));
    assert!(grid.origin.y > primary.origin.y);
    assert!(
        cx.debug_bounds("history:gpu:pci:0000:02:00.0/power")
            .is_some()
    );
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        let selected = crate::screen_data::selected_channel(&data, Screen::Energy).test_unwrap();
        let rows = crate::screen_data::by_quantity(
            &data,
            system_pulse_model::Quantity::Power,
            system_pulse_model::PhysicalUnit::Watts,
        );
        assert!(
            rows.iter().any(|row| row.sensor == selected.sensor),
            "the primary channel must also exercise the grid path"
        );
        let values: Vec<_> = selected
            .samples(&data)
            .iter()
            .map(|sample| sample.chart_value())
            .collect();
        assert_eq!(
            values,
            vec![Some(43.), Some(44.), None, Some(42.), Some(43.)]
        );
        let zero = rows
            .iter()
            .find(|row| row.monitor == fixture::GPU_B)
            .test_unwrap();
        assert_eq!(zero.measured(&data), Some(0.));
        assert!(zero.value(&data).starts_with('0'));
        assert!(selected.value(&data).contains("43"));
    });
}

#[gpui_kit::test]
fn summary_keeps_energy_card_when_all_power_sensors_are_unavailable(cx: &mut TestAppContext) {
    use crate::workspace::Command;

    let (view, cx) = populated(cx);
    for (monitor, sensor) in [
        ("cpu:host".to_owned(), "cpu:host/power".to_owned()),
        (
            fixture::GPU_A.to_owned(),
            format!("{}/power", fixture::GPU_A),
        ),
        (
            fixture::GPU_B.to_owned(),
            format!("{}/power", fixture::GPU_B),
        ),
    ] {
        command(&view, Command::SensorVisible(monitor, sensor), cx);
    }
    command(&view, Command::Screen(Screen::Summary), cx);

    let card = cx
        .debug_bounds("summary-history:energy")
        .unwrap_or_else(|| {
            std::panic::resume_unwind(Box::new("missing Summary Energy history card"))
        });
    assert!(card.size.width > gpui_kit::px(0.) && card.size.height > gpui_kit::px(0.));
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert!(crate::screen_data::selected_channel(&data, Screen::Energy).is_none());
    });
}

#[gpui_kit::test]
fn summary_keeps_thermals_card_when_all_temperature_sensors_are_unavailable(
    cx: &mut TestAppContext,
) {
    use crate::workspace::Command;

    let (view, cx) = populated(cx);
    for (monitor, sensor) in [
        ("cpu:host".to_owned(), "cpu:host/temperature".to_owned()),
        (
            fixture::GPU_A.to_owned(),
            format!("{}/temperature", fixture::GPU_A),
        ),
        (
            fixture::GPU_B.to_owned(),
            format!("{}/temperature", fixture::GPU_B),
        ),
    ] {
        command(&view, Command::SensorVisible(monitor, sensor), cx);
    }
    command(&view, Command::Screen(Screen::Summary), cx);

    let card = cx
        .debug_bounds("summary-history:thermals")
        .unwrap_or_else(|| {
            std::panic::resume_unwind(Box::new("missing Summary Thermals history card"))
        });
    assert!(card.size.width > gpui_kit::px(0.) && card.size.height > gpui_kit::px(0.));
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert!(
            crate::screen_data::highest_current(
                &data,
                &crate::screen_data::by_quantity(
                    &data,
                    system_pulse_model::Quantity::Temperature,
                    system_pulse_model::PhysicalUnit::Celsius,
                ),
            )
            .is_none()
        );
    });
}

#[gpui_kit::test]
fn selected_gpu_survives_reordering_restore_and_disconnect_without_switching(
    cx: &mut TestAppContext,
) {
    use crate::workspace::Command;
    let (view, cx) = populated(cx);
    command(
        &view,
        Command::ScreenDevice(Screen::Gpu, fixture::GPU_B.into()),
        cx,
    );
    command(&view, Command::Screen(Screen::Gpu), cx);
    assert!(
        cx.debug_bounds("history:gpu:pci:0000:02:00.0/usage")
            .is_some()
    );
    let mut reordered = fixture::snapshot(6);
    reordered.monitors.reverse();
    reordered.sensors.reverse();
    accept(&view, reordered, cx);
    command(&view, Command::Screen(Screen::Memory), cx);
    command(&view, Command::Screen(Screen::Gpu), cx);
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        let choices = crate::screen_data::devices(&data, Screen::Gpu);
        assert_eq!(choices.len(), 2);
        assert_ne!(choices[0].id, choices[1].id);
        assert_ne!(choices[0].label, choices[1].label);
        for choice in &choices {
            assert!(choice.label.contains(&choice.id));
        }
        assert_eq!(
            crate::screen_data::selected_device(&data, Screen::Gpu).as_deref(),
            Some(fixture::GPU_B)
        );
        let json = data.session.autosave_json().test_unwrap();
        let restored: system_pulse_model::Workspace = serde_json::from_str(&json).test_unwrap();
        assert_eq!(restored.screens.devices[&Screen::Gpu], fixture::GPU_B);
    });
    let mut disconnected = fixture::snapshot(7);
    disconnected
        .monitors
        .retain(|monitor| monitor.id != fixture::GPU_B);
    disconnected
        .sensors
        .retain(|sensor| sensor.monitor_id != fixture::GPU_B);
    disconnected
        .readings
        .retain(|reading| !reading.sensor_id.starts_with(fixture::GPU_B));
    accept(&view, disconnected, cx);
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert_eq!(
            crate::screen_data::selected_device(&data, Screen::Gpu).as_deref(),
            Some(fixture::GPU_B)
        );
        assert!(
            crate::screen_data::devices(&data, Screen::Gpu)
                .iter()
                .all(|device| device.id != fixture::GPU_B)
        );
        assert!(
            data.history
                .latest(fixture::GPU_B, &format!("{}/usage", fixture::GPU_B))
                .is_none()
        );
        assert!(
            data.history
                .latest(fixture::GPU_A, &format!("{}/usage", fixture::GPU_A))
                .test_unwrap()
                .chart_value()
                .is_some()
        );
    });
    assert!(
        cx.debug_bounds("history:gpu:pci:0000:01:00.0/usage")
            .is_none(),
        "disconnect must not display the other GPU's history"
    );
    accept(&view, fixture::snapshot(8), cx);
    assert!(
        cx.debug_bounds("history:gpu:pci:0000:02:00.0/usage")
            .is_some()
    );
}

#[gpui_kit::test]
fn detected_gpu_hides_unavailable_sensors_without_hiding_the_device(cx: &mut TestAppContext) {
    use crate::workspace::Command;
    let (view, cx) = populated(cx);
    let mut snapshot = fixture::snapshot(6);
    for reading in &mut snapshot.readings {
        if reading.sensor_id.starts_with(fixture::GPU_A) {
            reading.value = None;
            reading.total = None;
            reading.availability = ramag_infra_system::Availability::Unavailable;
            reading.reason = Some("Optional vendor API unavailable".into());
        }
    }
    accept(&view, snapshot, cx);
    command(
        &view,
        Command::ScreenDevice(Screen::Gpu, fixture::GPU_A.into()),
        cx,
    );
    command(&view, Command::Screen(Screen::Gpu), cx);
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert_eq!(crate::screen_data::devices(&data, Screen::Gpu).len(), 2);
        let rows = crate::screen_data::gpu_channels(&data, fixture::GPU_A);
        assert!(rows.is_empty());
    });
    assert!(
        cx.debug_bounds("history:gpu:pci:0000:01:00.0/usage")
            .is_none()
    );
    assert!(
        cx.debug_bounds("screen-stat:gpu:pci:0000:01:00.0/power")
            .is_none()
    );
    command(
        &view,
        Command::SensorVisible(fixture::GPU_A.into(), format!("{}/power", fixture::GPU_A)),
        cx,
    );
    assert!(
        cx.debug_bounds("screen-stat:gpu:pci:0000:01:00.0/power")
            .is_none()
    );
}

#[gpui_kit::test]
fn hidden_sensors_stay_hidden_after_new_snapshots_and_tab_switches(cx: &mut TestAppContext) {
    use crate::workspace::Command;
    let (view, cx) = populated(cx);
    command(
        &view,
        Command::SensorVisible("cpu:host".into(), "cpu:host/core-0-usage".into()),
        cx,
    );
    command(
        &view,
        Command::SensorVisible("cpu:host".into(), "cpu:host/power".into()),
        cx,
    );
    accept(&view, fixture::snapshot(6), cx);
    command(&view, Command::Screen(Screen::Cpu), cx);
    assert!(cx.debug_bounds("history:cpu:host/core-0-usage").is_none());
    assert!(cx.debug_bounds("history:cpu:host/core-1-usage").is_some());
    command(&view, Command::Screen(Screen::Energy), cx);
    assert!(cx.debug_bounds("history:cpu:host/power").is_none());
    assert!(
        cx.debug_bounds("history:gpu:pci:0000:01:00.0/power")
            .is_some()
    );
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        assert!(
            data.history
                .latest("cpu:host", "cpu:host/power")
                .test_unwrap()
                .chart_value()
                .is_some(),
            "hiding presentation must preserve collection"
        );
        assert!(
            crate::screen_data::devices(&data, Screen::Energy)
                .iter()
                .all(|device| device.id != "cpu:host/power")
        );
        assert_eq!(crate::screen_data::cpu_cores(&data).len(), 3);
    });
}

#[gpui_kit::test]
fn hiding_used_memory_preserves_other_memory_readings(cx: &mut TestAppContext) {
    use crate::workspace::Command;
    let (view, cx) = populated(cx);
    command(&view, Command::Screen(Screen::Memory), cx);
    assert!(cx.debug_bounds("history:memory:host/used").is_some());
    command(
        &view,
        Command::SensorVisible("memory:host".into(), "memory:host/used".into()),
        cx,
    );
    accept(&view, fixture::snapshot(6), cx);
    assert!(cx.debug_bounds("history:memory:host/used").is_none());
    assert!(cx.debug_bounds("level:memory:host/used").is_none());
    assert!(cx.debug_bounds("screen-stat:memory:host/used").is_none());
    for selector in [
        "screen-stat:memory:host/available",
        "screen-stat:memory:host/cache",
        "screen-stat:memory:host/swap",
    ] {
        let bounds = cx.debug_bounds(selector).unwrap_or_else(|| {
            std::panic::resume_unwind(Box::new(format!("hiding Used also hid {selector}")))
        });
        assert!(bounds.size.width > gpui_kit::px(0.) && bounds.size.height > gpui_kit::px(0.));
    }
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        let available = crate::screen_data::find(&data, "memory:host", "available").test_unwrap();
        assert_eq!(available.measured(&data), Some(20. * 1024_f64.powi(3)));
        let swap = crate::screen_data::find(&data, "memory:host", "swap").test_unwrap();
        assert_eq!(swap.measured(&data), Some(0.));
        assert!(
            data.history
                .latest("memory:host", "memory:host/used")
                .test_unwrap()
                .chart_value()
                .is_some()
        );
    });
}

#[gpui_kit::test]
fn unavailable_selected_thermal_sensor_keeps_other_charts_and_hottest_reading(
    cx: &mut TestAppContext,
) {
    use crate::workspace::Command;
    let (view, cx) = populated(cx);
    let selected_id = format!("{}/temperature", fixture::GPU_B);
    command(
        &view,
        Command::ScreenDevice(Screen::Thermals, selected_id.clone()),
        cx,
    );
    command(&view, Command::Screen(Screen::Thermals), cx);
    assert!(cx.debug_bounds("selected-channel-history").is_some());
    assert!(
        cx.debug_bounds("history:gpu:pci:0000:02:00.0/temperature")
            .is_some()
    );
    let hottest_panel = cx.debug_bounds("thermal-hottest-panel").test_unwrap();
    let selected_panel = cx.debug_bounds("selected-channel-panel").test_unwrap();
    let selected_chart = cx.debug_bounds("selected-channel-history").test_unwrap();
    assert!(hottest_panel.bottom() <= selected_panel.origin.y);
    assert!(selected_panel.bottom() <= selected_chart.origin.y);
    cx.read(|cx| {
        let data = view.read(cx).shared.borrow();
        let selected = crate::screen_data::selected_channel(&data, Screen::Thermals).test_unwrap();
        assert_eq!(selected.measured(&data), Some(46.));
        let temperatures = crate::screen_data::by_quantity(
            &data,
            system_pulse_model::Quantity::Temperature,
            system_pulse_model::PhysicalUnit::Celsius,
        );
        let hottest = crate::screen_data::highest_current(&data, &temperatures).test_unwrap();
        assert_ne!(selected.sensor, hottest.sensor);
    });

    let assert_other_temperatures = |expected_hottest: f64, cx: &mut VisualTestContext| {
        assert!(cx.debug_bounds("selected-channel-history").is_none());
        assert!(
            cx.debug_bounds("history:gpu:pci:0000:02:00.0/temperature")
                .is_none()
        );
        assert!(cx.debug_bounds("history:cpu:host/temperature").is_some());
        assert!(
            cx.debug_bounds("history:gpu:pci:0000:01:00.0/temperature")
                .is_some()
        );
        let indicator = cx.debug_bounds("thermal-hottest").test_unwrap();
        assert!(indicator.size.width > gpui_kit::px(0.));
        assert!(indicator.size.height > gpui_kit::px(0.));
        cx.read(|cx| {
            let data = view.read(cx).shared.borrow();
            assert_eq!(
                crate::screen_data::selected_device(&data, Screen::Thermals).as_deref(),
                Some(selected_id.as_str())
            );
            assert!(crate::screen_data::selected_channel(&data, Screen::Thermals).is_none());
            let temperatures = crate::screen_data::by_quantity(
                &data,
                system_pulse_model::Quantity::Temperature,
                system_pulse_model::PhysicalUnit::Celsius,
            );
            let hottest = crate::screen_data::highest_current(&data, &temperatures).test_unwrap();
            assert_eq!(hottest.sensor, "cpu:host/temperature");
            assert_eq!(hottest.measured(&data), Some(expected_hottest));
        });
    };

    command(
        &view,
        Command::SensorVisible(fixture::GPU_B.into(), selected_id.clone()),
        cx,
    );
    accept(&view, fixture::snapshot(6), cx);
    assert_other_temperatures(63., cx);

    command(
        &view,
        Command::SensorVisible(fixture::GPU_B.into(), selected_id.clone()),
        cx,
    );
    assert!(cx.debug_bounds("selected-channel-history").is_some());
    let mut disconnected = fixture::snapshot(7);
    disconnected
        .sensors
        .retain(|sensor| sensor.id != selected_id);
    disconnected
        .readings
        .retain(|reading| reading.sensor_id != selected_id);
    accept(&view, disconnected, cx);
    assert_other_temperatures(64., cx);
}
