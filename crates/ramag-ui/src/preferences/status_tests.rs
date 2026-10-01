//! Headless production-view coverage for failed preference saves and retries.

use std::{collections::HashMap, sync::Arc, time::Duration};

use gpui_kit::component::Root;
use gpui_kit::{
    AppContext as _, Bounds, Modifiers, Pixels, TestAppContext, VisualTestContext, px, size,
};
use ramag_app::{ConnectionService, SshService, StaticPluginHost, ToolRegistry};
use ramag_domain::traits::Storage;
use ramag_infra_ssh::OpenSshDriver;
use ramag_infra_storage::RedbStorage;

use super::test_storage::SettingsTestStorage;
use crate::SettingsView;

/// Locate the rendered control area while keeping missing selectors explicit.
fn bounds(visual: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    let result = visual.debug_bounds(selector);
    assert!(result.is_some(), "missing settings element: {selector}");
    result.unwrap_or_default()
}

/// Click the center of a production-rendered control and drain its UI callback.
fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let target = bounds(visual, selector);
    visual.simulate_click(target.center(), Modifiers::default());
    visual.run_until_parked();
}

/// Drain the GPUI worker before reading a preference from the real test database.
fn stored(visual: &mut VisualTestContext, storage: &dyn Storage, key: &str) -> Option<String> {
    visual.run_until_parked();
    let result = futures::executor::block_on(storage.get_preference(key));
    assert!(result.is_ok(), "preference read failed: {key}");
    result.ok().flatten()
}

/// Switch through the existing settings-view route without depending on nav focus internals.
fn open_page(view: &gpui_kit::Entity<SettingsView>, visual: &mut VisualTestContext, tool_id: &str) {
    visual.update(|window, app| {
        view.update(app, |view, cx| view.open_tool_page(tool_id, window, cx));
    });
    visual.run_until_parked();
}

/// A real SettingsView must expose durable failure, retain the draft, and retry it successfully.
#[gpui_kit::test]
fn production_settings_show_bounded_failed_save_and_retry_status(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    cx.update(|app| {
        app.set_reduce_motion(true);
        crate::apply_theme(crate::Mode::Dark, app);
    });

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ramag-preference-status-test-{}-{stamp}.redb",
        std::process::id()
    ));
    let result = RedbStorage::open_with_key(&path, &[0x4b; 32]);
    assert!(result.is_ok(), "isolated settings store should open");
    let Ok(store) = result else { return };
    let test_storage = Arc::new(SettingsTestStorage::new(store));
    let storage: Arc<dyn Storage> = test_storage.clone();
    let connection = Arc::new(ConnectionService::new(HashMap::new(), storage.clone()));
    let ssh = Arc::new(SshService::new(
        Arc::new(OpenSshDriver::new()),
        storage.clone(),
    ));
    let host = Arc::new(StaticPluginHost::new(Arc::new(ToolRegistry::new())));
    cx.update(|app| {
        app.set_global(crate::activity_bar::UpdateIndicatorGlobal::default());
        app.set_global(crate::StorageGlobal(storage.clone()));
        crate::set_system_settings(crate::SystemSettings::default(), app);
        crate::set_monitor_settings(crate::MonitorSettings::default(), app);
    });

    let ssh_for_view = ssh.clone();
    let connection_for_view = connection.clone();
    let mut view_entity = None;
    let (root, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            SettingsView::new(
                host,
                None,
                connection_for_view,
                ssh_for_view,
                None,
                window,
                cx,
            )
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let Some(view) = view_entity else { return };

    let cases = [
        (
            "system",
            "settings-text-size-2",
            crate::SYSTEM_SETTINGS_PREF_KEY,
            "settings-save-status-system_settings",
            "settings-save-retry-system_settings",
            "settings-page-system",
            r#"{"minimize_to_tray":false,"text_size":"large","scrollbar_visibility":"always","interface_font":"inter","numeric_font":"jetbrains_mono"}"#,
        ),
        (
            "system",
            "settings-theme-0",
            "theme_mode",
            "settings-save-status-theme_mode",
            "settings-save-retry-theme_mode",
            "settings-page-system",
            "light",
        ),
        (
            "system",
            "settings-monitor-rate-2",
            crate::MONITOR_SETTINGS_PREF_KEY,
            "settings-save-status-monitor_settings",
            "settings-save-retry-monitor_settings",
            "settings-page-monitor",
            r#"{"refresh_rate":"five_seconds"}"#,
        ),
    ];

    for (tool, control, key, status, retry, nav_page, expected) in cases {
        open_page(&view, visual, tool);
        click(visual, nav_page);
        let before = stored(visual, storage.as_ref(), key);
        test_storage.fail_next_preference_write();
        click(visual, control);

        visual.update(|_, app| {
            assert!(matches!(
                crate::preferences::preference_save_status(key, app),
                Some(crate::preferences::PreferenceSaveStatus::Failed(_))
            ));
        });
        assert_eq!(stored(visual, storage.as_ref(), key), before);

        for mode in [crate::Mode::Light, crate::Mode::Dark] {
            visual.update(|_, app| crate::apply_theme(mode, app));
            for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
                visual.simulate_resize(size(px(width), px(height)));
                visual.run_until_parked();
                let retry_area = bounds(visual, retry);
                let status_area = bounds(visual, status);
                let content = bounds(visual, "settings-content");
                let navigation = bounds(visual, "settings-navigation");
                assert!(retry_area.left() >= content.left());
                assert!(retry_area.right() <= content.right());
                assert!(retry_area.top() >= content.top());
                assert!(retry_area.bottom() <= content.bottom());
                assert!(status_area.left() >= content.left());
                assert!(status_area.right() <= content.right());
                assert!(status_area.top() >= content.top());
                assert!(status_area.bottom() <= content.bottom());
                assert!(navigation.size.width > px(0.0) && navigation.size.height > px(0.0));
            }
        }

        visual.simulate_resize(size(px(1024.0), px(768.0)));
        visual.run_until_parked();
        click(visual, "settings-page-ssh");
        click(visual, nav_page);
        assert!(
            visual.debug_bounds(retry).is_some(),
            "failed status survives page navigation"
        );
        click(visual, retry);
        visual.update(|_, app| {
            assert_eq!(
                crate::preferences::preference_save_status(key, app),
                Some(crate::preferences::PreferenceSaveStatus::Saved)
            );
        });
        assert_eq!(
            stored(visual, storage.as_ref(), key).as_deref(),
            Some(expected)
        );
        assert!(
            visual.debug_bounds(retry).is_none(),
            "successful retry removes retry action"
        );
        assert!(
            bounds(visual, "settings-save-area").size.height <= px(100.0),
            "short saved messages must not reserve a large blank status panel"
        );
    }

    visual.update(|window, app| {
        app.remove_global::<crate::StorageGlobal>();
        app.remove_global::<crate::activity_bar::UpdateIndicatorGlobal>();
        app.remove_global::<crate::SystemSettingsGlobal>();
        app.remove_global::<crate::MonitorSettingsGlobal>();
        app.remove_global::<crate::preferences::PreferenceWriterGlobal>();
        window.remove_window();
    });
    drop(root);
    drop(view);
    visual.run_until_parked();
    drop(ssh);
    drop(connection);
    drop(storage);
    // Entity releases are deferred until an app update, including each view's services.
    cx.update(|_| {});
    assert_eq!(
        Arc::strong_count(&test_storage),
        1,
        "unexpected remaining settings storage owners"
    );
    drop(test_storage);

    let reopened_result = RedbStorage::open_with_key(&path, &[0x4b; 32]);
    assert!(
        reopened_result.is_ok(),
        "isolated settings store should reopen: {}",
        reopened_result
            .as_ref()
            .err()
            .map(ToString::to_string)
            .unwrap_or_else(|| "unknown error".to_string())
    );
    let Ok(reopened) = reopened_result else {
        return;
    };
    let theme = futures::executor::block_on(reopened.get_preference("theme_mode"));
    assert!(
        theme.is_ok(),
        "theme preference should be readable after reopen"
    );
    assert_eq!(theme.ok().flatten().as_deref(), Some("light"));

    let system =
        futures::executor::block_on(reopened.get_preference(crate::SYSTEM_SETTINGS_PREF_KEY));
    assert!(
        system.is_ok(),
        "system settings should be readable after reopen"
    );
    let system = system.ok().flatten();
    assert!(
        system.is_some(),
        "saved system settings must survive reopening"
    );
    let Some(system) = system else {
        return;
    };
    let parsed_system = crate::SystemSettings::parse(&system);
    assert!(
        parsed_system.is_ok(),
        "system settings should parse after reopen"
    );
    let Ok(parsed_system) = parsed_system else {
        return;
    };
    assert!(!parsed_system.minimize_to_tray);
    assert_eq!(parsed_system.text_size, crate::InterfaceTextSize::Large);
    assert_eq!(
        parsed_system.scrollbar_visibility,
        crate::ScrollbarVisibility::Always
    );

    let monitor =
        futures::executor::block_on(reopened.get_preference(crate::MONITOR_SETTINGS_PREF_KEY));
    assert!(
        monitor.is_ok(),
        "monitor settings should be readable after reopen"
    );
    let monitor = monitor.ok().flatten();
    assert!(
        monitor.is_some(),
        "saved monitor settings must survive reopening"
    );
    let Some(monitor) = monitor else {
        return;
    };
    let parsed_monitor = serde_json::from_str::<crate::MonitorSettings>(&monitor);
    assert!(
        parsed_monitor.is_ok(),
        "monitor settings should parse after reopen"
    );
    let Ok(parsed_monitor) = parsed_monitor else {
        return;
    };
    assert_eq!(
        parsed_monitor.refresh_rate,
        crate::MonitorRefreshRate::FiveSeconds
    );

    drop(reopened);
    assert!(
        std::fs::remove_file(path).is_ok(),
        "isolated store should be removed"
    );
}
