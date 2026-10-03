#[cfg(test)]
use crate::test_support::TestUnwrapExt;
use crate::{
    native_tests::{draw, native_key},
    screens::{ApplicationView, ScreenView},
};
use gpui_kit::{AppContext, Entity, Modifiers, TestAppContext, VisualTestContext};
use system_pulse_model::Screen;

#[path = "screen_fixture.rs"]
mod fixture;

fn accept(
    view: &Entity<ScreenView>,
    snapshot: ramag_infra_system::Snapshot,
    cx: &mut VisualTestContext,
) {
    let sequence = snapshot.sequence;
    cx.update(|window, cx| {
        let owner = view.read(cx).shared.borrow().owner.clone().test_unwrap();
        owner
            .update(cx, |owner, cx| owner.accept_snapshot(snapshot, window, cx))
            .test_unwrap();
    });
    draw(cx);
    cx.read(|cx| {
        assert_eq!(
            view.read(cx)
                .shared
                .borrow()
                .snapshot
                .as_ref()
                .test_unwrap()
                .sequence,
            sequence
        )
    });
}

fn command(
    view: &Entity<ScreenView>,
    command: crate::workspace::Command,
    cx: &mut VisualTestContext,
) {
    cx.update(|window, cx| {
        let owner = view.read(cx).shared.borrow().owner.clone().test_unwrap();
        owner
            .update(cx, |owner, cx| owner.command(command, window, cx))
            .test_unwrap();
    });
    draw(cx);
}

fn populated(cx: &mut TestAppContext) -> (Entity<ScreenView>, &mut VisualTestContext) {
    let (view, cx) = harness(cx);
    for sequence in 1..=5 {
        accept(&view, fixture::snapshot(sequence), cx);
    }
    (view, cx)
}

fn harness(cx: &mut TestAppContext) -> (Entity<ScreenView>, &mut VisualTestContext) {
    cx.update(gpui_kit::component::init);
    let mut screens = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| ApplicationView::new_fixture(window, cx));
        screens = Some(view.read(cx).screens.clone());
        gpui_kit::component::Root::new(view, window, cx)
    });
    draw(cx);
    (screens.test_unwrap(), cx)
}

fn active(view: &Entity<ScreenView>, cx: &VisualTestContext) -> Screen {
    cx.read(|cx| {
        view.read(cx)
            .shared
            .borrow()
            .session
            .workspace
            .screens
            .active
    })
}

mod navigation;
mod process_table;
mod rendered_screens;
