use super::*;
use gpui_kit::{Bounds, Pixels};

/// Confirms each modal control remains inside its parent after responsive layout and scrolling.
fn assert_bounds_contained(child: Bounds<Pixels>, parent: Bounds<Pixels>, description: &str) {
    assert!(child.left() >= parent.left(), "{description} left edge");
    assert!(child.right() <= parent.right(), "{description} right edge");
    assert!(child.top() >= parent.top(), "{description} top edge");
    assert!(
        child.bottom() <= parent.bottom(),
        "{description} bottom edge: child={child:?}, parent={parent:?}"
    );
}

/// Exercises confirmation bounds, body scrolling, and cancellation across compact typography themes.
#[gpui_kit::test]
fn confirmation_actions_fit_a_compact_360_by_240_view(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|view_cx| test_view(window, view_cx, MonitorSnapshot::default()));
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(360.0), px(240.0)));
    let view = required!(view_entity, "system view");
    let long_name = format!(
        "{} {}",
        "long-running-worker-process-".repeat(8),
        "数据库后台任务".repeat(12)
    );

    for (mode_index, mode) in [ramag_ui::Mode::Light, ramag_ui::Mode::Dark]
        .into_iter()
        .enumerate()
    {
        for (size_index, text_size) in [
            ramag_ui::InterfaceTextSize::Standard,
            ramag_ui::InterfaceTextSize::Large,
        ]
        .into_iter()
        .enumerate()
        {
            visual.update(|_, app| ramag_ui::apply_theme(mode, app));
            visual.update(|_, app| {
                ramag_ui::set_system_settings(
                    ramag_ui::SystemSettings {
                        text_size,
                        ..Default::default()
                    },
                    app,
                )
            });
            let prepared = visual.update(|_, app| {
                view.update(app, |this, cx| {
                    this.prepare_termination(
                        crate::StableProcessIdentity {
                            pid: 4242,
                            start_time_ticks: 1,
                        },
                        long_name.clone(),
                        cx,
                    )
                })
            });
            assert!(
                prepared.is_some(),
                "eligible process should open confirmation"
            );
            visual.run_until_parked();

            let card = required!(
                visual.debug_bounds("system-termination-card"),
                "confirmation card bounds"
            );
            let title = required!(
                visual.debug_bounds("system-termination-title"),
                "confirmation title bounds"
            );
            let body = required!(
                visual.debug_bounds("system-termination-body"),
                "confirmation body bounds"
            );
            let footer = required!(
                visual.debug_bounds("system-termination-actions"),
                "confirmation footer and actions bounds"
            );
            let cancel = required!(
                visual.debug_bounds("system-kill-cancel"),
                "cancel button bounds"
            );
            let confirm = required!(
                visual.debug_bounds("system-kill-confirm"),
                "confirm button bounds"
            );
            assert!(card.left() >= px(0.0) && card.right() <= px(360.0));
            assert!(card.top() >= px(0.0) && card.bottom() <= px(240.0));
            assert_bounds_contained(title, card, "confirmation title");
            assert_bounds_contained(body, card, "confirmation body");
            assert_bounds_contained(footer, card, "confirmation footer");
            assert_bounds_contained(cancel, footer, "cancel action");
            assert_bounds_contained(confirm, footer, "confirm action");
            assert!(
                body.size.height <= px(100.0),
                "long body should be height-bounded"
            );
            assert!(
                body.size.height > px(0.0),
                "long body should remain visible"
            );
            assert!(
                cancel.right() <= confirm.left(),
                "actions should not overlap"
            );
            visual.update(|window, app| {
                assert!(view.read(app).termination_focus.is_focused(window));
            });
            visual.simulate_keystrokes("enter");
            visual.run_until_parked();
            visual.update(|_, app| {
                assert!(view.read(app).termination_request.is_some());
                assert!(!view.read(app).termination_in_progress);
            });

            visual.simulate_event(gpui_kit::ScrollWheelEvent {
                position: body.center(),
                delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.0), px(-120.0))),
                modifiers: gpui_kit::Modifiers::default(),
                touch_phase: gpui_kit::TouchPhase::Moved,
            });
            visual.run_until_parked();
            visual.update(|_, app| {
                assert!(
                    view.read(app).termination_scroll.offset().y < px(0.0),
                    "scrolling must expose the rest of the long target description"
                );
            });
            assert_bounds_contained(
                required!(
                    visual.debug_bounds("system-termination-title"),
                    "title remains available after scrolling"
                ),
                card,
                "title after body scroll",
            );
            assert_bounds_contained(
                required!(
                    visual.debug_bounds("system-termination-actions"),
                    "actions remain available after scrolling"
                ),
                card,
                "actions after body scroll",
            );

            if (mode_index + size_index) % 2 == 0 {
                visual.simulate_keystrokes("escape");
            } else {
                visual.simulate_click(cancel.center(), gpui_kit::Modifiers::default());
            }
            visual.run_until_parked();
            visual.update(|_, app| {
                let view = view.read(app);
                assert!(view.termination_request.is_none());
                assert!(!view.termination_in_progress);
            });
        }
    }
}

/// Checks the process action stays reachable across compact/desktop layouts, themes, and type sizes.
#[gpui_kit::test]
fn process_termination_action_fits_theme_size_and_viewport_matrix(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|view_cx| {
            let mut view = test_view(window, view_cx, process_snapshot());
            view.section = SystemSection::Processes;
            view
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = required!(view_entity, "system view");

    for mode in [ramag_ui::Mode::Light, ramag_ui::Mode::Dark] {
        for text_size in [
            ramag_ui::InterfaceTextSize::Standard,
            ramag_ui::InterfaceTextSize::Large,
        ] {
            visual.update(|_, app| ramag_ui::apply_theme(mode, app));
            visual.update(|_, app| {
                ramag_ui::set_system_settings(
                    ramag_ui::SystemSettings {
                        text_size,
                        ..Default::default()
                    },
                    app,
                )
            });
            let mut compact_action_width = None;
            let mut desktop_action_width = None;
            for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
                visual.simulate_resize(size(px(width), px(height)));
                visual.run_until_parked();
                let row = required!(
                    visual.debug_bounds("system-process-row-4242"),
                    "process row bounds"
                );
                let action = required!(
                    visual.debug_bounds("system-kill-4242"),
                    "process termination action bounds"
                );
                assert!(row.left() >= px(0.0) && row.right() <= px(width));
                assert!(row.top() >= px(0.0) && row.bottom() <= px(height));
                assert_bounds_contained(action, row, "process termination action");
                if width == 360.0 {
                    compact_action_width = Some(action.size.width);
                } else if desktop_action_width.is_none() {
                    desktop_action_width = Some(action.size.width);
                }
            }
            let compact_action_width = required!(compact_action_width, "compact action width");
            let desktop_action_width = required!(desktop_action_width, "desktop action width");
            assert!(
                compact_action_width < desktop_action_width,
                "compact icon action should use less width than the desktop text action"
            );
        }
    }

    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    let action = required!(
        visual.debug_bounds("system-kill-4242"),
        "compact process termination action bounds"
    );
    visual.simulate_click(action.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    visual.update(|_, app| {
        assert!(view.read(app).termination_request.is_some());
        assert!(!view.read(app).termination_in_progress);
    });
    let cancel = required!(
        visual.debug_bounds("system-kill-cancel"),
        "compact confirmation cancel action"
    );
    visual.simulate_click(cancel.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    visual.update(|_, app| {
        assert!(view.read(app).termination_request.is_none());
        assert!(!view.read(app).termination_in_progress);
    });
}
