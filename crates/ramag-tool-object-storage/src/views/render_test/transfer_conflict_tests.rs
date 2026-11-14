use std::sync::Arc;

use gpui_kit::{Modifiers, MouseButton, TestAppContext, px, size};
use ramag_domain::entities::OverwritePolicy;
use ramag_ui::Mode;

use support::{
    ControlledDriver, Gate, accounts, click, expect_target, pending_id, select_other_account,
    select_other_mount, setup_transfer, start_download, start_upload,
};

mod support;

#[gpui_kit::test]
fn upload_conflicts_keep_original_target_after_account_or_bucket_switch(cx: &mut TestAppContext) {
    let accounts = accounts();
    for switch_account in [false, true] {
        for switch_bucket in [false, true] {
            let gate = Arc::new(Gate::default());
            let driver = Arc::new(ControlledDriver::new(true, Some(gate.clone())));
            let (view, visual, target) = setup_transfer(cx, &accounts, driver.clone());
            start_upload(&view, visual, target.clone());
            assert_eq!(pending_id(&view, visual, true), None);
            assert_eq!(driver.calls().len(), 1);
            if switch_account {
                select_other_account(&view, visual, &accounts);
            }
            if switch_bucket {
                select_other_mount(
                    &view,
                    visual,
                    if switch_account {
                        &accounts[1]
                    } else {
                        &accounts[0]
                    },
                );
            }
            gate.release();
            visual.run_until_parked();
            assert!(pending_id(&view, visual, true).is_some());
            click(visual, "ramag-confirm-ok");
            visual.run_until_parked();
            let calls = driver.calls();
            assert_eq!(calls.len(), 2);
            expect_target(&calls[0], &target, true, OverwritePolicy::Refuse);
            expect_target(&calls[1], &target, true, OverwritePolicy::Overwrite);
            assert_eq!(calls[0].key, calls[1].key);
            assert_eq!(calls[0].path, calls[1].path);
        }
    }
}

#[gpui_kit::test]
fn download_conflicts_keep_original_target_after_account_or_bucket_switch(cx: &mut TestAppContext) {
    let accounts = accounts();
    for switch_account in [false, true] {
        for switch_bucket in [false, true] {
            let gate = Arc::new(Gate::default());
            let driver = Arc::new(ControlledDriver::new(true, Some(gate.clone())));
            let (view, visual, target) = setup_transfer(cx, &accounts, driver.clone());
            start_download(&view, visual, target.clone());
            assert_eq!(pending_id(&view, visual, false), None);
            assert_eq!(driver.calls().len(), 1);
            if switch_account {
                select_other_account(&view, visual, &accounts);
            }
            if switch_bucket {
                select_other_mount(
                    &view,
                    visual,
                    if switch_account {
                        &accounts[1]
                    } else {
                        &accounts[0]
                    },
                );
            }
            gate.release();
            visual.run_until_parked();
            assert!(pending_id(&view, visual, false).is_some());
            click(visual, "ramag-confirm-ok");
            visual.run_until_parked();
            let calls = driver.calls();
            assert_eq!(calls.len(), 2);
            expect_target(&calls[0], &target, false, OverwritePolicy::Refuse);
            expect_target(&calls[1], &target, false, OverwritePolicy::Overwrite);
            assert_eq!(calls[0].key, calls[1].key);
            assert_eq!(calls[0].path, calls[1].path);
        }
    }
}

#[gpui_kit::test]
fn late_upload_conflict_after_session_close_does_not_restore_confirmation(cx: &mut TestAppContext) {
    let accounts = accounts();
    let gate = Arc::new(Gate::default());
    let driver = Arc::new(ControlledDriver::new(true, Some(gate.clone())));
    let (view, visual, target) = setup_transfer(cx, &accounts, driver.clone());
    start_upload(&view, visual, target.clone());
    assert_eq!(driver.calls().len(), 1);
    visual.update(|window, app| {
        view.update(app, |view, cx| {
            view.close_session(target.account_id.clone(), window, cx);
        });
    });
    visual.run_until_parked();
    gate.release();
    visual.run_until_parked();
    assert_eq!(pending_id(&view, visual, true), None);
    let calls = driver.calls();
    assert_eq!(calls.len(), 1);
    expect_target(&calls[0], &target, true, OverwritePolicy::Refuse);
}

#[gpui_kit::test]
fn late_download_conflict_after_session_close_does_not_restore_confirmation(
    cx: &mut TestAppContext,
) {
    let accounts = accounts();
    let gate = Arc::new(Gate::default());
    let driver = Arc::new(ControlledDriver::new(true, Some(gate.clone())));
    let (view, visual, target) = setup_transfer(cx, &accounts, driver.clone());
    start_download(&view, visual, target.clone());
    assert_eq!(driver.calls().len(), 1);
    visual.update(|window, app| {
        view.update(app, |view, cx| {
            view.close_session(target.account_id.clone(), window, cx);
        });
    });
    visual.run_until_parked();
    gate.release();
    visual.run_until_parked();
    assert_eq!(pending_id(&view, visual, false), None);
    let calls = driver.calls();
    assert_eq!(calls.len(), 1);
    expect_target(&calls[0], &target, false, OverwritePolicy::Refuse);
}

#[gpui_kit::test]
fn cancelling_and_stale_transfer_ids_cannot_confirm_new_conflicts(cx: &mut TestAppContext) {
    let accounts = accounts();
    let driver = Arc::new(ControlledDriver::new(true, None));
    let (view, visual, target) = setup_transfer(cx, &accounts, driver.clone());
    start_upload(&view, visual, target.clone());
    let old_id = pending_id(&view, visual, true).expect("first conflict should be pending");
    click(visual, "ramag-confirm-cancel");
    assert_eq!(pending_id(&view, visual, true), None);
    assert_eq!(driver.calls().len(), 1);

    start_upload(&view, visual, target);
    let new_id = pending_id(&view, visual, true).expect("second conflict should be pending");
    assert_ne!(old_id, new_id);
    visual.update(|window, app| {
        view.update(app, |view, cx| {
            view.confirm_overwrite_upload(old_id, window, cx)
        });
    });
    visual.run_until_parked();
    assert_eq!(pending_id(&view, visual, true), Some(new_id));
    assert_eq!(driver.calls().len(), 2);

    click(visual, "ramag-confirm-ok");
    visual.run_until_parked();
    assert_eq!(driver.calls().len(), 3);
    assert_eq!(driver.calls()[2].overwrite, OverwritePolicy::Overwrite);
}

#[gpui_kit::test]
fn conflict_confirmation_fits_supported_sizes_and_themes(cx: &mut TestAppContext) {
    let accounts = accounts();
    let driver = Arc::new(ControlledDriver::new(true, None));
    let (view, visual, target) = setup_transfer(cx, &accounts, driver.clone());
    start_upload(&view, visual, target);
    for mode in [Mode::Light, Mode::Dark] {
        visual.update(|_, app| ramag_ui::apply_theme(mode, app));
        for (width, height) in [
            (360.0, 240.0),
            (360.0, 640.0),
            (1024.0, 768.0),
            (1440.0, 900.0),
        ] {
            visual.simulate_resize(size(px(width), px(height)));
            visual.run_until_parked();
            for selector in ["ramag-confirm-cancel", "ramag-confirm-ok"] {
                let bounds = visual
                    .debug_bounds(selector)
                    .unwrap_or_else(|| panic!("{selector} should be visible"));
                assert!(
                    bounds.origin.x >= px(0.0)
                        && bounds.origin.y >= px(0.0)
                        && bounds.right() <= px(width)
                        && bounds.bottom() <= px(height),
                    "{selector} must fit {width}x{height}: {bounds:?}"
                );
            }
        }
    }
    assert!(pending_id(&view, visual, true).is_some());
    let bounds = visual
        .debug_bounds("ramag-confirm-ok")
        .expect("confirm action should be visible");
    visual.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::default());
    visual.simulate_mouse_up(bounds.center(), MouseButton::Left, Modifiers::default());
    visual.run_until_parked();
    assert_eq!(driver.calls().len(), 2);
}

#[gpui_kit::test]
fn successful_account_delete_clears_pending_transfer_conflict(cx: &mut TestAppContext) {
    let accounts = accounts();
    let driver = Arc::new(ControlledDriver::new(true, None));
    let (view, visual, target) = setup_transfer(cx, &accounts, driver.clone());
    start_download(&view, visual, target.clone());
    let transfer_id =
        pending_id(&view, visual, false).expect("download conflict should be pending");
    click(visual, "ramag-confirm-cancel");
    view.update(visual, |view, _| {
        view.pending_download = Some(super::super::model::PendingTransferConflict {
            transfer_id,
            target: target.clone(),
            account_name: accounts[0].name.clone(),
            path: "existing-local.txt".into(),
            key: "team-a/test.txt".into(),
            existing_summary: "local file exists".into(),
        });
    });

    visual.update(|window, app| {
        view.update(app, |view, cx| {
            view.request_delete_account(window, cx);
        });
    });
    visual.run_until_parked();
    click(visual, "ramag-confirm-ok");
    visual.run_until_parked();
    assert_eq!(pending_id(&view, visual, false), None);
    assert_eq!(driver.calls().len(), 1);
}
