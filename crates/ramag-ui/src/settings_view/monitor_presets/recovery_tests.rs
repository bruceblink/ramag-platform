use super::MonitorPresetManager;
use gpui_kit::component::Root;
use gpui_kit::{AppContext, Modifiers, TestAppContext, VisualTestContext, px, size};
use std::cell::RefCell;
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

/// Corrupt persisted data stays untouched while named edits are unavailable.
#[gpui_kit::test]
fn invalid_library_locks_named_edits_but_keeps_builtin_presets_available(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut load_result = None;
    cx.update(|app| {
        load_result = Some(crate::init_monitor_preset_library(Some("{"), app));
        crate::set_monitor_settings(crate::MonitorSettings::default(), app);
        crate::set_monitor_presentation_settings(
            crate::MonitorPresentationSettings::default(),
            app,
        );
    });
    assert!(load_result.is_some_and(|result| result.is_err()));

    let entity = Rc::new(RefCell::new(None));
    let entity_for_window = entity.clone();
    let (_, visual) = cx.add_window_view(|window, cx| {
        let manager = cx.new(|cx| MonitorPresetManager::new(window, cx));
        *entity_for_window.borrow_mut() = Some(manager.clone());
        Root::new(manager, window, cx)
    });
    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        visual.simulate_resize(size(px(width), px(height)));
        visual.run_until_parked();
        assert!(visual.debug_bounds("monitor-preset-load-error").is_some());
        assert!(visual.debug_bounds("monitor-preset-save").is_none());
    }
    for mode in [crate::Mode::Dark, crate::Mode::Light] {
        visual.update(|_, app| crate::apply_theme(mode, app));
        visual.run_until_parked();
        assert!(visual.debug_bounds("monitor-preset-load-error").is_some());
    }

    assert!(visual.debug_bounds("monitor-preset-load-error").is_some());
    assert!(visual.debug_bounds("monitor-preset-save").is_none());
    assert!(
        visual
            .debug_bounds("monitor-preset-builtin-Developer")
            .is_some()
    );
    click(visual, "monitor-preset-builtin-Developer");
    assert_eq!(
        visual.read(|app| crate::monitor_settings(app).refresh_rate),
        crate::MonitorRefreshRate::HalfSecond
    );
    let mut attempted_save = None;
    visual.update(|_, app| {
        let mut library = crate::MonitorPresetLibrary::default();
        let _ = library.upsert("Work", crate::MonitorPreset::default());
        attempted_save = Some(crate::save_monitor_preset_library(library, app));
        assert!(crate::monitor_preset_library_load_error(app).is_some());
        assert!(crate::monitor_preset_library(app).presets.is_empty());
    });
    assert!(attempted_save.is_some_and(|result| result.is_err()));

    visual.update(|_, app| {
        assert!(
            crate::init_monitor_preset_library(Some(r#"{"version":1,"presets":{}}"#), app).is_ok()
        );
    });
    visual.run_until_parked();
    assert!(visual.debug_bounds("monitor-preset-load-error").is_none());
    assert!(visual.debug_bounds("monitor-preset-save").is_some());
}
