use super::{
    InputEvent, MonitorPanel, ProcessIdentity, ProcessSignal, UNVERIFIED_PROCESS_NOTICE,
    process_action_block_reason,
};
use crate::native_tests::{draw, native_key, process_harness};
#[cfg(test)]
use crate::test_support::TestUnwrapExt;
use gpui_kit::{AppContext, Entity, Modifiers, TestAppContext, VisualTestContext, point, px};

fn focus_table(processes: &Entity<MonitorPanel>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        processes.read(cx).controls["table"]
            .handle
            .clone()
            .focus(window, cx)
    });
    draw(cx);
}

#[test]
fn unverified_source_identity_is_read_only() {
    let unknown = ProcessIdentity {
        pid: 4242,
        start_time_ticks: 0,
    };
    let verified = ProcessIdentity {
        pid: 4242,
        start_time_ticks: 7,
    };
    assert_eq!(
        process_action_block_reason(&unknown),
        Some(UNVERIFIED_PROCESS_NOTICE)
    );
    assert_eq!(process_action_block_reason(&verified), None);
}

#[gpui_kit::test]
fn unverified_process_remains_selectable_but_cannot_open_action_confirmation(
    cx: &mut TestAppContext,
) {
    let (_view, processes, cx) = process_harness(cx);
    let unknown = cx.update(|_, cx| {
        processes.update(cx, |this, cx| {
            let mut data = this.shared.borrow_mut();
            let row = &mut data.processes[0];
            row.identity.pid = 0;
            row.identity.start_time_ticks = 0;
            this.process_state.sort.column = 0;
            this.process_state.sort.descending = false;
            let identity = row.identity.clone();
            drop(data);
            cx.notify();
            identity
        })
    });
    draw(cx);
    let visible_index = cx.read(|cx| {
        processes
            .read(cx)
            .process_projection()
            .iter()
            .position(|&index| {
                processes.read(cx).shared.borrow().processes[index].identity == unknown
            })
            .test_unwrap()
    });
    assert_eq!(visible_index, 0);
    let row = cx
        .debug_bounds("process-row:0")
        .test_unwrap_msg("unverified process row remains rendered");
    cx.simulate_click(row.center(), Modifiers::none());
    draw(cx);
    assert_eq!(
        cx.read(|cx| processes.read(cx).selected.clone()),
        Some(unknown.clone())
    );

    cx.update(|_, cx| {
        processes.update(cx, |this, cx| {
            this.prepare_process_action(unknown.clone(), ProcessSignal::Kill, cx);
            assert!(this.process_state.confirmation.is_none());
            assert_eq!(
                this.shared.borrow().process_action.notice,
                UNVERIFIED_PROCESS_NOTICE
            );
            this.process_state.confirmation = Some((
                unknown.clone(),
                "unknown source process".into(),
                ProcessSignal::Terminate,
            ));
            this.confirm_process_action(cx);
            assert!(this.process_state.confirmation.is_none());
            assert_eq!(
                this.shared.borrow().process_action.notice,
                UNVERIFIED_PROCESS_NOTICE
            );
            assert!(!this.shared.borrow().process_action.busy);
        })
    });
    draw(cx);
}

#[gpui_kit::test]
fn search_and_sort_controls_preserve_identity_and_navigate_visible_rows(cx: &mut TestAppContext) {
    let (_view, processes, cx) = process_harness(cx);
    focus_table(&processes, cx);
    cx.update(|_, cx| {
        processes.update(cx, |this, cx| {
            let mut data = this.shared.borrow_mut();
            data.processes.truncate(3);
            for (index, row) in data.processes.iter_mut().enumerate() {
                row.cells[1] = ["Firefox", "terminal", "editor"][index].into();
                row.numeric[0] = Some([9., 80., 20.][index]);
            }
            cx.notify();
        })
    });
    draw(cx);
    assert_eq!(
        cx.read(|cx| processes.read(cx).process_projection()),
        vec![1, 2, 0]
    );
    let row = cx.debug_bounds("process-row:0").test_unwrap();
    cx.simulate_mouse_move(row.center(), None, Modifiers::none());
    draw(cx);
    cx.simulate_click(row.origin + point(px(15.), px(10.)), Modifiers::none());
    draw(cx);
    let selected = cx
        .read(|cx| processes.read(cx).selected.clone())
        .test_unwrap_msg("pointer selects a row");
    assert_eq!(cx.read(|cx| processes.read(cx).selected_index()), Some(1));
    let heading = cx.debug_bounds("process-sort:2").test_unwrap();
    cx.simulate_click(heading.center(), Modifiers::none());
    draw(cx);
    assert_eq!(
        cx.read(|cx| processes.read(cx).process_projection()),
        vec![0, 2, 1]
    );
    assert_eq!(
        cx.read(|cx| processes.read(cx).selected.clone()),
        Some(selected.clone())
    );
    let input = cx.read(|cx| processes.read(cx).process_state.input.clone().test_unwrap());
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.set_value("FIREFOX", window, cx);
            cx.emit(InputEvent::Change);
        })
    });
    draw(cx);
    assert_eq!(
        cx.read(|cx| processes.read(cx).process_projection()),
        vec![0]
    );
    assert_eq!(
        cx.read(|cx| processes.read(cx).selected.clone()),
        Some(selected)
    );
    focus_table(&processes, cx);
    native_key("down", cx);
    draw(cx);
    assert_eq!(cx.read(|cx| processes.read(cx).selected_index()), Some(0));
    assert!(cx.debug_bounds("process-row:1").is_none());
}

#[gpui_kit::test]
fn confirmation_cancel_keeps_identity_and_fixture_execution_reports_error(cx: &mut TestAppContext) {
    let (_view, processes, cx) = process_harness(cx);
    focus_table(&processes, cx);
    let identity = cx.read(|cx| {
        processes.read(cx).shared.borrow().processes[0]
            .identity
            .clone()
    });
    cx.update(|_, cx| {
        processes.update(cx, |this, cx| {
            this.prepare_process_action(identity.clone(), ProcessSignal::Kill, cx)
        })
    });
    draw(cx);
    // A later selection must not retarget a pending confirmation.
    cx.update(|_, cx| {
        processes.update(cx, |this, _| {
            this.selected = Some(this.shared.borrow().processes[1].identity.clone());
        })
    });
    assert_eq!(
        cx.read(|cx| processes
            .read(cx)
            .process_state
            .confirmation
            .as_ref()
            .test_unwrap()
            .0
            .clone()),
        identity
    );
    let cancel = cx.debug_bounds("process-action:cancel").test_unwrap();
    cx.simulate_mouse_move(cancel.center(), None, Modifiers::none());
    draw(cx);
    cx.simulate_click(cancel.center(), Modifiers::none());
    draw(cx);
    cx.read(|cx| {
        let panel = processes.read(cx);
        assert!(panel.process_state.confirmation.is_none());
        let data = panel.shared.borrow();
        assert!(!data.process_action.busy);
        assert!(data.process_action.notice.is_empty());
    });
    cx.update(|_, cx| {
        processes.update(cx, |this, cx| {
            this.prepare_process_action(identity.clone(), ProcessSignal::Terminate, cx)
        })
    });
    draw(cx);
    let confirm = cx.debug_bounds("process-action:confirm").test_unwrap();
    cx.simulate_click(confirm.center(), Modifiers::none());
    draw(cx);
    cx.read(|cx| {
        let panel = processes.read(cx);
        assert!(panel.process_state.confirmation.is_none());
        let data = panel.shared.borrow();
        assert!(!data.process_action.busy);
        assert_eq!(
            data.process_action.notice,
            "Process actions require a live system snapshot."
        );
    });
    cx.update(|_, cx| {
        processes.update(cx, |this, cx| {
            this.shared
                .borrow_mut()
                .processes
                .retain(|row| row.identity != identity);
            this.prepare_process_action(identity.clone(), ProcessSignal::Kill, cx);
        })
    });
    assert!(cx.read(|cx| {
        processes
            .read(cx)
            .shared
            .borrow()
            .process_action
            .notice
            .contains("has exited")
    }));
}
#[gpui_kit::test]
fn pending_action_rejects_duplicate_confirmation_and_selection_changes(cx: &mut TestAppContext) {
    let (_view, processes, cx) = process_harness(cx);
    focus_table(&processes, cx);
    cx.update(|_, cx| {
        processes.update(cx, |this, cx| {
            let original = this.shared.borrow().processes[0].identity.clone();
            let other = this.shared.borrow().processes[1].identity.clone();
            this.prepare_process_action(original.clone(), ProcessSignal::Kill, cx);
            this.shared.borrow_mut().process_action.busy = true;
            this.shared.borrow_mut().process_action.notice =
                "Awaiting Windows authorization".into();
            this.selected = Some(other.clone());
            this.prepare_process_action(other, ProcessSignal::Terminate, cx);
            this.confirm_process_action(cx);
            this.confirm_process_action(cx);
            assert!(this.shared.borrow().process_action.busy);
            assert_eq!(
                this.shared.borrow().process_action.notice,
                "Awaiting Windows authorization"
            );
            let confirmation = this.process_state.confirmation.as_ref().test_unwrap();
            assert_eq!(confirmation.0, original);
            assert_eq!(confirmation.2, ProcessSignal::Kill);
        });
    });
    draw(cx);
    // The table still handles navigation while authorization is pending.
    native_key("down", cx);
    draw(cx);
    assert!(cx.read(|cx| processes.read(cx).selected.is_some()));
}

#[test]
fn completion_releases_busy_state_without_claiming_unobserved_exit() {
    use super::ProcessActionState;
    use ramag_infra_system::process_control::ProcessActionOutcome;
    for (outcome, expected) in [
        (
            ProcessActionOutcome::SignalSent,
            "Waiting for the process list",
        ),
        (
            ProcessActionOutcome::CloseRequested,
            "exit has not been confirmed",
        ),
        (
            ProcessActionOutcome::TerminationPending,
            "exit is still pending",
        ),
        (ProcessActionOutcome::ExitObserved, "has exited"),
    ] {
        let mut state = ProcessActionState {
            busy: true,
            ..Default::default()
        };
        state.finish_action("owned target", 4242, Ok(outcome));
        assert!(!state.busy);
        assert!(state.notice.contains("owned target (PID 4242)"));
        assert!(state.notice.contains(expected));
        assert_eq!(
            state.notice.contains("has exited"),
            outcome == ProcessActionOutcome::ExitObserved
        );
    }
    let mut state = ProcessActionState {
        busy: true,
        ..Default::default()
    };
    state.finish_action(
        "owned target",
        4242,
        Err("Unknown outcome; check the process list".into()),
    );
    assert!(!state.busy);
    assert_eq!(state.notice, "Unknown outcome; check the process list");
}

#[gpui_kit::test]
fn recreated_process_panel_cannot_resubmit_an_inflight_action(cx: &mut TestAppContext) {
    let (_view, processes, cx) = process_harness(cx);
    cx.update(|_, cx| {
        let (monitor, shared) = processes.update(cx, |this, _| {
            this.shared.borrow_mut().process_action.busy = true;
            this.shared.borrow_mut().process_action.notice = "Awaiting authorization".into();
            (this.monitor.clone(), this.shared.clone())
        });
        let replacement = cx.new(|cx| MonitorPanel::new_standalone(monitor, shared, cx));
        replacement.update(cx, |this, cx| {
            let identity = this.shared.borrow().processes[0].identity.clone();
            this.prepare_process_action(identity, ProcessSignal::Kill, cx);
            assert!(this.process_state.confirmation.is_none());
            assert!(this.shared.borrow().process_action.busy);
            assert_eq!(
                this.shared.borrow().process_action.notice,
                "Awaiting authorization"
            );
        });
        processes.update(cx, |this, _| {
            this.shared.borrow_mut().process_action.finish_action(
                "original target",
                4242,
                Err("Authorization cancelled".into()),
            );
        });
        replacement.update(cx, |this, _| {
            assert!(!this.shared.borrow().process_action.busy);
            assert_eq!(
                this.shared.borrow().process_action.notice,
                "Authorization cancelled"
            );
            assert!(this.process_state.confirmation.is_none());
        });
    });
}
