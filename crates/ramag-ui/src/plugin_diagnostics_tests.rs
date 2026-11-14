#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;
use std::sync::Arc;

use gpui_kit::{
    AppContext, Context, InteractiveElement, ParentElement, Render, StatefulInteractiveElement,
    Styled, TestAppContext, VisualTestContext, Window, div, px, size,
};
use ramag_app::{
    PluginContext, PluginInstallation, PluginInstallationState, PluginInstallationStore,
    PluginOperationError, PluginPackageCache, PluginPackageCachedArchive,
    PluginPackageCatalogEntry, PluginPackageInstaller, StaticPlugin, StaticPluginAdapter,
    StaticPluginHost, ToolRegistry,
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

#[path = "plugin_diagnostics_tests_catalog.rs"]
mod catalog;
#[path = "plugin_diagnostics_tests_installations.rs"]
mod installations;
