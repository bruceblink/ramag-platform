//! 隔离的原生 UI 验收入口，复用生产 Shell 与工具视图，不读取用户配置或连接。
//! 用法：cargo run -p ramag-bin --example ui-preview -- settings dark 1024 768
//! DBClient：dbclient dark 1024 768 large；F6 失败、F8 长错误、F7 恢复。
//! 插件诊断：plugin-empty 或 plugin-failure；两者只构造本次窗口的受控验收状态。
//! 数据同步遮罩：sync-progress、sync-cancel、sync-failure 或 sync-summary；只注入门禁状态。
//! 仅创建进程专属临时 redb；不连接外部服务，不写系统凭据库，退出时清理临时文件。

#[path = "ui_preview/dbclient.rs"]
mod dbclient;
#[path = "ui_preview/offline_database.rs"]
mod offline_database;

use std::sync::Arc;

use gpui_kit::component::Root;
use gpui_kit::{App, AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};
use ramag_app::{
    DataSyncExecutionContext, DataSyncGate, DataSyncPermit, PluginContext, PluginOperationError,
    SshService, StaticPlugin, StaticPluginAdapter, StaticPluginHost, ToolRegistry,
};
use ramag_domain::{
    PluginDescriptor, PluginId, Tool, ToolMeta,
    entities::{DataSyncProgress, DataSyncStage, DataSyncSummary, DataSyncTaskId},
    traits::Storage,
};
use ramag_infra_ssh::OpenSshDriver;
use ramag_infra_storage::RedbStorage;
use ramag_ui::{HomeView, Mode, NavTarget, RamagAssets, SettingsView, Shell};

/// 只用于原生 UI 验收的最小静态工具，不进入生产组合根。
struct PreviewTool {
    meta: ToolMeta,
}

impl PreviewTool {
    fn new(id: &str, name: &str) -> Arc<Self> {
        Arc::new(Self {
            meta: ToolMeta::new(id, name, "ui-preview test tool"),
        })
    }
}

impl Tool for PreviewTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

/// 只在 `plugin-failure` 模式中返回初始化错误，不进入生产组合根。
struct FailingPreviewPlugin {
    descriptor: PluginDescriptor,
    tool: Arc<PreviewTool>,
}

impl StaticPlugin for FailingPreviewPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn tool(&self) -> Arc<dyn Tool> {
        self.tool.clone()
    }

    fn initialize(&self, _context: &PluginContext) -> Result<(), PluginOperationError> {
        Err(PluginOperationError::new(
            "ui-preview 受控初始化失败：用于检查插件错误状态卡。",
        ))
    }
}

const SYNC_PREVIEW_MODES: [&str; 4] = [
    "sync-progress",
    "sync-cancel",
    "sync-failure",
    "sync-summary",
];

/// 向共享门禁注入受控状态，供原生窗口检查同步遮罩；不会启动同步执行器。
fn seed_sync_preview(page: &str, gate: &Arc<DataSyncGate>) -> Option<DataSyncPermit> {
    if !SYNC_PREVIEW_MODES.contains(&page) {
        return None;
    }
    let permit = gate.begin(
        DataSyncTaskId::new(),
        DataSyncExecutionContext {
            source_connection: "UI 验收源".into(),
            source_scope: "preview_source".into(),
            target_connection: "UI 验收目标".into(),
            target_scope: "preview_target".into(),
        },
    )?;
    let progress = DataSyncProgress {
        stage: DataSyncStage::Writing,
        object: "orders".into(),
        objects_done: 42,
        objects_total: Some(100),
        scanned: 42,
        inserted: 36,
        skipped: 4,
        failed: 2,
        warnings: 1,
        bytes: 3 * 1024 * 1024,
        elapsed_ms: 1_240,
    };
    gate.update_progress(&permit, progress);
    match page {
        "sync-failure" => {
            let mut summary = DataSyncSummary {
                objects: 42,
                scanned: 42,
                inserted: 36,
                skipped: 4,
                failed: 2,
                bytes: 3 * 1024 * 1024,
                elapsed_ms: 1_240,
                ..DataSyncSummary::default()
            };
            summary.push_warning("UI 验收模拟：目标结构与源结构存在差异。");
            gate.finish_failed(
                &permit,
                summary,
                "UI 验收模拟同步失败：目标表 orders 的结构校验未通过，请检查目标结构后重试。",
            );
        }
        "sync-summary" => {
            let summary = DataSyncSummary {
                objects: 100,
                scanned: 100,
                inserted: 82,
                skipped: 16,
                failed: 2,
                bytes: 12 * 1024 * 1024,
                elapsed_ms: 4_820,
                ..DataSyncSummary::default()
            };
            gate.finish_completed(&permit, summary);
        }
        "sync-progress" | "sync-cancel" => {}
        _ => return None,
    }
    Some(permit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_app::DataSyncGatePhase;

    #[test]
    fn sync_preview_modes_seed_only_expected_gate_states() {
        for mode in SYNC_PREVIEW_MODES {
            let gate = Arc::new(DataSyncGate::default());
            let permit = seed_sync_preview(mode, &gate).expect("预览模式应创建受控同步状态");
            let snapshot = gate.snapshot().expect("预览状态应保持在门禁中");
            match mode {
                "sync-progress" | "sync-cancel" => {
                    assert_eq!(snapshot.phase, DataSyncGatePhase::Running);
                    assert_eq!(snapshot.progress.objects_total, Some(100));
                }
                "sync-failure" => {
                    assert_eq!(snapshot.phase, DataSyncGatePhase::Failed);
                    assert!(snapshot.error.is_some());
                }
                "sync-summary" => {
                    assert_eq!(snapshot.phase, DataSyncGatePhase::Completed);
                    assert!(snapshot.summary.is_some());
                }
                _ => unreachable!("未知同步预览模式"),
            }
            if snapshot.phase.terminal() {
                assert!(gate.acknowledge_and_release(&permit));
            } else {
                assert!(gate.request_cancel(&permit));
                assert!(gate.finish_cancelled(&permit, DataSyncSummary::default()));
                assert!(gate.acknowledge_and_release(&permit));
            }
        }
    }
}

/// 参数只选择预览页面、主题和窗口大小；数据始终来自本次新建的临时存储。
fn main() -> anyhow::Result<()> {
    if let Some(code) = ramag_tool_system::system_helper_entry(std::env::args_os()) {
        std::process::exit(code);
    }
    let monitor_state =
        std::env::temp_dir().join(format!("ramag-monitor-preview-{}", std::process::id()));
    // Set the collector's configuration location before storage or GPUI starts
    // threads. A preview must never read or overwrite the user's monitor state.
    unsafe { std::env::set_var("RAMAG_SYSTEM_STATE_DIR", &monitor_state) };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let page = args.first().cloned().unwrap_or_else(|| "settings".into());
    let mode = if args.get(1).is_some_and(|mode| mode == "dark") {
        Mode::Dark
    } else {
        Mode::Light
    };
    let width = args
        .get(2)
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|n| n.is_finite())
        .unwrap_or(1024.0)
        .clamp(360.0, 1920.0);
    let height = args
        .get(3)
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|n| n.is_finite())
        .unwrap_or(768.0)
        .clamp(240.0, 1080.0);
    let text_size = if args.get(4).is_some_and(|value| value == "large") {
        ramag_ui::InterfaceTextSize::Large
    } else {
        ramag_ui::InterfaceTextSize::Standard
    };
    let path = std::env::temp_dir().join(format!("ramag-ui-preview-{}.redb", std::process::id()));
    let storage: Arc<dyn Storage> = Arc::new(RedbStorage::open_with_key(&path, &[0x42; 32])?);
    let registry = Arc::new(ToolRegistry::new());
    let host = Arc::new(StaticPluginHost::new(registry.clone()));
    let plugin_diagnostics_mode = matches!(page.as_str(), "plugin-empty" | "plugin-failure");
    if !plugin_diagnostics_mode {
        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(ramag_tool_dbclient::DbClientTool::new()),
            Arc::new(ramag_tool_api::ApiTool::new()),
            Arc::new(ramag_tool_container::ContainerTool::new()),
            Arc::new(ramag_tool_ssh::SshTool::new()),
            Arc::new(ramag_tool_system::SystemTool::new()),
        ];
        for tool in tools {
            host.register_plugin(Arc::new(StaticPluginAdapter::from_tool(tool)?))?;
        }
        let _ = host.initialize_all();
    } else if page == "plugin-failure" {
        host.register_plugin(Arc::new(StaticPluginAdapter::from_tool(PreviewTool::new(
            "preview-ready",
            "验收就绪插件",
        ))?))?;
        host.register_plugin(Arc::new(FailingPreviewPlugin {
            descriptor: PluginDescriptor::new(
                PluginId::new("preview-failure")?,
                "验收失败插件",
                "preview-failure",
            ),
            tool: PreviewTool::new("preview-failure", "验收失败插件"),
        }))?;
        let _ = host.initialize_all();
    }
    let preview = dbclient::DbclientPreview::new(storage.clone())?;
    let sync_permit = seed_sync_preview(&page, &preview.gate);
    let ssh = Arc::new(SshService::new(
        Arc::new(OpenSshDriver::new()),
        storage.clone(),
    ));
    gpui_kit::platform::application()
        .with_assets(RamagAssets)
        .run(move |cx: &mut App| {
            gpui_kit::component::init(cx);
            ramag_tool_ssh::init(cx);
            dbclient::bind_controls(cx);
            ramag_ui::apply_theme(mode, cx);
            ramag_ui::set_system_settings(
                ramag_ui::SystemSettings {
                    text_size,
                    ..Default::default()
                },
                cx,
            );
            ramag_ui::set_monitor_settings(Default::default(), cx);
            cx.set_global(ramag_ui::StorageGlobal(storage));
            let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(1008.0), px(672.0))),
                    ..Default::default()
                },
                move |window, cx| {
                    // Native window creation can synchronize the OS appearance.
                    // Apply the requested acceptance theme after that initial sync.
                    ramag_ui::apply_theme(mode, cx);
                    let home = cx.new(|cx| HomeView::new(registry.clone(), cx));
                    let settings = cx.new(|cx| {
                        SettingsView::new(
                            host.clone(),
                            None,
                            preview.connection.clone(),
                            ssh.clone(),
                            None,
                            window,
                            cx,
                        )
                    });
                    let api = cx.new(|cx| ramag_tool_api::ApiView::new(window, cx));
                    let containers =
                        cx.new(|cx| ramag_tool_container::ContainerView::new(window, cx));
                    let ssh_view = ramag_tool_ssh::create_ssh_view(ssh, window, cx);
                    let monitor = ramag_tool_system::create_system_view(window, cx);
                    let dbclient = preview.create_view(window, cx);
                    let shell = cx.new(|cx| {
                        let mut shell =
                            Shell::new(registry, host, preview.gate.clone(), window, cx);
                        shell.set_home_view(home, window, cx);
                        shell.set_settings_view(settings.into());
                        shell.register_tool_view("api", api.into());
                        shell.register_tool_view("container", containers.into());
                        shell.register_tool_view("ssh", ssh_view.into());
                        shell.register_tool_view("system", monitor.into());
                        shell.register_tool_view("dbclient", dbclient.into());
                        let target = match page.as_str() {
                            "settings" | "plugin-empty" | "plugin-failure" => NavTarget::Settings,
                            mode if SYNC_PREVIEW_MODES.contains(&mode) => {
                                NavTarget::Tool("dbclient".into())
                            }
                            "home" => NavTarget::Home,
                            _ => NavTarget::Tool(page),
                        };
                        shell.navigate_to(target, window, cx);
                        shell
                    });
                    let preview_host = cx.new(|_| dbclient::PreviewHost {
                        shell,
                        preview,
                        _sync_permit: sync_permit,
                    });
                    cx.new(|cx| Root::new(preview_host, window, cx))
                },
            );
            if let Err(error) = result {
                eprintln!("UI preview failed: {error}");
                cx.quit();
            }
            cx.activate(true);
        });
    if path.is_file() {
        std::fs::remove_file(&path)?;
    }
    if monitor_state.is_dir() {
        std::fs::remove_dir_all(&monitor_state)?;
    }
    Ok(())
}
