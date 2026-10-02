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

#[gpui_kit::test]
fn sampling_readout_tracks_refresh_controls_at_supported_sizes(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let entity = Rc::new(RefCell::new(None));
    let entity_for_view = entity.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|view_cx| {
            let mut view = test_view(window, view_cx, MonitorSnapshot::default());
            view.section = SystemSection::Settings;
            view
        });
        *entity_for_view.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = required!(entity.borrow().clone(), "system view missing");
    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        visual.simulate_resize(size(px(width), px(height)));
        visual.run_until_parked();
        for selector in [
            "system-page-settings",
            "system-refresh-current-value",
            "system-refresh-description",
            "system-refresh-save-note",
        ] {
            assert!(
                visual.debug_bounds(selector).is_some(),
                "missing {selector} at {width}x{height}"
            );
        }
        for (selector, interval) in [
            ("system-refresh-0.5s", crate::RefreshInterval::HalfSecond),
            ("system-refresh-1s", crate::RefreshInterval::OneSecond),
            ("system-refresh-2s", crate::RefreshInterval::TwoSeconds),
            ("system-refresh-5s", crate::RefreshInterval::FiveSeconds),
        ] {
            let control = required!(visual.debug_bounds(selector), "refresh interval control");
            visual.simulate_click(control.center(), gpui_kit::Modifiers::default());
            visual.run_until_parked();
            visual.update(|_, app| assert_eq!(view.read(app).monitor.refresh_interval(), interval));
        }
    }
}
