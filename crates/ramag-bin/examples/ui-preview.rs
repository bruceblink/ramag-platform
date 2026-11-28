//! 隔离的原生 UI 验收入口，复用生产 Shell 与工具视图，不读取用户配置或连接。
//! 用法：cargo run -p ramag-bin --example ui-preview -- settings dark 1024 768
//! DBClient：dbclient dark 1024 768 large；F6 失败、F8 长错误、F7 恢复。
//! SQLite 表设计器：cargo run --locked -p ramag-bin --example ui-preview -- sqlite-designer dark 1024 768
//! 该模式仅连接本次预览创建的本地样例库。
//! SQLite 窄屏验收：sqlite-designer dark 360 640 --compact-window
//! API：api-local；连接 scripts/api-test 的本机 HTTP/gRPC 服务，包含可延迟的验收请求。
//! Kafka：kafka；仅连接 scripts/kafka-test 的本机测试 Broker。
//! 插件诊断：plugin-empty 或 plugin-failure；两者只构造本次窗口的受控验收状态。
//! 对象存储：object_storage；只读取 RAMAG_OBJECT_PREVIEW_* 配置的本机回环测试服务。
//! 容器：container-local；只使用 RAMAG_CONTAINER_PREVIEW_ENDPOINT 显式指定的本机只读 Docker 端点。
//! RAMAG_CONTAINER_PREVIEW_DELAY=3s 可延迟只读请求，用于检查切换平台或页面后的旧回调。
//! 数据同步遮罩：sync-progress、sync-cancel、sync-failure 或 sync-summary；只注入门禁状态。
//! settings/update 模式支持 Release 手动检查；MQTT 预览不会自动连接或启动 Broker。
//! 预览使用进程专属临时 redb；SQLite 表设计器模式另建本机临时目录。不会读取用户连接或系统凭据。

#[path = "ui_preview/api.rs"]
mod api;
#[path = "ui_preview/container.rs"]
mod container;
#[path = "ui_preview/dbclient.rs"]
mod dbclient;
#[path = "ui_preview/kafka.rs"]
mod kafka;
#[path = "ui_preview/mqtt.rs"]
mod mqtt;
#[path = "ui_preview/object_storage.rs"]
mod object_storage;
#[path = "ui_preview/offline_database.rs"]
mod offline_database;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::Root;
use gpui_kit::{App, AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};
use ramag_app::{
    DataSyncExecutionContext, DataSyncGate, DataSyncPermit, PluginContext, PluginOperationError,
    SshService, StaticPlugin, StaticPluginAdapter, StaticPluginHost, ToolRegistry,
};
use ramag_domain::{
    PluginDescriptor, PluginId, Tool, ToolMeta,
    entities::{ConnectionId, DataSyncProgress, DataSyncStage, DataSyncSummary, DataSyncTaskId},
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

/// 为 SQLite 设计器验收独占一份临时目录；清理只会删除本次成功创建的目录。
struct PreviewSqliteDirectory {
    path: PathBuf,
    owned: bool,
}

impl PreviewSqliteDirectory {
    fn create() -> anyhow::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "ramag-sqlite-designer-preview-{}-{}",
            std::process::id(),
            ConnectionId::new()
        ));
        std::fs::create_dir(&path)?;
        Ok(Self { path, owned: true })
    }

    fn database_path(&self) -> PathBuf {
        self.path.join("designer.sqlite")
    }

    fn cleanup(&mut self) -> anyhow::Result<()> {
        if !self.owned {
            return Ok(());
        }
        match std::fs::remove_dir_all(&self.path) {
            Ok(()) => self.owned = false,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => self.owned = false,
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }
}

impl Drop for PreviewSqliteDirectory {
    fn drop(&mut self) {
        if self.owned
            && let Err(error) = std::fs::remove_dir_all(&self.path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!(
                "UI preview cleanup failed for {}: {error}",
                self.path.display()
            );
        }
    }
}

/// 删除当前预览创建的临时记录；失败时返回具体路径，供退出钩子记录。
fn cleanup_preview_resources(
    redb_path: &Path,
    monitor_state: &Path,
    sqlite_directory: Option<PreviewSqliteDirectory>,
) -> anyhow::Result<()> {
    let mut failures = Vec::new();
    if let Err(error) = std::fs::remove_file(redb_path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        failures.push(format!("{}: {error}", redb_path.display()));
    }
    if let Err(error) = std::fs::remove_dir_all(monitor_state)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        failures.push(format!("{}: {error}", monitor_state.display()));
    }
    if let Some(mut directory) = sqlite_directory
        && let Err(error) = directory.cleanup()
    {
        failures.push(format!("{}: {error}", directory.path.display()));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("UI preview cleanup incomplete: {}", failures.join("; "))
    }
}

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
#[path = "ui_preview/cleanup_tests.rs"]
mod cleanup_tests;

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
    #[cfg(feature = "ui-profiling")]
    if std::env::var_os("RAMAG_UI_PROFILE").is_some() {
        // The normal preview stays silent; an explicit profiling run needs a
        // subscriber so its bounded foreground records are observable.
        tracing_subscriber::fmt::init();
    }
    let monitor_state =
        std::env::temp_dir().join(format!("ramag-monitor-preview-{}", std::process::id()));
    // Set the collector's configuration location before storage or GPUI starts
    // threads. A preview must never read or overwrite the user's monitor state.
    unsafe { std::env::set_var("RAMAG_SYSTEM_STATE_DIR", &monitor_state) };
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let smoke = raw_args.iter().any(|arg| arg == "--smoke");
    let compact_window = raw_args.iter().any(|arg| arg == "--compact-window");
    let args: Vec<String> = raw_args
        .into_iter()
        .filter(|arg| !matches!(arg.as_str(), "--smoke" | "--compact-window"))
        .collect();
    let page = args.first().cloned().unwrap_or_else(|| "settings".into());
    let sqlite_preview_dir = if page == "sqlite-designer" {
        Some(PreviewSqliteDirectory::create()?)
    } else {
        None
    };
    let container_endpoint = (page == "container-local")
        .then(container::preview_endpoint)
        .transpose()?;
    // ContainerView initializes from DOCKER_HOST; set only the already
    // validated preview endpoint before storage and GPUI workers start.
    if let Some(endpoint) = &container_endpoint {
        unsafe { std::env::set_var("DOCKER_HOST", endpoint) };
    }
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
        let mut tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(ramag_tool_dbclient::DbClientTool::new()),
            Arc::new(ramag_tool_api::ApiTool::new()),
            Arc::new(ramag_tool_container::ContainerTool::new()),
            Arc::new(ramag_tool_ssh::SshTool::new()),
            Arc::new(ramag_tool_system::SystemTool::new()),
            Arc::new(ramag_tool_object_storage::ObjectStorageTool::new()),
        ];
        if matches!(page.as_str(), "settings" | "mqtt") {
            tools.push(Arc::new(ramag_tool_mqtt::MqttTool::new()));
        }
        if page == "kafka" {
            tools.push(Arc::new(ramag_tool_kafka::KafkaTool::new()));
        }
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
    let preview = if let Some(directory) = &sqlite_preview_dir {
        dbclient::DbclientPreview::new_sqlite_designer(storage.clone(), &directory.database_path())?
    } else {
        dbclient::DbclientPreview::new(storage.clone())?
    };
    let sqlite_pool_cleanup = preview
        .sqlite_config
        .as_ref()
        .map(|config| (preview.connection.clone(), config.clone()));
    let api_service = (page == "api-local")
        .then(|| api::service(storage.clone()))
        .transpose()?;
    let kafka_service = (page == "kafka")
        .then(|| kafka::service(storage.clone()))
        .transpose()?;
    let container_services = container_endpoint
        .as_deref()
        .map(container::service)
        .transpose()?;
    let object_storage_service = (page == "object_storage")
        .then(|| object_storage::service(storage.clone()))
        .transpose()?;
    let sync_permit = seed_sync_preview(&page, &preview.gate);
    let ssh = Arc::new(SshService::new(
        Arc::new(OpenSshDriver::new()),
        storage.clone(),
    ));
    let mqtt = matches!(page.as_str(), "settings" | "mqtt")
        .then(|| mqtt::service(storage.clone(), page == "mqtt"))
        .transpose()?;
    let update_service = if matches!(page.as_str(), "settings" | "update") {
        let _ = rustls::crypto::ring::default_provider().install_default();
        Some(Arc::new(ramag_app::UpdateService::new(
            Arc::new(ramag_infra_update::GitHubUpdateDriver::new(env!(
                "CARGO_PKG_VERSION"
            ))?),
            storage.clone(),
            env!("CARGO_PKG_VERSION"),
        )))
    } else {
        None
    };
    let cleanup_redb_path = path.clone();
    let cleanup_monitor_state = monitor_state.clone();
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

            // macOS 可能在 NSApplication 终止时不返回 main 尾部；退出钩子在窗口释放后清理本次预览文件。
            let mut sqlite_preview_dir = sqlite_preview_dir;
            let mut sqlite_pool_cleanup = sqlite_pool_cleanup;
            let cleanup = cx.on_app_quit(move |cx| {
                if let Some((connection, config)) = sqlite_pool_cleanup.take() {
                    connection.evict_pool(&config);
                }
                let storage = cx.remove_global::<ramag_ui::StorageGlobal>();
                let sqlite_directory = sqlite_preview_dir.take();
                let redb_path = cleanup_redb_path.clone();
                let monitor_state = cleanup_monitor_state.clone();
                async move {
                    drop(storage);
                    if let Err(error) =
                        cleanup_preview_resources(&redb_path, &monitor_state, sqlite_directory)
                    {
                        eprintln!("UI preview cleanup failed: {error:#}");
                    }
                }
            });
            cleanup.detach();

            #[cfg(feature = "ui-profiling")]
            ramag_ui::profiling::start(cx);
            let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(if compact_window {
                        size(px(360.0), px(240.0))
                    } else {
                        size(px(1008.0), px(672.0))
                    }),
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
                            update_service.clone(),
                            window,
                            cx,
                        )
                    });
                    settings.update(cx, |view, cx| {
                        view.set_plugin_catalog_source_url(None, window, cx);
                    });
                    let api = match api_service {
                        Some(service) => ramag_tool_api::create_api_view(service, window, cx),
                        None => cx.new(|cx| ramag_tool_api::ApiView::new(window, cx)),
                    };
                    let containers = container_services.as_ref().map(|service| {
                        ramag_tool_container::create_container_view(service.clone(), window, cx)
                    });
                    let object_storage_view = object_storage_service.as_ref().map(|service| {
                        ramag_tool_object_storage::create_object_storage_view(
                            service.clone(),
                            window,
                            cx,
                        )
                    });
                    let ssh_view = ramag_tool_ssh::create_ssh_view(ssh, window, cx);
                    let monitor = ramag_tool_system::create_system_view(window, cx);
                    let dbclient = preview.create_view(window, cx);
                    let mqtt_view = mqtt.as_ref().map(|service| {
                        ramag_tool_mqtt::create_mqtt_view(service.clone(), window, cx)
                    });
                    let kafka_view = kafka_service.as_ref().map(|service| {
                        ramag_tool_kafka::create_kafka_view(service.clone(), window, cx)
                    });
                    let shell = cx.new(|cx| {
                        let mut shell =
                            Shell::new(registry, host, preview.gate.clone(), window, cx);
                        shell.set_home_view(home, window, cx);
                        shell.set_settings_view(settings.into());
                        shell.register_tool_view("api", api.into());
                        if let Some(containers) = containers {
                            shell.register_tool_view("container", containers.into());
                        } else {
                            let containers =
                                cx.new(|cx| ramag_tool_container::ContainerView::new(window, cx));
                            shell.register_tool_view("container", containers.into());
                        }
                        shell.register_tool_view("ssh", ssh_view.into());
                        shell.register_tool_view("system", monitor.into());
                        shell.register_tool_view("dbclient", dbclient.into());
                        if let Some(object_storage_view) = object_storage_view {
                            shell.register_tool_view("object_storage", object_storage_view.into());
                        }
                        if let Some(mqtt_view) = mqtt_view {
                            shell.register_tool_view("mqtt", mqtt_view.into());
                        }
                        if let Some(kafka_view) = kafka_view {
                            shell.register_tool_view("kafka", kafka_view.into());
                        }
                        let target = match page.as_str() {
                            "settings" | "update" | "plugin-empty" | "plugin-failure" => {
                                NavTarget::Settings
                            }
                            mode if SYNC_PREVIEW_MODES.contains(&mode) => {
                                NavTarget::Tool("dbclient".into())
                            }
                            "home" => NavTarget::Home,
                            "sqlite-designer" => NavTarget::Tool("dbclient".into()),
                            "api-local" => NavTarget::Tool("api".into()),
                            "container-local" => NavTarget::Tool("container".into()),
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
            let window_opened = result.is_ok();
            if let Err(error) = result {
                eprintln!("UI preview failed: {error}");
                cx.quit();
            }
            cx.activate(true);
            if smoke && window_opened {
                // CI smoke 模式在窗口创建成功后立即退出，避免依赖后台定时器保持进程运行。
                cx.quit();
            }
        });
    cleanup_preview_resources(&path, &monitor_state, None)?;
    Ok(())
}
