use super::MonitorPresetManager;
use gpui_kit::component::Root;
use gpui_kit::{AppContext, Modifiers, TestAppContext, VisualTestContext, px, size};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx.debug_bounds(selector);
    assert!(bounds.is_some(), "missing preset control: {selector}");
    let Some(bounds) = bounds else { return };
    cx.simulate_mouse_move(bounds.center(), None, Modifiers::default());
    cx.simulate_mouse_down(
        bounds.center(),
        gpui_kit::MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        bounds.center(),
        gpui_kit::MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
}

/// Exercises named and built-in preset actions through the rendered manager.
#[gpui_kit::test]
fn manager_covers_create_apply_overwrite_rename_and_delete_confirmations(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    cx.update(|app| {
        crate::set_monitor_settings(crate::MonitorSettings::default(), app);
        crate::set_monitor_presentation_settings(
            crate::MonitorPresentationSettings {
                selected_devices: BTreeMap::from([("gpu".into(), "gpu-1".into())]),
                selected_sensors: BTreeMap::from([("energy".into(), "gpu.power".into())]),
                hidden_sensors: BTreeSet::from(["cpu.temp".into()]),
            },
            app,
        );
    });
    let entity = Rc::new(RefCell::new(None));
    let entity_for_window = entity.clone();
    let (_, visual) = cx.add_window_view(|window, cx| {
        let manager = cx.new(|cx| MonitorPresetManager::new(window, cx));
        *entity_for_window.borrow_mut() = Some(manager.clone());
        Root::new(manager, window, cx)
    });
    visual.simulate_resize(size(px(800.0), px(900.0)));
    visual.run_until_parked();
    let manager = entity.borrow().clone();
    assert!(manager.is_some(), "preset manager missing");
    let Some(manager) = manager else { return };

    visual.update(|window, app| {
        manager.update(app, |manager, cx| {
            manager
                .input
                .update(cx, |state, cx| state.set_value("Work", window, cx));
        });
    });
    click(visual, "monitor-preset-save");
    assert!(visual.debug_bounds("monitor-preset-apply-0").is_some());
    assert_eq!(
        visual.read(|app| crate::monitor_preset_library(app).presets["Work"]
            .monitor_settings
            .refresh_rate),
        crate::MonitorRefreshRate::OneSecond
    );

    click(visual, "monitor-preset-apply-0");
    visual.update(|_, app| {
        assert_eq!(
            crate::monitor_settings(app).refresh_rate,
            crate::MonitorRefreshRate::OneSecond
        );
        crate::set_monitor_settings(
            crate::MonitorSettings {
                refresh_rate: crate::MonitorRefreshRate::FiveSeconds,
            },
            app,
        );
        crate::set_monitor_presentation_settings(
            crate::MonitorPresentationSettings::default(),
            app,
        );
    });
    click(visual, "monitor-preset-apply-0");
    visual.update(|_, app| {
        assert_eq!(
            crate::monitor_settings(app).refresh_rate,
            crate::MonitorRefreshRate::OneSecond,
            "applying a saved preset must restore its sampling cadence"
        );
        assert_eq!(
            crate::monitor_presentation_settings(app)
                .selected_devices
                .get("gpu")
                .map(String::as_str),
            Some("gpu-1")
        );
        assert_eq!(
            crate::monitor_presentation_settings(app)
                .selected_sensors
                .get("energy")
                .map(String::as_str),
            Some("gpu.power")
        );
        assert!(
            crate::monitor_presentation_settings(app)
                .hidden_sensors
                .contains("cpu.temp")
        );
    });

    visual.update(|_, app| {
        crate::set_monitor_settings(
            crate::MonitorSettings {
                refresh_rate: crate::MonitorRefreshRate::TwoSeconds,
            },
            app,
        );
    });
    click(visual, "monitor-preset-overwrite-0");
    assert!(visual.debug_bounds("monitor-preset-confirmation").is_some());
    click(visual, "monitor-preset-cancel");
    click(visual, "monitor-preset-overwrite-0");
    click(visual, "monitor-preset-confirm-overwrite");
    assert_eq!(
        visual.read(|app| crate::monitor_preset_library(app).presets["Work"]
            .monitor_settings
            .refresh_rate),
        crate::MonitorRefreshRate::TwoSeconds
    );
    visual.update(|_, app| {
        crate::set_monitor_settings(
            crate::MonitorSettings {
                refresh_rate: crate::MonitorRefreshRate::FiveSeconds,
            },
            app,
        );
    });
    click(visual, "monitor-preset-apply-0");
    assert_eq!(
        visual.read(|app| crate::monitor_settings(app).refresh_rate),
        crate::MonitorRefreshRate::TwoSeconds
    );

    visual.update(|window, app| {
        manager.update(app, |manager, cx| {
            manager
                .input
                .update(cx, |state, cx| state.set_value("Coding", window, cx));
        });
    });
    click(visual, "monitor-preset-rename-0");
    assert!(visual.read(|app| {
        crate::monitor_preset_library(app)
            .presets
            .contains_key("Coding")
    }));
    click(visual, "monitor-preset-delete-0");
    click(visual, "monitor-preset-cancel");
    assert!(visual.read(|app| {
        crate::monitor_preset_library(app)
            .presets
            .contains_key("Coding")
    }));
    click(visual, "monitor-preset-delete-0");
    click(visual, "monitor-preset-confirm-delete");
    assert!(visual.read(|app| crate::monitor_preset_library(app).presets.is_empty()));

    click(visual, "monitor-preset-builtin-Developer");
    assert_eq!(
        visual.read(|app| crate::monitor_settings(app).refresh_rate),
        crate::MonitorRefreshRate::HalfSecond
    );
    assert!(visual.read(|app| {
        crate::monitor_presentation_settings(app)
            .hidden_sensors
            .contains("cpu.temp")
    }));
    click(visual, "monitor-preset-builtin-Default");
    visual.update(|_, app| {
        assert_eq!(
            crate::monitor_settings(app).refresh_rate,
            crate::MonitorRefreshRate::OneSecond
        );
        assert_eq!(
            crate::monitor_presentation_settings(app),
            crate::MonitorPresentationSettings::default()
        );
    });
}
