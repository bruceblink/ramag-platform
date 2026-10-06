use super::*;
use crate::test_support::TestUnwrapExt;
use ramag_infra_system::Availability;

#[gpui_kit::test]
fn selected_energy_and_thermal_failures_keep_identity_and_reason(cx: &mut TestAppContext) {
    let (view, cx) = populated(cx);
    for (screen, suffix, reason, sequence) in [
        (
            Screen::Energy,
            "power",
            "Firmware disabled the selected power sensor",
            6,
        ),
        (
            Screen::Thermals,
            "temperature",
            "Firmware sensor access was denied",
            7,
        ),
    ] {
        let id = format!("{}/{}", fixture::GPU_B, suffix);
        command(
            &view,
            crate::workspace::Command::ScreenDevice(screen, id.clone()),
            cx,
        );
        let mut snapshot = fixture::snapshot(sequence);
        let reading = snapshot
            .readings
            .iter_mut()
            .find(|reading| reading.sensor_id == id)
            .test_unwrap();
        reading.value = None;
        reading.availability = Availability::Unavailable;
        reading.reason = Some(reason.into());
        accept(&view, snapshot, cx);
        command(&view, crate::workspace::Command::Screen(screen), cx);

        assert!(cx.debug_bounds("selected-channel-unavailable").is_some());
        assert!(cx.debug_bounds("selected-channel-history").is_some());
        cx.read(|cx| {
            let data = view.read(cx).shared.borrow();
            assert_eq!(
                crate::screen_data::selected_device(&data, screen).as_deref(),
                Some(id.as_str())
            );
            assert!(crate::screen_data::selected_channel(&data, screen).is_none());
            let selected =
                crate::screen_data::selected_environmental_channel(&data, screen).test_unwrap();
            let detail = crate::screen_data::unavailable_detail(&selected, &data).test_unwrap();
            assert!(detail.contains(reason), "{detail}");
            assert!(detail.contains(&selected.label), "{detail}");
            let picker = crate::screens::device_picker_state(&data, screen);
            assert_eq!(picker.selected.as_deref(), Some(id.as_str()));
            assert!(
                picker.label.starts_with("Unavailable ·"),
                "{}",
                picker.label
            );
            assert!(!picker.choices.iter().any(|choice| choice.id == id));
            assert!(
                !picker.choices.is_empty(),
                "alternatives should remain selectable"
            );
            assert!(picker.accessibility_label.contains(reason));
        });

        command(
            &view,
            crate::workspace::Command::Screen(Screen::Summary),
            cx,
        );
        let history_selector = if screen == Screen::Energy {
            "summary-history:energy"
        } else {
            "summary-history:thermals"
        };
        assert!(cx.debug_bounds(history_selector).is_some());
        cx.read(|cx| {
            let data = view.read(cx).shared.borrow();
            let selected =
                crate::screen_data::selected_environmental_channel(&data, screen).test_unwrap();
            assert!(
                crate::screen_data::unavailable_detail(&selected, &data)
                    .test_unwrap()
                    .contains(reason)
            );
        });
    }
}
