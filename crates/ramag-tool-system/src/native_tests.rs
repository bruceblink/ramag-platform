//! Shared headless input helpers for the imported fixed-screen acceptance tests.
//! Retired fork-specific dock replay tests stay in the upstream source repository.

use crate::{panel::MonitorPanel, screens::ApplicationView, workspace::WorkspaceView};
use gpui_kit::{
    AppContext, Entity, KeyDownEvent, KeyUpEvent, Keystroke, TestAppContext, VisualTestContext,
};

/// Build a deterministic workspace without collectors or user configuration.
pub(crate) fn harness(cx: &mut TestAppContext) -> (Entity<WorkspaceView>, &mut VisualTestContext) {
    cx.update(gpui_kit::component::init);
    let mut workspace = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| WorkspaceView::new_fixture(window, cx));
        workspace = Some(view.clone());
        gpui_kit::component::Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    draw(cx);
    (view, cx)
}

/// Render the same ten-screen application entry point used by the production tool.
pub(crate) fn application_harness(
    cx: &mut TestAppContext,
) -> (Entity<ApplicationView>, &mut VisualTestContext) {
    cx.update(gpui_kit::component::init);
    let mut application = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApplicationView::new_fixture(window, cx));
        application = Some(view.clone());
        gpui_kit::component::Root::new(view, window, cx)
    });
    draw(cx);
    (application.unwrap(), cx)
}

/// Return the settings and presets controller hosted by the active screen.
/// Render the ten-screen entry point and activate its settings page.
pub(crate) fn settings_harness(
    cx: &mut TestAppContext,
) -> (
    Entity<WorkspaceView>,
    Entity<crate::settings::SettingsPanel>,
    &mut VisualTestContext,
) {
    let (application, cx) = application_harness(cx);
    let screen = cx.read(|cx| application.read(cx).screens.clone());
    let (workspace, settings) = cx.read(|cx| {
        let screen = screen.read(cx);
        (
            screen
                .shared
                .borrow()
                .owner
                .clone()
                .unwrap()
                .upgrade()
                .unwrap(),
            screen.settings_panel(),
        )
    });
    cx.update(|window, cx| {
        screen.update(cx, |screen, cx| {
            screen.select(system_pulse_model::Screen::Settings, window, cx)
        })
    });
    draw(cx);
    let dark = cx.debug_bounds("settings-dark").unwrap();
    cx.simulate_click(dark.center(), gpui_kit::Modifiers::none());
    draw(cx);
    (workspace, settings, cx)
}

/// Render the production Processes screen and return its standalone table panel.
pub(crate) fn process_harness(
    cx: &mut TestAppContext,
) -> (
    Entity<WorkspaceView>,
    Entity<MonitorPanel>,
    &mut VisualTestContext,
) {
    let (application, cx) = application_harness(cx);
    let screen = cx.read(|cx| application.read(cx).screens.clone());
    let (workspace, processes) = cx.read(|cx| {
        let screen = screen.read(cx);
        (
            screen
                .shared
                .borrow()
                .owner
                .clone()
                .unwrap()
                .upgrade()
                .unwrap(),
            screen.processes.clone(),
        )
    });
    cx.update(|window, cx| {
        screen.update(cx, |screen, cx| {
            screen.select(system_pulse_model::Screen::Processes, window, cx)
        })
    });
    draw(cx);
    (workspace, processes, cx)
}

/// Complete layout and prepaint notifications before reading interaction bounds.
pub(crate) fn draw(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
            window.draw(cx).clear(cx);
        });
    }
}

/// Deliver both key edges so repeated navigation is tested as actual input.
pub(crate) fn native_key(key: &str, cx: &mut VisualTestContext) {
    let keystroke = Keystroke::parse(key).unwrap();
    cx.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_event(KeyUpEvent { keystroke });
}
