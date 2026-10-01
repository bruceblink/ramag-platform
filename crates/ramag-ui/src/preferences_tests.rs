use super::{PreferenceSaveStatus, PreferenceWriter, PreferenceWriterGlobal, schedule_write};
use futures::channel::oneshot;
use gpui_kit::TestAppContext;

/// Missing storage must surface failure, and retry must retain the latest unpersisted value.
#[gpui_kit::test]
fn unavailable_storage_never_reports_saved(cx: &mut TestAppContext) {
    cx.update(|app| {
        super::persist_preference_latest("theme_mode", "dark".into(), app);
        assert_eq!(
            super::preference_save_status("theme_mode", app),
            Some(PreferenceSaveStatus::Failed("应用存储不可用".into()))
        );
        super::retry_failed_preference("theme_mode", app);
        assert_eq!(
            super::preference_save_status("theme_mode", app),
            Some(PreferenceSaveStatus::Failed("应用存储不可用".into()))
        );
    });
}

/// A queued superseded draft never reaches storage; only the newest task reports success.
#[gpui_kit::test]
fn superseded_queued_write_is_not_executed(cx: &mut TestAppContext) {
    let writes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    cx.update(|app| {
        let writer = PreferenceWriter::default();
        app.set_global(PreferenceWriterGlobal(writer.clone()));
        for value in ["dark", "light"] {
            let revision = writer.begin("theme_mode", value.into());
            let writes = writes.clone();
            schedule_write(
                "theme_mode",
                writer.clone(),
                revision,
                async move {
                    writes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                },
                app,
            );
        }
    });
    cx.run_until_parked();
    assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(
        cx.update(|app| super::preference_save_status("theme_mode", app)),
        Some(PreferenceSaveStatus::Saved)
    );
}

/// Holds a storage future pending while the app still accepts a separate preference update.
#[gpui_kit::test]
fn delayed_preference_write_keeps_app_updates_responsive(cx: &mut TestAppContext) {
    let (release, wait) = oneshot::channel();
    let release = cx.update(|app| {
        let writer = PreferenceWriter::default();
        let revision = writer.begin("theme_mode", "dark".to_string());
        app.set_global(PreferenceWriterGlobal(writer.clone()));
        schedule_write(
            "theme_mode",
            writer,
            revision,
            async move {
                match wait.await {
                    Ok(result) => result,
                    Err(_) => Err("test did not release delayed write".into()),
                }
            },
            app,
        );
        release
    });

    cx.run_until_parked();
    assert_eq!(
        cx.update(|app| {
            app.try_global::<PreferenceWriterGlobal>()
                .and_then(|global| global.0.status("theme_mode"))
        }),
        Some(PreferenceSaveStatus::Saving)
    );

    cx.update(|app| {
        let global = app.try_global::<PreferenceWriterGlobal>();
        assert!(
            global.is_some(),
            "preference writer should remain installed"
        );
        if let Some(global) = global {
            global
                .0
                .begin("monitor_settings", "five-seconds".to_string());
        }
    });
    assert!(release.send(Ok(())).is_ok());
    cx.run_until_parked();

    assert_eq!(
        cx.update(|app| {
            app.try_global::<PreferenceWriterGlobal>()
                .and_then(|global| global.0.status("theme_mode"))
        }),
        Some(PreferenceSaveStatus::Saved)
    );
}

/// Holds the first same-key write in storage, then verifies its stale failure cannot replace a newer draft.
#[gpui_kit::test]
fn in_flight_stale_write_cannot_replace_latest_status_or_value(cx: &mut TestAppContext) {
    let write_order = std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
    let persisted_value = std::sync::Arc::new(parking_lot::Mutex::new(None));
    let old_entered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let latest_entered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (release_old, wait_old) = oneshot::channel();
    let (release_latest, wait_latest) = oneshot::channel();

    let (old_entered, latest_entered) = cx.update(|app| {
        let writer = PreferenceWriter::default();
        app.set_global(PreferenceWriterGlobal(writer.clone()));

        let old_revision = writer.begin("theme_mode", "dark".to_string());
        let old_order = write_order.clone();
        let old_entered_for_write = old_entered.clone();
        schedule_write(
            "theme_mode",
            writer.clone(),
            old_revision,
            async move {
                old_order.lock().push("old");
                old_entered_for_write.store(true, std::sync::atomic::Ordering::SeqCst);
                match wait_old.await {
                    Ok(()) => Err("stale write failed".to_string()),
                    Err(_) => Err("test did not release stale write".to_string()),
                }
            },
            app,
        );
        (old_entered, latest_entered)
    });

    cx.run_until_parked();
    assert!(old_entered.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(write_order.lock().as_slice(), &["old"]);

    let release_latest = cx.update(|app| {
        let global = app.try_global::<PreferenceWriterGlobal>();
        assert!(
            global.is_some(),
            "preference writer should remain installed"
        );
        let Some(global) = global else {
            return release_latest;
        };
        let writer = global.0.clone();
        let latest_revision = writer.begin("theme_mode", "light".to_string());
        let latest_order = write_order.clone();
        let persisted_value = persisted_value.clone();
        let latest_entered_for_write = latest_entered.clone();
        schedule_write(
            "theme_mode",
            writer,
            latest_revision,
            async move {
                latest_order.lock().push("latest");
                latest_entered_for_write.store(true, std::sync::atomic::Ordering::SeqCst);
                match wait_latest.await {
                    Ok(()) => {
                        *persisted_value.lock() = Some("light");
                        Ok(())
                    }
                    Err(_) => Err("test did not release latest write".to_string()),
                }
            },
            app,
        );
        release_latest
    });

    assert_eq!(
        cx.update(|app| super::preference_save_status("theme_mode", app)),
        Some(PreferenceSaveStatus::Saving)
    );
    assert!(release_old.send(()).is_ok());
    cx.run_until_parked();

    assert!(latest_entered.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(write_order.lock().as_slice(), &["old", "latest"]);
    assert_eq!(*persisted_value.lock(), None);
    assert_eq!(
        cx.update(|app| super::preference_save_status("theme_mode", app)),
        Some(PreferenceSaveStatus::Saving),
        "stale failure must not replace the latest in-flight status"
    );
    assert_eq!(
        cx.update(|app| {
            app.try_global::<PreferenceWriterGlobal>()
                .and_then(|global| global.0.retry_value("theme_mode"))
        }),
        None,
        "stale failure must not expose its value for retry"
    );

    assert!(release_latest.send(()).is_ok());
    cx.run_until_parked();
    assert_eq!(
        cx.update(|app| super::preference_save_status("theme_mode", app)),
        Some(PreferenceSaveStatus::Saved)
    );
    assert_eq!(write_order.lock().as_slice(), &["old", "latest"]);
    assert_eq!(*persisted_value.lock(), Some("light"));
    assert_eq!(
        cx.update(|app| {
            app.try_global::<PreferenceWriterGlobal>()
                .and_then(|global| global.0.retry_value("theme_mode"))
        }),
        None
    );
}

/// Confirms older completions cannot replace the status or retry draft for a newer choice.
#[test]
fn stale_preference_completion_does_not_replace_latest_status_or_value() {
    let writer = PreferenceWriter::default();
    let old_revision = writer.begin("theme_mode", "dark".to_string());
    let latest_revision = writer.begin("theme_mode", "light".to_string());

    assert!(!writer.complete("theme_mode", old_revision, Ok(())));
    assert_eq!(
        writer.status("theme_mode"),
        Some(PreferenceSaveStatus::Saving)
    );
    assert_eq!(writer.retry_value("theme_mode"), None);

    assert!(writer.complete(
        "theme_mode",
        latest_revision,
        Err("disk unavailable".to_string())
    ));
    assert_eq!(
        writer.status("theme_mode"),
        Some(PreferenceSaveStatus::Failed("disk unavailable".to_string()))
    );
    assert_eq!(writer.retry_value("theme_mode"), Some("light".to_string()));
}

/// Confirms only a failed latest draft is offered for retry and success clears that state.
#[test]
fn preference_retry_value_tracks_latest_failed_draft() {
    let writer = PreferenceWriter::default();
    let first_revision = writer.begin("monitor_settings", "one-second".to_string());
    assert!(writer.complete(
        "monitor_settings",
        first_revision,
        Err("temporary failure".to_string())
    ));
    assert_eq!(
        writer.retry_value("monitor_settings"),
        Some("one-second".to_string())
    );

    let retry_revision = writer.begin("monitor_settings", "two-seconds".to_string());
    assert_eq!(writer.retry_value("monitor_settings"), None);
    assert!(writer.complete("monitor_settings", retry_revision, Ok(())));
    assert_eq!(
        writer.status("monitor_settings"),
        Some(PreferenceSaveStatus::Saved)
    );
    assert_eq!(writer.retry_value("monitor_settings"), None);
}
