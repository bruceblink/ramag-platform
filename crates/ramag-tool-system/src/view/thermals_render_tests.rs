use super::*;

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

fn thermal_snapshot() -> MonitorSnapshot {
    let sensor = |id: &str, title: &str, scope: &str| SensorDescriptor {
        id: id.into(),
        monitor_id: "thermals".into(),
        title: title.into(),
        kind: SensorKind::Temperature,
        unit: Unit::Celsius,
        source: "windows-thermal".into(),
        scope: scope.into(),
        scale: Some(100.0),
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
            "gpu:0/temperature",
            "GPU temperature",
            "NVIDIA GeForce RTX 3060",
        ),
        sensor(
            "cpu:host/package-temperature",
            "CPU package temperature",
            "CPU package",
        ),
    ];
    MonitorSnapshot {
        host: ramag_infra_system::Snapshot {
            sensors,
            ..Default::default()
        },
        histories: BTreeMap::from([
            ("gpu:0/temperature".into(), VecDeque::from([sample(64.0)])),
            (
                "cpu:host/package-temperature".into(),
                VecDeque::from([sample(58.0)]),
            ),
        ]),
        ..Default::default()
    }
}

#[test]
fn thermal_selection_prefers_hottest_and_keeps_missing_identity() {
    let snapshot = thermal_snapshot();
    let readings = snapshot.host.sensors.iter().collect::<Vec<_>>();
    assert_eq!(
        crate::view::pages::hottest_current_temperature(&readings, &snapshot)
            .map(|(sensor, value)| (sensor.id.as_str(), value)),
        Some(("gpu:0/temperature", 64.0))
    );
    assert_eq!(
        crate::view::pages::selected_thermal_sensor(&readings, None, &snapshot)
            .map(|sensor| sensor.id.as_str()),
        Some("gpu:0/temperature")
    );
    assert_eq!(
        crate::view::pages::selected_thermal_sensor(
            &readings,
            Some("cpu:host/package-temperature"),
            &snapshot,
        )
        .map(|sensor| sensor.id.as_str()),
        Some("cpu:host/package-temperature")
    );
    assert!(
        crate::view::pages::selected_thermal_sensor(
            &readings,
            Some("temperature:removed"),
            &snapshot,
        )
        .is_none(),
        "a missing saved temperature identity must remain unavailable"
    );
    let mut stale = snapshot.clone();
    stale.collection_stale = true;
    assert!(crate::view::pages::hottest_current_temperature(&readings, &stale).is_none());
}

#[gpui_kit::test]
fn thermals_page_renders_hottest_primary_and_sensor_selection(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let entity = Rc::new(RefCell::new(None));
    let entity_for_view = entity.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|view_cx| {
            let mut view = test_view(window, view_cx, thermal_snapshot());
            view.section = SystemSection::Thermals;
            view
        });
        *entity_for_view.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();
    for selector in [
        "system-page-thermals",
        "system-thermals-sensor-selector",
        "system-thermals-hottest",
        "system-thermals-primary-meter",
        "system-thermals-primary-value",
        "system-thermals-primary-chart",
        "system-thermals-domain",
        "system-thermals-temperature-sensors",
        "system-sensor-gpu-0-temperature",
        "system-sensor-cpu-host-package-temperature",
    ] {
        assert!(
            visual.debug_bounds(selector).is_some(),
            "missing {selector}"
        );
    }
    let selector = required!(
        visual.debug_bounds("system-thermals-sensor-selector"),
        "thermal sensor selector"
    );
    visual.simulate_click(selector.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    let option = required!(
        visual.debug_bounds("system-thermals-sensor-option-cpu-host-package-temperature"),
        "CPU temperature option"
    );
    visual.simulate_click(option.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    let view = required!(entity.borrow().clone(), "system view missing");
    visual.update(|_, app| {
        assert_eq!(
            view.read(app)
                .presentation
                .selected_sensors
                .get("thermals")
                .map(String::as_str),
            Some("cpu:host/package-temperature")
        );
        assert_eq!(
            ramag_ui::monitor_presentation_settings(app)
                .selected_sensors
                .get("thermals")
                .map(String::as_str),
            Some("cpu:host/package-temperature")
        );
    });
    for width in [360.0, 1024.0, 1440.0] {
        visual.simulate_resize(size(px(width), px(768.0)));
        visual.run_until_parked();
        let page = required!(visual.debug_bounds("system-page-thermals"), "thermals page");
        let primary = required!(
            visual.debug_bounds("system-thermals-primary-meter"),
            "thermal primary meter"
        );
        let sensors = required!(
            visual.debug_bounds("system-thermals-temperature-sensors"),
            "thermal sensors"
        );
        assert!(
            primary.left() >= page.left() && primary.right() <= page.right(),
            "primary meter overflows {width}px Thermals page"
        );
        assert!(
            sensors.left() >= page.left() && sensors.right() <= page.right(),
            "temperature sensors overflow {width}px Thermals page"
        );
    }
}

#[gpui_kit::test]
fn thermals_missing_saved_sensor_keeps_hottest_summary(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|view_cx| {
            let mut view = test_view(window, view_cx, thermal_snapshot());
            view.section = SystemSection::Thermals;
            view.presentation
                .selected_sensors
                .insert("thermals".into(), "temperature:removed".into());
            view
        });
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();
    assert!(
        visual
            .debug_bounds("system-thermals-primary-unavailable")
            .is_some()
    );
    assert!(visual.debug_bounds("system-thermals-hottest").is_some());
}
