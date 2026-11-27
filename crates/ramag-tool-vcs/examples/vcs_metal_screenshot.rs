//! 在 macOS 主线程运行 GPUI Metal 截图，用于验收 Git 工作区视觉状态。

use std::{
    error::Error,
    path::Path,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use gpui_kit::{AppContext as _, VisualTestAppContext, component::Root, platform, px, size};
use ramag_domain::entities::{
    Branch, BranchKind, Commit, ConnectionConfig, ConnectionId, DiffKind, FileDiff, LogOptions,
    QueryRecord, QueryRecordId, RepoConfig, RepoId, WorkingTreeStatus,
};
use ramag_domain::error::{DomainError, Result as DomainResult};
use ramag_domain::traits::{GitDriver, Storage};

struct MockGit;

#[async_trait]
impl GitDriver for MockGit {
    fn name(&self) -> &'static str {
        "screenshot-mock"
    }

    async fn open_repo(&self, _: &Path) -> DomainResult<RepoConfig> {
        Err(DomainError::NotImplemented("screenshot".into()))
    }

    async fn close_repo(&self, _: &RepoId) -> DomainResult<()> {
        Ok(())
    }

    async fn status(&self, _: &RepoId) -> DomainResult<WorkingTreeStatus> {
        Err(DomainError::NotImplemented("screenshot".into()))
    }

    async fn list_branches(&self, _: &RepoId, _: BranchKind) -> DomainResult<Vec<Branch>> {
        Ok(Vec::new())
    }

    async fn log(&self, _: &RepoId, _: LogOptions) -> DomainResult<Vec<Commit>> {
        Ok(Vec::new())
    }

    async fn diff_file(&self, _: &RepoId, _: &str, _: DiffKind) -> DomainResult<FileDiff> {
        Err(DomainError::NotImplemented("screenshot".into()))
    }
}

struct MockStorage;

#[async_trait]
impl Storage for MockStorage {
    async fn list_repos(&self) -> DomainResult<Vec<RepoConfig>> {
        Ok(vec![RepoConfig::from_path("/tmp/ramag-screenshot-repo")])
    }

    async fn list_connections(&self) -> DomainResult<Vec<ConnectionConfig>> {
        Ok(Vec::new())
    }

    async fn get_connection(&self, _: &ConnectionId) -> DomainResult<Option<ConnectionConfig>> {
        Ok(None)
    }

    async fn save_connection(&self, _: &ConnectionConfig) -> DomainResult<()> {
        Ok(())
    }

    async fn delete_connection(&self, _: &ConnectionId) -> DomainResult<()> {
        Ok(())
    }

    async fn append_history(&self, _: &QueryRecord) -> DomainResult<()> {
        Ok(())
    }

    async fn list_history(
        &self,
        _: Option<&ConnectionId>,
        _: usize,
    ) -> DomainResult<Vec<QueryRecord>> {
        Ok(Vec::new())
    }

    async fn delete_history(&self, _: &QueryRecordId) -> DomainResult<()> {
        Ok(())
    }

    async fn clear_history(&self, _: Option<&ConnectionId>) -> DomainResult<()> {
        Ok(())
    }

    async fn get_preference(&self, _: &str) -> DomainResult<Option<String>> {
        Ok(None)
    }

    async fn set_preference(&self, _: &str, _: &str) -> DomainResult<()> {
        Ok(())
    }
}

fn capture() -> std::result::Result<(), Box<dyn Error>> {
    eprintln!("screenshot runner started");
    let mut cx = VisualTestAppContext::with_asset_source(
        platform::current_platform(true),
        Arc::new(ramag_ui::RamagAssets),
    );
    cx.update(|app| {
        gpui_kit::component::init(app);
        ramag_ui::theme::init_theme(Some("dark"), app);
    });

    let output_dir = Path::new("/tmp/ramag-ui-headless");
    std::fs::create_dir_all(output_dir)?;
    let output = output_dir.join("vcs-repo-list-metal-1024x768.png");
    let mut view = None;
    let window = cx.open_offscreen_window(size(px(1024.0), px(768.0)), |window, app| {
        let entity = app.new(|cx| {
            ramag_tool_vcs::VcsView::new(Arc::new(MockGit), Arc::new(MockStorage), window, cx)
        });
        view = Some(entity.clone());
        app.new(|cx| Root::new(entity, window, cx))
    })?;
    let _ = view.ok_or("VCS 视图未创建")?;
    cx.run_until_parked();
    let image = cx.capture_screenshot(window.into())?;
    eprintln!("captured {}x{}", image.width(), image.height());
    if image.width() == 0 || image.height() == 0 || image.as_raw().iter().all(|byte| *byte == 0) {
        return Err("Metal 截图尺寸或像素内容无效".into());
    }
    image.save(&output)?;
    println!("{}", output.display());
    Ok(())
}

fn main() {
    let error = Arc::new(Mutex::new(None::<String>));
    let error_for_run = error.clone();
    gpui_kit::platform::application()
        .with_assets(ramag_ui::RamagAssets)
        .run(move |app| {
            if let Err(error) = capture()
                && let Ok(mut guard) = error_for_run.lock()
            {
                *guard = Some(error.to_string());
            }
            app.quit();
        });
    match error.lock() {
        Ok(guard) => {
            if let Some(error) = guard.as_ref() {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        Err(_) => {
            eprintln!("截图运行器未能读取错误状态");
            std::process::exit(1);
        }
    }
}
