//! 应用更新源与平台文件操作抽象。

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::entities::{ReleaseAsset, ReleaseInfo, UpdateCancellation, UpdateProgressFn};
use crate::error::Result;

#[async_trait]
pub trait UpdateDriver: Send + Sync {
    async fn latest_stable_release(&self) -> Result<ReleaseInfo>;

    async fn download_asset(
        &self,
        release: &ReleaseInfo,
        asset: &ReleaseAsset,
        progress: UpdateProgressFn,
        cancellation: UpdateCancellation,
    ) -> Result<PathBuf>;

    fn reveal_download(&self, path: &Path) -> Result<()>;

    /// 由用户点击后启动已校验的安装包；返回值只表示启动结果，不表示安装完成。
    /// 不退出当前应用，也不绕过系统安装确认。
    fn install_download(&self, path: &Path) -> Result<()>;
}
