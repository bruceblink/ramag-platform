//! 对象存储确认框的隔离原生验收入口。直接启动即可，不需要环境变量或真实凭据。
//! 使用生产视图、应用服务和确认框；驱动只保存本次进程的受控测试数据，不联网。

#[path = "ui_preview/object_storage_offline.rs"]
mod object_storage_offline;

use std::sync::Arc;

use gpui_kit::component::Root;
use gpui_kit::{App, AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};
use ramag_domain::traits::Storage;
use ramag_infra_storage::RedbStorage;
use ramag_ui::{Mode, RamagAssets};

fn main() -> anyhow::Result<()> {
    let log_path = std::env::temp_dir().join(format!(
        "ramag-object-native-preview-{}.log",
        std::process::id()
    ));
    tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(std::fs::File::create_new(log_path)?))
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .init();
    let path = std::env::temp_dir().join(format!(
        "ramag-object-native-preview-{}.redb",
        std::process::id()
    ));
    let storage: Arc<dyn Storage> = Arc::new(RedbStorage::open_with_key(&path, &[0x42; 32])?);
    let service = object_storage_offline::service(storage.clone())?;
    gpui_kit::platform::application()
        .with_assets(RamagAssets)
        .run(move |cx: &mut App| {
            gpui_kit::component::init(cx);
            ramag_ui::apply_theme(Mode::Dark, cx);
            ramag_ui::set_system_settings(Default::default(), cx);
            cx.set_global(ramag_ui::StorageGlobal(storage));
            let bounds = Bounds::centered(None, size(px(1100.0), px(800.0)), cx);
            if let Err(error) = cx.open_window(
                WindowOptions {
                    titlebar: Some(gpui_kit::TitlebarOptions {
                        title: Some("Ramag — 对象覆盖原生验收（离线）".into()),
                        ..Default::default()
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(1008.0), px(672.0))),
                    ..Default::default()
                },
                move |window, cx| {
                    ramag_ui::apply_theme(Mode::Dark, cx);
                    let view =
                        ramag_tool_object_storage::create_object_storage_view(service, window, cx);
                    cx.new(|cx| Root::new(view, window, cx))
                },
            ) {
                eprintln!("Object storage native preview failed: {error}");
                cx.quit();
            }
            cx.activate(true);
        });
    if path.is_file() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
