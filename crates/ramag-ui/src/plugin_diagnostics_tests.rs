#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;

use gpui_kit::{
    AppContext, Context, InteractiveElement, ParentElement, Render, StatefulInteractiveElement,
    Styled, TestAppContext, VisualTestContext, Window, div, px, size,
};
use ramag_app::{
    PluginContext, PluginInstallation, PluginInstallationState, PluginInstallationStore,
    PluginOperationError, PluginPackageCatalogEntry, PluginPackageInstaller, StaticPlugin,
    StaticPluginAdapter, StaticPluginHost, ToolRegistry,
};
use ramag_domain::{
    MAX_PLUGIN_PACKAGE_SHA256_BYTES, PluginApiVersion, PluginCapability, PluginDescriptor,
    PluginId, PluginPackageManifest, PluginPackageRuntime, Tool, ToolMeta,
};

use super::PluginDiagnosticsView;

struct TestTool {
    meta: ToolMeta,
}

impl TestTool {
    fn new(id: &str, name: &str) -> Arc<Self> {
        Arc::new(Self {
            meta: ToolMeta::new(id, name, "headless test tool"),
        })
    }
}

impl Tool for TestTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

struct FailingPlugin {
    descriptor: PluginDescriptor,
    tool: Arc<TestTool>,
}

impl StaticPlugin for FailingPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn tool(&self) -> Arc<dyn Tool> {
        self.tool.clone()
    }

    fn initialize(&self, _context: &PluginContext) -> Result<(), PluginOperationError> {
        Err(PluginOperationError::new(format!(
            "headless 初始化失败：{}",
            "详细诊断信息 ".repeat(16)
        )))
    }
}

fn diagnostic_host() -> Arc<StaticPluginHost> {
    let registry = Arc::new(ToolRegistry::new());
    let host = Arc::new(StaticPluginHost::new(registry));
    let ready_tool = TestTool::new("ready-entry", "可用测试入口");
    host.register_plugin(Arc::new(
        StaticPluginAdapter::from_tool(ready_tool).unwrap(),
    ))
    .unwrap();

    let registration_tool = TestTool::new("actual-entry", "注册失败测试入口");
    let registration_descriptor = PluginDescriptor::new(
        PluginId::new("registration-failed").unwrap(),
        "注册失败测试插件",
        "declared-entry",
    );
    let _ = host.register_plugin(Arc::new(StaticPluginAdapter::new(
        registration_descriptor,
        registration_tool,
    )));

    let broken_tool = TestTool::new("broken-entry", "失败测试插件");
    host.register_plugin(Arc::new(FailingPlugin {
        descriptor: PluginDescriptor::new(
            PluginId::new("broken-plugin").unwrap(),
            "失败测试插件",
            "broken-entry",
        ),
        tool: broken_tool,
    }))
    .unwrap();
    host.initialize_all();
    host
}

fn installable_plugin() -> PluginInstallation {
    PluginInstallation::new(
        PluginPackageManifest::new(
            PluginId::new("example.package").unwrap(),
            "示例可安装插件",
            "1.2.3",
            "bin/example-plugin",
            "a".repeat(MAX_PLUGIN_PACKAGE_SHA256_BYTES),
        )
        .with_capabilities([ramag_domain::PluginCapability::new("ui.entry")]),
    )
}

fn catalog_plugin() -> PluginPackageCatalogEntry {
    PluginPackageCatalogEntry {
        id: PluginId::new("catalog.example").unwrap(),
        name: "目录示例插件".into(),
        version: "1.0.0".into(),
        api_version: PluginApiVersion::new(1, 0),
        runtime: PluginPackageRuntime::Process,
        package_url: "https://plugins.example.test/catalog.tar.gz".into(),
        archive_sha256: "a".repeat(64),
        capabilities: vec![PluginCapability::new("ui.entry")],
    }
}

fn assert_inside(
    child: gpui_kit::Bounds<gpui_kit::Pixels>,
    parent: gpui_kit::Bounds<gpui_kit::Pixels>,
) {
    assert!(child.origin.x >= parent.origin.x);
    assert!(child.origin.y >= parent.origin.y);
    assert!(child.right() <= parent.right());
    assert!(child.bottom() <= parent.bottom());
}

struct PluginDiagnosticsTestHost {
    view: gpui_kit::Entity<PluginDiagnosticsView>,
}

impl Render for PluginDiagnosticsTestHost {
    fn render(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        div()
            .id("plugin-diagnostics-scroll")
            .debug_selector(|| "plugin-diagnostics-scroll".into())
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .child(self.view.clone())
    }
}

#[gpui_kit::test]
fn plugin_diagnostics_stays_inside_supported_headless_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let host = diagnostic_host();
    let host_for_view = host.clone();
    let package_installer = Arc::new(PluginPackageInstaller::new(
        std::env::temp_dir().join("ramag-ui-plugin-diagnostics-packages"),
        Arc::new(PluginInstallationStore::new(
            std::env::temp_dir().join("ramag-ui-plugin-diagnostics-state"),
        )),
    ));
    let installer_for_view = package_installer.clone();
    let (_, visual_cx) = cx.add_window_view(move |_, cx| PluginDiagnosticsTestHost {
        view: cx.new(|view_cx| {
            let mut view = PluginDiagnosticsView::with_installations(
                host_for_view.clone(),
                vec![installable_plugin()],
            );
            view.set_package_installer(installer_for_view, view_cx);
            view.set_catalog_entries(vec![catalog_plugin()], view_cx);
            view
        }),
    });
    let visual_cx: &mut VisualTestContext = visual_cx;

    for mode in [crate::Mode::Light, crate::Mode::Dark] {
        visual_cx.update(|_, app| crate::apply_theme(mode, app));
        for (width, height) in [(360.0, 520.0), (1024.0, 520.0), (1440.0, 520.0)] {
            visual_cx.simulate_resize(size(px(width), px(height)));
            visual_cx.run_until_parked();

            let root = visual_cx
                .debug_bounds("plugin-diagnostics")
                .expect("插件诊断页应渲染");
            let scroll = visual_cx
                .debug_bounds("plugin-diagnostics-scroll")
                .expect("插件诊断滚动区应渲染");
            let summary = visual_cx
                .debug_bounds("plugin-summary")
                .expect("插件概览应渲染");
            let metric = visual_cx
                .debug_bounds("pulse-metric-card")
                .expect("插件概览应使用 Pulse 指标卡");
            let installations = visual_cx
                .debug_bounds("plugin-installations")
                .expect("可安装插件状态区域应渲染");
            let package_catalog = visual_cx
                .debug_bounds("plugin-package-catalog")
                .expect("本地目录候选区域应渲染");
            let package_entry = visual_cx
                .debug_bounds("plugin-package-catalog-catalog.example-1.0.0")
                .expect("本地目录候选应渲染");
            let package_download = visual_cx
                .debug_bounds("plugin-package-catalog-download-catalog.example-1.0.0")
                .expect("本地目录候选应显示下载按钮");
            let installation = visual_cx
                .debug_bounds("plugin-installation-example.package")
                .expect("可安装插件状态应渲染");
            let uninstall = visual_cx
                .debug_bounds("plugin-installation-uninstall-example.package")
                .expect("可安装插件应显示卸载按钮");
            let install_archive = visual_cx
                .debug_bounds("plugin-installation-install-archive")
                .expect("可安装插件应显示本地归档安装按钮");
            let catalog = visual_cx
                .debug_bounds("plugin-catalog")
                .expect("第一方目录应渲染");
            let catalog_entry = visual_cx
                .debug_bounds("plugin-catalog-ready-entry-ready-entry")
                .expect("第一方目录入口应渲染");
            let available = visual_cx
                .debug_bounds("plugin-available")
                .expect("可用入口区域应渲染");
            let states = visual_cx
                .debug_bounds("plugin-state-list")
                .expect("插件状态区域应渲染");
            let ready = visual_cx
                .debug_bounds("plugin-state-ready-entry")
                .expect("就绪插件状态应渲染");
            let broken = visual_cx
                .debug_bounds("plugin-state-broken-plugin")
                .expect("失败插件状态应渲染");
            let registration_failed = visual_cx
                .debug_bounds("plugin-state-registration-failed")
                .expect("注册失败插件状态应渲染");
            let broken_failure = visual_cx
                .debug_bounds("plugin-state-broken-plugin-failure")
                .expect("初始化失败状态卡应渲染");
            let registration_failure = visual_cx
                .debug_bounds("plugin-state-registration-failed-failure")
                .expect("注册失败状态卡应渲染");
            let entry = visual_cx
                .debug_bounds("plugin-available-ready-entry")
                .expect("可用入口明细应渲染");

            assert_eq!(root.size.width, px(width));
            assert!(scroll.origin.x >= px(0.0));
            assert!(scroll.origin.y >= px(0.0));
            assert!(scroll.right() <= px(width));
            assert!(scroll.bottom() <= px(height));
            assert!(root.origin.x >= scroll.origin.x);
            assert!(root.right() <= scroll.right());
            assert!(root.origin.y >= scroll.origin.y);
            for child in [
                summary,
                metric,
                installations,
                installation,
                uninstall,
                install_archive,
                package_catalog,
                package_entry,
                package_download,
                catalog,
                catalog_entry,
                available,
                states,
                ready,
                broken,
                registration_failed,
                broken_failure,
                registration_failure,
                entry,
            ] {
                assert_inside(child, root);
            }
            assert!(broken.bottom() <= states.bottom());
            assert!(broken_failure.bottom() <= broken.bottom());
            assert!(registration_failure.bottom() <= registration_failed.bottom());
            assert!(entry.bottom() <= available.bottom());
            assert!(catalog_entry.bottom() <= catalog.bottom());
            assert!(installation.bottom() <= installations.bottom());
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
            view.downloaded_packages.insert(
                super::package_download_key(&entry),
                std::path::PathBuf::from("/tmp/catalog.example.tar.gz"),
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
fn downloaded_catalog_entry_without_installer_shows_error(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let host = Arc::new(StaticPluginHost::new(Arc::new(ToolRegistry::new())));
    let (_, visual_cx) = cx.add_window_view(move |_, cx| PluginDiagnosticsTestHost {
        view: cx.new(|view_cx| {
            let mut view = PluginDiagnosticsView::with_installations(host, Vec::new());
            let entry = catalog_plugin();
            view.set_catalog_entries(vec![entry.clone()], view_cx);
            view.downloaded_packages.insert(
                super::package_download_key(&entry),
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
