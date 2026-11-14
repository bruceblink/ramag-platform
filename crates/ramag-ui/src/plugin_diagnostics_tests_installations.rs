use super::*;

#[gpui_kit::test]
fn plugin_installation_feedback_notices_fit_supported_themes_and_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let host = Arc::new(StaticPluginHost::new(Arc::new(ToolRegistry::new())));
    let mut installation = installable_plugin();
    installation.last_error = Some("启动前包摘要核验失败".into());
    let plugin_id = installation.manifest.id.clone();
    let view = cx.new(|_| PluginDiagnosticsView::with_installations(host, vec![installation]));
    view.update(cx, |view, view_cx| {
        view.confirm_uninstall(plugin_id, view_cx);
    });
    let (_, visual_cx) = cx.add_window_view(move |_, _| PluginDiagnosticsTestHost { view });
    let visual_cx: &mut VisualTestContext = visual_cx;

    for mode in [crate::Mode::Light, crate::Mode::Dark] {
        visual_cx.update(|_, app| crate::apply_theme(mode, app));
        for (width, height) in [(360.0, 520.0), (1024.0, 520.0), (1440.0, 520.0)] {
            visual_cx.simulate_resize(size(px(width), px(height)));
            visual_cx.run_until_parked();

            let installations = visual_cx
                .debug_bounds("plugin-installations")
                .expect("插件安装状态区域应渲染");
            let operation_error = visual_cx
                .debug_bounds("plugin-installations-error")
                .expect("插件操作错误应使用统一状态卡");
            let recent_error = visual_cx
                .debug_bounds("plugin-installation-example.package-recent-error")
                .expect("插件最近错误应使用统一状态卡");

            assert!(installations.right() <= px(width));
            for notice in [operation_error, recent_error] {
                assert!(notice.origin.x >= installations.origin.x);
                assert!(notice.right() <= installations.right());
                assert!(notice.origin.y >= installations.origin.y);
                assert!(notice.bottom() <= installations.bottom());
            }
        }
    }
}

#[gpui_kit::test]
fn plugin_installation_toggle_persists_state(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let host = Arc::new(StaticPluginHost::new(Arc::new(ToolRegistry::new())));
    let installation = installable_plugin();
    let plugin_id = installation.manifest.id.clone();
    let root = std::env::temp_dir().join(format!(
        "ramag-ui-plugin-toggle-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("系统时间应有效")
            .as_nanos()
    ));
    let store = Arc::new(PluginInstallationStore::new(&root));
    store
        .upsert(installation.clone())
        .expect("测试插件状态应可写入");
    let store_for_view = store.clone();
    let view = cx.new(|view_cx| {
        let mut view = PluginDiagnosticsView::with_installations(host, vec![installation]);
        view.set_installation_store(store_for_view, view_cx);
        view
    });

    view.update(cx, |view, view_cx| {
        view.toggle_installation(plugin_id.clone(), view_cx);
    });
    assert_eq!(
        store
            .load()
            .expect("启用状态应可读取")
            .first()
            .expect("应保留插件状态")
            .state,
        PluginInstallationState::Enabled
    );

    view.update(cx, |view, view_cx| {
        view.toggle_installation(plugin_id, view_cx);
    });
    assert_eq!(
        store
            .load()
            .expect("禁用状态应可读取")
            .first()
            .expect("应保留插件状态")
            .state,
        PluginInstallationState::Disabled
    );
    let _ = std::fs::remove_dir_all(root);
}

#[gpui_kit::test]
fn plugin_installation_uninstall_confirmation_removes_state_and_package(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let host = Arc::new(StaticPluginHost::new(Arc::new(ToolRegistry::new())));
    let installation = installable_plugin();
    let plugin_id = installation.manifest.id.clone();
    let root = std::env::temp_dir().join(format!(
        "ramag-ui-plugin-uninstall-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("系统时间应有效")
            .as_nanos()
    ));
    let packages_root = root.join("packages");
    let state_root = root.join("state");
    let target = packages_root.join("example.package/1.2.3/bin/plugin");
    std::fs::create_dir_all(target.parent().expect("测试入口应有父目录"))
        .expect("测试插件目录应可创建");
    std::fs::write(&target, b"test plugin").expect("测试入口应可写入");
    let store = Arc::new(PluginInstallationStore::new(state_root));
    store
        .upsert(installation.clone())
        .expect("测试安装状态应可写入");
    let installer = Arc::new(PluginPackageInstaller::new(
        packages_root.clone(),
        store.clone(),
    ));
    let installer_for_view = installer.clone();
    let view = cx.new(|view_cx| {
        let mut view = PluginDiagnosticsView::with_installations(host, vec![installation]);
        view.set_package_installer(installer_for_view, view_cx);
        view
    });

    view.update(cx, |view, view_cx| {
        view.request_uninstall(plugin_id.clone(), view_cx);
        assert_eq!(view.pending_uninstall.as_ref(), Some(&plugin_id));
    });
    view.update(cx, |view, view_cx| {
        view.confirm_uninstall(plugin_id.clone(), view_cx);
    });
    assert!(store.load().expect("卸载后状态应可读取").is_empty());
    assert!(!target.exists());
    view.update(cx, |view, _| {
        assert!(view.installations.is_empty());
        assert!(view.pending_uninstall.is_none());
    });
    let _ = std::fs::remove_dir_all(root);
}
