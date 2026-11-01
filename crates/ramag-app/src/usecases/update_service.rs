//! 应用更新用例：版本比较、检查节流与安装包选择。

mod download;

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::lock::Mutex;
use parking_lot::RwLock;
use ramag_domain::entities::{ReleaseAsset, ReleaseInfo, UpdateCancellation, UpdateProgressFn};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::{Storage, UpdateDriver};
use semver::Version;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

pub const UPDATE_CHECK_PREF_KEY: &str = "update_last_check_at_v1";
pub const AUTO_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const AUTO_CHECK_PREF_KEY: &str = "update_auto_check_enabled_v1";
const UPDATE_RESULT_PREF_KEY: &str = "update_last_result_v1";
const MAX_CACHED_UPDATE_NOTICE_BYTES: usize = 256;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CachedUpdateNotice {
    current_version: String,
    latest_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdatePlatform {
    MacosArm64,
    MacosX86_64,
    WindowsX86_64,
    LinuxDebX86_64,
    LinuxAppImageX86_64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableUpdate {
    pub release: ReleaseInfo,
    pub asset: Option<ReleaseAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheckResult {
    UpToDate {
        current_version: String,
        latest_version: String,
    },
    Available(AvailableUpdate),
    UnsupportedPlatform(AvailableUpdate),
}

pub struct UpdateService {
    driver: Arc<dyn UpdateDriver>,
    storage: Arc<dyn Storage>,
    current_version: String,
    check_lock: Mutex<()>,
    last_result: RwLock<Option<UpdateCheckResult>>,
}

impl UpdateService {
    pub fn new(
        driver: Arc<dyn UpdateDriver>,
        storage: Arc<dyn Storage>,
        current_version: impl Into<String>,
    ) -> Self {
        Self {
            driver,
            storage,
            current_version: current_version.into(),
            check_lock: Mutex::new(()),
            last_result: RwLock::new(None),
        }
    }

    pub fn current_version(&self) -> &str {
        &self.current_version
    }

    pub fn last_result(&self) -> Option<UpdateCheckResult> {
        self.last_result.read().clone()
    }

    pub async fn auto_check_enabled(&self) -> Result<bool> {
        match self
            .storage
            .get_preference(AUTO_CHECK_PREF_KEY)
            .await?
            .as_deref()
        {
            None | Some("true") => Ok(true),
            Some("false") => Ok(false),
            Some(_) => Err(DomainError::Storage("自动检测更新设置无效".into())),
        }
    }

    pub async fn set_auto_check_enabled(&self, enabled: bool) -> Result<()> {
        self.storage
            .set_preference(AUTO_CHECK_PREF_KEY, if enabled { "true" } else { "false" })
            .await
    }

    pub async fn check(&self, force: bool) -> Result<UpdateCheckResult> {
        let _guard = self.check_lock.lock().await;
        info!(
            operation = "update_check",
            force,
            current_version = %self.current_version,
            "update check started"
        );
        if !force
            && !self.should_check().await
            && let Some(result) = self.cached_result().await
        {
            *self.last_result.write() = Some(result.clone());
            info!(
                operation = "update_check",
                result = update_result_kind(&result),
                source = "cache",
                "cached update check result restored"
            );
            return Ok(result);
        }
        let release = match self.driver.latest_stable_release().await {
            Ok(release) => release,
            Err(error) => {
                warn!(
                    operation = "update_check",
                    error = %error,
                    force,
                    current_version = %self.current_version,
                    "update check failed"
                );
                return Err(error);
            }
        };
        let result = match evaluate_release(&self.current_version, release, current_platform()) {
            Ok(result) => result,
            Err(error) => {
                warn!(
                    operation = "update_check",
                    error = %error,
                    force,
                    current_version = %self.current_version,
                    "update metadata validation failed"
                );
                return Err(error);
            }
        };
        self.persist_result(&result).await;
        self.mark_check_attempt().await;
        *self.last_result.write() = Some(result.clone());
        info!(
            operation = "update_check",
            result = update_result_kind(&result),
            source = "network",
            "update check completed"
        );
        Ok(result)
    }

    pub fn reveal_download(&self, path: &std::path::Path) -> Result<()> {
        self.driver.reveal_download(path)
    }

    pub fn install_download(&self, path: &std::path::Path) -> Result<()> {
        self.driver.install_download(path)
    }

    async fn should_check(&self) -> bool {
        let value = match self.storage.get_preference(UPDATE_CHECK_PREF_KEY).await {
            Ok(value) => value,
            Err(error) => {
                warn!(
                    operation = "update_check_throttle",
                    resource = "last_check_at",
                    error = %error,
                    "load update check timestamp failed"
                );
                return true;
            }
        };
        let Some(value) = value else { return true };
        let Ok(last) = value.parse::<u64>() else {
            return true;
        };
        let now = unix_timestamp();
        now < last || now.saturating_sub(last) >= AUTO_CHECK_INTERVAL.as_secs()
    }

    async fn mark_check_attempt(&self) {
        if let Err(error) = self
            .storage
            .set_preference(UPDATE_CHECK_PREF_KEY, &unix_timestamp().to_string())
            .await
        {
            warn!(
                operation = "update_check_throttle",
                resource = "last_check_at",
                error = %error,
                "persist update check timestamp failed"
            );
        }
    }

    async fn load_cached_result(&self) -> Option<UpdateCheckResult> {
        let value = match self.storage.get_preference(UPDATE_RESULT_PREF_KEY).await {
            Ok(value) => value,
            Err(error) => {
                warn!(
                    operation = "update_cache",
                    resource = "result",
                    error = %error,
                    "load cached update result failed"
                );
                return None;
            }
        }?;
        match parse_cached_result(&self.current_version, &value) {
            Ok(result) => result,
            Err(error) => {
                warn!(
                    operation = "update_cache",
                    resource = "result",
                    error = %error,
                    "parse cached update result failed"
                );
                None
            }
        }
    }

    async fn cached_result(&self) -> Option<UpdateCheckResult> {
        let result = self.last_result();
        if result.is_some() {
            return result;
        }
        self.load_cached_result().await
    }

    async fn persist_result(&self, result: &UpdateCheckResult) {
        let notice = cached_notice(&self.current_version, result);
        let value = match serde_json::to_string(&notice) {
            Ok(value) => value,
            Err(error) => {
                warn!(
                    operation = "update_cache",
                    resource = "result",
                    error = %error,
                    "serialize update result failed"
                );
                return;
            }
        };
        if let Err(error) = self
            .storage
            .set_preference(UPDATE_RESULT_PREF_KEY, &value)
            .await
        {
            warn!(
                operation = "update_cache",
                resource = "result",
                error = %error,
                "persist update result failed"
            );
        }
    }
}

fn update_result_kind(result: &UpdateCheckResult) -> &'static str {
    match result {
        UpdateCheckResult::UpToDate { .. } => "up_to_date",
        UpdateCheckResult::Available(_) => "available",
        UpdateCheckResult::UnsupportedPlatform(_) => "unsupported_platform",
    }
}

fn cached_notice(current_version: &str, result: &UpdateCheckResult) -> CachedUpdateNotice {
    let latest_version = match result {
        UpdateCheckResult::UpToDate { latest_version, .. } => latest_version.clone(),
        UpdateCheckResult::Available(update) | UpdateCheckResult::UnsupportedPlatform(update) => {
            update.release.version.clone()
        }
    };
    CachedUpdateNotice {
        current_version: current_version.into(),
        latest_version,
    }
}

fn parse_cached_result(current_version: &str, value: &str) -> Result<Option<UpdateCheckResult>> {
    if value.len() > MAX_CACHED_UPDATE_NOTICE_BYTES {
        return Err(DomainError::Storage("缓存更新结果过大".into()));
    }
    let cached: CachedUpdateNotice = serde_json::from_str(value)
        .map_err(|error| DomainError::Storage(format!("缓存更新结果无效：{error}")))?;
    if cached.current_version != current_version {
        return Ok(None);
    }
    let current = Version::parse(current_version)
        .map_err(|error| DomainError::InvalidConfig(format!("当前应用版本无效：{error}")))?;
    let latest = Version::parse(&cached.latest_version)
        .map_err(|error| DomainError::Storage(format!("缓存更新版本无效：{error}")))?;
    if current.to_string() != current_version
        || latest.to_string() != cached.latest_version
        || !latest.pre.is_empty()
    {
        return Err(DomainError::Storage("缓存更新版本不是规范稳定版本".into()));
    }
    if latest <= current {
        return Ok(Some(UpdateCheckResult::UpToDate {
            current_version: current_version.into(),
            latest_version: cached.latest_version,
        }));
    }
    // 版本缓存不含安装包元数据；重新取清单后才能判断平台是否受支持。
    Ok(None)
}

pub fn asset_name_for(platform: UpdatePlatform, version: &str) -> String {
    match platform {
        UpdatePlatform::MacosArm64 => format!("Ramag-{version}-macos-arm64.dmg"),
        UpdatePlatform::MacosX86_64 => format!("Ramag-{version}-macos-x86_64.dmg"),
        UpdatePlatform::WindowsX86_64 => format!("Ramag-{version}-windows-x64-setup.exe"),
        UpdatePlatform::LinuxDebX86_64 => format!("Ramag-{version}-linux-amd64.deb"),
        UpdatePlatform::LinuxAppImageX86_64 => {
            format!("Ramag-{version}-linux-x86_64.AppImage")
        }
    }
}

fn evaluate_release(
    current_version: &str,
    release: ReleaseInfo,
    platform: Option<UpdatePlatform>,
) -> Result<UpdateCheckResult> {
    let current = Version::parse(current_version).map_err(|error| {
        DomainError::InvalidConfig(format!("当前应用版本无效 {current_version}：{error}"))
    })?;
    let latest = Version::parse(&release.version).map_err(|error| {
        DomainError::Other(format!(
            "GitHub Release 版本无效 {}：{error}",
            release.version
        ))
    })?;
    if !latest.pre.is_empty() {
        return Err(DomainError::Other(format!(
            "稳定更新通道返回了预发布版本：{latest}"
        )));
    }
    if latest <= current {
        return Ok(UpdateCheckResult::UpToDate {
            current_version: current_version.to_string(),
            latest_version: release.version,
        });
    }
    let asset = platform
        .map(|platform| asset_name_for(platform, &release.version))
        .and_then(|name| release.assets.iter().find(|asset| asset.name == name))
        .cloned();
    let update = AvailableUpdate { release, asset };
    if update.asset.is_some() {
        Ok(UpdateCheckResult::Available(update))
    } else {
        Ok(UpdateCheckResult::UnsupportedPlatform(update))
    }
}

pub fn current_platform() -> Option<UpdatePlatform> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return Some(UpdatePlatform::MacosArm64);
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Some(UpdatePlatform::MacosX86_64);
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return Some(UpdatePlatform::WindowsX86_64);
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        if std::env::var_os("APPIMAGE").is_some_and(|path| !path.is_empty()) {
            return Some(UpdatePlatform::LinuxAppImageX86_64);
        }
        return Some(UpdatePlatform::LinuxDebX86_64);
    }
    #[allow(unreachable_code)]
    None
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
#[path = "update_service/tests.rs"]
mod tests;
