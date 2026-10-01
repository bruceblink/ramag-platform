//! 隔离的原生 UI 验收入口，复用生产 Shell 与工具视图，不读取用户配置或连接。
//! 用法：cargo run -p ramag-bin --example ui-preview -- settings dark 1024 768
//! 仅创建进程专属临时 redb；不连接外部服务，不写系统凭据库，退出时清理临时文件。

use std::{collections::HashMap, sync::Arc};

use gpui_kit::component::Root;
use gpui_kit::{App, AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};
use ramag_app::{
    ConnectionService, DataSyncGate, SshService, StaticPluginAdapter, StaticPluginHost,
    ToolRegistry,
};
use ramag_domain::{Tool, traits::Storage};
use ramag_infra_ssh::OpenSshDriver;
use ramag_infra_storage::RedbStorage;
use ramag_ui::{HomeView, Mode, NavTarget, RamagAssets, SettingsView, Shell};

/// 参数只选择预览页面、主题和窗口大小；数据始终来自本次新建的临时存储。
fn main() -> anyhow::Result<()> {
    if let Some(code) = ramag_tool_system::system_helper_entry(std::env::args_os()) {
        std::process::exit(code);
    }
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
    let path = std::env::temp_dir().join(format!("ramag-ui-preview-{}.redb", std::process::id()));
    let storage: Arc<dyn Storage> = Arc::new(RedbStorage::open_with_key(&path, &[0x42; 32])?);
    let registry = Arc::new(ToolRegistry::new());
    let host = Arc::new(StaticPluginHost::new(registry.clone()));
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(ramag_tool_api::ApiTool::new()),
        Arc::new(ramag_tool_container::ContainerTool::new()),
        Arc::new(ramag_tool_ssh::SshTool::new()),
        Arc::new(ramag_tool_system::SystemTool::new()),
    ];
    for tool in tools {
        host.register_plugin(Arc::new(StaticPluginAdapter::from_tool(tool)?))?;
    }
    let _ = host.initialize_all();
    let connection = Arc::new(ConnectionService::new(HashMap::new(), storage.clone()));
    let ssh = Arc::new(SshService::new(
        Arc::new(OpenSshDriver::new()),
        storage.clone(),
    ));
    gpui_kit::platform::application()
        .with_assets(RamagAssets)
        .run(move |cx: &mut App| {
            gpui_kit::component::init(cx);
            ramag_tool_ssh::init(cx);
            ramag_ui::apply_theme(mode, cx);
            ramag_ui::set_system_settings(Default::default(), cx);
            ramag_ui::set_monitor_settings(Default::default(), cx);
            cx.set_global(ramag_ui::StorageGlobal(storage));
            let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(960.0), px(640.0))),
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
                            connection,
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
                    let shell = cx.new(|cx| {
                        let mut shell = Shell::new(
                            registry,
                            host,
                            Arc::new(DataSyncGate::default()),
                            window,
                            cx,
                        );
                        shell.set_home_view(home, window, cx);
                        shell.set_settings_view(settings.into());
                        shell.register_tool_view("api", api.into());
                        shell.register_tool_view("container", containers.into());
                        shell.register_tool_view("ssh", ssh_view.into());
                        shell.register_tool_view("system", monitor.into());
                        let target = match page.as_str() {
                            "settings" => NavTarget::Settings,
                            "home" => NavTarget::Home,
                            _ => NavTarget::Tool(page),
                        };
                        shell.navigate_to(target, window, cx);
                        shell
                    });
                    cx.new(|cx| Root::new(shell, window, cx))
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
    Ok(())
}
