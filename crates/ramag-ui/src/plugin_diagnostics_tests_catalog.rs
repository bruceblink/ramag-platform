use super::*;

#[gpui_kit::test]
fn catalog_source_controls_stay_inside_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let host = diagnostic_host();
    let (_, visual_cx) = cx.add_window_view(move |window, cx| PluginDiagnosticsTestHost {
        view: cx.new(|view_cx| {
            let mut view = PluginDiagnosticsView::with_installations(host, Vec::new());
            view.set_catalog_source_url(None, window, view_cx);
            view
        }),
    });
    let visual_cx: &mut VisualTestContext = visual_cx;

    for mode in [crate::Mode::Light, crate::Mode::Dark] {
        visual_cx.update(|_, app| crate::apply_theme(mode, app));
        for (width, height) in [(360.0, 520.0), (1024.0, 520.0), (1440.0, 520.0)] {
            visual_cx.simulate_resize(size(px(width), px(height)));
            visual_cx.run_until_parked();
            let host_bounds = visual_cx
                .debug_bounds("plugin-diagnostics-scroll")
                .expect("插件诊断测试容器应渲染");
            let source = visual_cx
                .debug_bounds("plugin-package-catalog-source")
                .expect("目录源控件应渲染");
            let refresh = visual_cx
                .debug_bounds("plugin-package-catalog-refresh")
                .expect("目录刷新按钮应渲染");
            assert!(source.origin.x >= host_bounds.origin.x);
            assert!(source.right() <= host_bounds.right());
            assert_inside(refresh, source);
        }
    }
}

#[gpui_kit::test]
fn empty_plugin_diagnostics_shows_bounded_states(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let host = Arc::new(StaticPluginHost::new(Arc::new(ToolRegistry::new())));
    let host_for_view = host.clone();
    let (_, visual_cx) = cx.add_window_view(move |_, cx| PluginDiagnosticsTestHost {
        view: cx
            .new(|_| PluginDiagnosticsView::with_installations(host_for_view.clone(), Vec::new())),
    });
    let visual_cx: &mut VisualTestContext = visual_cx;

    for (width, height) in [(360.0, 520.0), (640.0, 520.0), (1024.0, 520.0)] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();

        let root = visual_cx
            .debug_bounds("plugin-diagnostics")
            .expect("空插件诊断页应渲染");
        let scroll = visual_cx
            .debug_bounds("plugin-diagnostics-scroll")
            .expect("空插件诊断页应有滚动区");
        let catalog = visual_cx
            .debug_bounds("plugin-catalog")
            .expect("空第一方目录应渲染");
        let catalog_empty = visual_cx
            .debug_bounds("plugin-catalog-empty-state")
            .expect("空第一方目录应显示状态");
        let catalog_heading = visual_cx
            .debug_bounds("plugin-catalog-empty-heading")
            .expect("空第一方目录应显示标题");
        let catalog_guidance = visual_cx
            .debug_bounds("plugin-catalog-empty-guidance")
            .expect("空第一方目录应显示引导");
        let package_catalog = visual_cx
            .debug_bounds("plugin-package-catalog")
            .expect("空本地目录候选区域应渲染");
        let package_catalog_empty = visual_cx
            .debug_bounds("plugin-package-catalog-empty-state")
            .expect("空本地目录候选应显示状态");
        let package_catalog_heading = visual_cx
            .debug_bounds("plugin-package-catalog-empty-heading")
            .expect("空本地目录候选应显示标题");
        let package_catalog_guidance = visual_cx
            .debug_bounds("plugin-package-catalog-empty-guidance")
            .expect("空本地目录候选应显示引导");
        let available = visual_cx
            .debug_bounds("plugin-available")
            .expect("空入口列表应渲染");
        let available_empty = visual_cx
            .debug_bounds("plugin-available-empty-state")
            .expect("空入口列表应显示状态");
        let available_heading = visual_cx
            .debug_bounds("plugin-available-empty-heading")
            .expect("空入口列表应显示标题");
        let available_guidance = visual_cx
            .debug_bounds("plugin-available-empty-guidance")
            .expect("空入口列表应显示引导");
        let diagnostics = visual_cx
            .debug_bounds("plugin-state-list")
            .expect("空插件状态列表应渲染");
        let diagnostics_empty = visual_cx
            .debug_bounds("plugin-diagnostics-empty-state")
            .expect("空插件状态列表应显示状态");
        let diagnostics_heading = visual_cx
            .debug_bounds("plugin-diagnostics-empty-heading")
            .expect("空插件状态列表应显示标题");
        let diagnostics_guidance = visual_cx
            .debug_bounds("plugin-diagnostics-empty-guidance")
            .expect("空插件状态列表应显示引导");
        let installations = visual_cx
            .debug_bounds("plugin-installations")
            .expect("空已安装插件列表应渲染");
        let installations_empty = visual_cx
            .debug_bounds("plugin-installations-empty-state")
            .expect("空已安装插件列表应显示状态");
        let installations_heading = visual_cx
            .debug_bounds("plugin-installations-empty-heading")
            .expect("空已安装插件列表应显示标题");
        let installations_guidance = visual_cx
            .debug_bounds("plugin-installations-empty-guidance")
            .expect("空已安装插件列表应显示引导");

        assert_eq!(root.size.width, px(width));
        assert!(scroll.right() <= px(width));
        assert!(scroll.bottom() <= px(height));
        for (empty_state, heading, guidance, parent) in [
            (catalog_empty, catalog_heading, catalog_guidance, catalog),
            (
                package_catalog_empty,
                package_catalog_heading,
                package_catalog_guidance,
                package_catalog,
            ),
            (
                available_empty,
                available_heading,
                available_guidance,
                available,
            ),
            (
                diagnostics_empty,
                diagnostics_heading,
                diagnostics_guidance,
                diagnostics,
            ),
            (
                installations_empty,
                installations_heading,
                installations_guidance,
                installations,
            ),
        ] {
            assert_inside(empty_state, parent);
            assert_inside(heading, empty_state);
            assert_inside(guidance, empty_state);
            assert_inside(empty_state, root);
            assert!(guidance.origin.y > heading.origin.y);
        }
    }
}

#[gpui_kit::test]
fn catalog_download_without_service_shows_bounded_error(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let host = Arc::new(StaticPluginHost::new(Arc::new(ToolRegistry::new())));
    let (_, visual_cx) = cx.add_window_view(move |_, cx| PluginDiagnosticsTestHost {
        view: cx.new(|view_cx| {
            let mut view = PluginDiagnosticsView::with_installations(host, Vec::new());
            let entry = catalog_plugin();
            view.set_catalog_entries(vec![entry.clone()], view_cx);
            view.download_package(entry, view_cx);
            view
        }),
    });
    let visual_cx: &mut VisualTestContext = visual_cx;
    visual_cx.simulate_resize(size(px(640.0), px(520.0)));
    visual_cx.run_until_parked();

    let root = visual_cx
        .debug_bounds("plugin-diagnostics")
        .expect("插件诊断页应渲染");
    let catalog = visual_cx
        .debug_bounds("plugin-package-catalog")
        .expect("本地目录候选区域应渲染");
    let error = visual_cx
        .debug_bounds("plugin-package-catalog-error")
        .expect("下载器未初始化时应显示错误状态卡");
    assert_inside(error, catalog);
    assert_inside(error, root);
}

#[gpui_kit::test]
fn downloaded_catalog_entry_shows_install_action(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let host = Arc::new(StaticPluginHost::new(Arc::new(ToolRegistry::new())));
    let (_, visual_cx) = cx.add_window_view(move |_, cx| PluginDiagnosticsTestHost {
        view: cx.new(|view_cx| {
            let mut view = PluginDiagnosticsView::with_installations(host, Vec::new());
            let entry = catalog_plugin();
            view.set_catalog_entries(vec![entry.clone()], view_cx);
            view.set_downloaded_packages(
                vec![PluginPackageCachedArchive {
                    plugin_id: entry.id.clone(),
                    version: entry.version.clone(),
                    path: std::path::PathBuf::from("/tmp/catalog.example.tar.gz"),
                }],
                view_cx,
            );
            view
        }),
    });
    let visual_cx: &mut VisualTestContext = visual_cx;
    visual_cx.simulate_resize(size(px(640.0), px(520.0)));
    visual_cx.run_until_parked();

    let install = visual_cx
        .debug_bounds("plugin-package-catalog-install-catalog.example-1.0.0")
        .expect("已下载目录候选应显示安装按钮");
    let status = visual_cx
        .debug_bounds("plugin-package-catalog-status-catalog.example-1.0.0")
        .expect("已下载目录候选应显示状态");
    let catalog = visual_cx
        .debug_bounds("plugin-package-catalog")
        .expect("本地目录候选区域应渲染");
    assert_inside(install, catalog);
    assert_inside(status, catalog);
}

#[gpui_kit::test]
fn refreshed_catalog_restores_matching_download_cache(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let host = Arc::new(StaticPluginHost::new(Arc::new(ToolRegistry::new())));
    let mut entry = catalog_plugin();
    entry.archive_sha256 =
        "ee2cc4e912b2aa9fd37f005dfbaf1496ebdf36f093f63764d230e37775873898".into();
    let root = std::env::temp_dir().join(format!(
        "ramag-ui-plugin-cache-refresh-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("创建缓存测试目录");
    let archive = root.join("catalog.example-1.0.0-1.tar.gz");
    fs::write(&archive, b"cached archive").expect("写入缓存归档");
    let cache = PluginPackageCache::new(root.clone());
    cache
        .record_download(&entry, &archive)
        .expect("记录已下载归档");
    let restored = super::super::package_catalog::restore_catalog_cache(
        Some(cache.clone()),
        std::slice::from_ref(&entry),
    )
    .expect("恢复缓存归档");
    assert_eq!(restored.len(), 1);
    let mut changed_entry = entry.clone();
    changed_entry.archive_sha256 = "f".repeat(64);
    assert!(
        super::super::package_catalog::restore_catalog_cache(Some(cache), &[changed_entry])
            .expect("核验变更摘要")
            .is_empty()
    );

    let (_, visual_cx) = cx.add_window_view(move |_, cx| PluginDiagnosticsTestHost {
        view: cx.new(|view_cx| {
            let mut view = PluginDiagnosticsView::with_installations(host, Vec::new());
            view.set_catalog_entries(vec![entry.clone()], view_cx);
            view.set_downloaded_packages(restored, view_cx);
            view
        }),
    });
    let visual_cx: &mut VisualTestContext = visual_cx;
    visual_cx.simulate_resize(size(px(640.0), px(520.0)));
    visual_cx.run_until_parked();
    visual_cx
        .debug_bounds("plugin-package-catalog-install-catalog.example-1.0.0")
        .expect("目录刷新后应保留缓存安装按钮");
    let _ = fs::remove_dir_all(root);
}

#[gpui_kit::test]
fn downloaded_catalog_entry_without_installer_shows_error(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let host = Arc::new(StaticPluginHost::new(Arc::new(ToolRegistry::new())));
    let (_, visual_cx) = cx.add_window_view(move |_, cx| PluginDiagnosticsTestHost {
        view: cx.new(|view_cx| {
            let mut view = PluginDiagnosticsView::with_installations(host, Vec::new());
            let entry = catalog_plugin();
            view.set_catalog_entries(vec![entry.clone()], view_cx);
            view.downloaded_packages.insert(
                super::super::package_catalog::package_download_key(&entry),
                std::path::PathBuf::from("/tmp/catalog.example.tar.gz"),
            );
            view.install_downloaded_package(entry, view_cx);
            view
        }),
    });
    let visual_cx: &mut VisualTestContext = visual_cx;
    visual_cx.simulate_resize(size(px(640.0), px(520.0)));
    visual_cx.run_until_parked();

    let error = visual_cx
        .debug_bounds("plugin-package-catalog-error")
        .expect("安装器未初始化时应显示错误状态卡");
    let catalog = visual_cx
        .debug_bounds("plugin-package-catalog")
        .expect("本地目录候选区域应渲染");
    assert_inside(error, catalog);
}

#[gpui_kit::test]
fn long_catalog_text_stays_inside_narrow_window(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let host = Arc::new(StaticPluginHost::new(Arc::new(ToolRegistry::new())));
    let mut entry = catalog_plugin();
    entry.name = "长插件名称".repeat(12);
    entry.package_url = format!(
        "https://plugins.example.test/{}",
        "very-long-path-segment-".repeat(70)
    );
    let (_, visual_cx) = cx.add_window_view(move |_, cx| PluginDiagnosticsTestHost {
        view: cx.new(|view_cx| {
            let mut view = PluginDiagnosticsView::with_installations(host, Vec::new());
            view.set_catalog_entries(vec![entry], view_cx);
            view
        }),
    });
    let visual_cx: &mut VisualTestContext = visual_cx;
    visual_cx.simulate_resize(size(px(360.0), px(520.0)));
    visual_cx.run_until_parked();

    let root = visual_cx
        .debug_bounds("plugin-diagnostics")
        .expect("插件诊断页应渲染");
    let catalog = visual_cx
        .debug_bounds("plugin-package-catalog")
        .expect("目录候选区域应渲染");
    let row = visual_cx
        .debug_bounds("plugin-package-catalog-catalog.example-1.0.0")
        .expect("长文本候选行应渲染");
    let url = visual_cx
        .debug_bounds("plugin-package-catalog-url-catalog.example-1.0.0")
        .expect("下载地址应有稳定选择器");
    let status = visual_cx
        .debug_bounds("plugin-package-catalog-status-catalog.example-1.0.0")
        .expect("候选状态应渲染");
    let button = visual_cx
        .debug_bounds("plugin-package-catalog-download-catalog.example-1.0.0")
        .expect("下载按钮应渲染");
    for bounds in [row, url, status, button] {
        assert_inside(bounds, catalog);
        assert_inside(bounds, root);
    }
}
