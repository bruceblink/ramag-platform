use ramag_domain::entities::{ReleaseAsset, ReleaseInfo};

use super::{
    UpdateCheckResult, UpdatePlatform, asset_name_for, cached_notice, evaluate_release,
    parse_cached_result,
};

#[test]
fn asset_names_are_exact_and_platform_specific() {
    assert_eq!(
        asset_name_for(UpdatePlatform::MacosArm64, "1.2.3"),
        "Ramag-1.2.3-macos-arm64.dmg"
    );
    assert_eq!(
        asset_name_for(UpdatePlatform::MacosX86_64, "1.2.3"),
        "Ramag-1.2.3-macos-x86_64.dmg"
    );
    assert_eq!(
        asset_name_for(UpdatePlatform::WindowsX86_64, "1.2.3"),
        "Ramag-1.2.3-windows-x64-setup.exe"
    );
    assert_eq!(
        asset_name_for(UpdatePlatform::LinuxDebX86_64, "1.2.3"),
        "Ramag-1.2.3-linux-amd64.deb"
    );
    assert_eq!(
        asset_name_for(UpdatePlatform::LinuxAppImageX86_64, "1.2.3"),
        "Ramag-1.2.3-linux-x86_64.AppImage"
    );
}

fn release(version: &str, with_asset: bool) -> ReleaseInfo {
    let name = asset_name_for(UpdatePlatform::MacosArm64, version);
    ReleaseInfo {
        version: version.into(),
        tag_name: format!("v{version}"),
        release_url: format!(
            "https://github.com/bruceblink/ramag-platform/releases/tag/v{version}"
        ),
        notes: String::new(),
        published_at: None,
        assets: with_asset
            .then(|| ReleaseAsset {
                name,
                download_url: "https://github.com/bruceblink/ramag-platform/releases/download/file"
                    .into(),
                size: 1,
                sha256: None,
                delta: None,
            })
            .into_iter()
            .collect(),
    }
}

#[test]
fn newer_version_requires_exact_platform_asset() {
    let available = evaluate_release(
        "1.0.0",
        release("1.1.0", true),
        Some(UpdatePlatform::MacosArm64),
    )
    .expect("newer release should evaluate");
    assert!(matches!(available, UpdateCheckResult::Available(_)));

    let unsupported = evaluate_release(
        "1.0.0",
        release("1.1.0", false),
        Some(UpdatePlatform::MacosArm64),
    )
    .expect("newer release should evaluate");
    assert!(matches!(
        unsupported,
        UpdateCheckResult::UnsupportedPlatform(_)
    ));
}

#[test]
fn same_older_and_prerelease_versions_are_handled_conservatively() {
    assert!(matches!(
        evaluate_release(
            "1.0.0",
            release("1.0.0", true),
            Some(UpdatePlatform::MacosArm64)
        ),
        Ok(UpdateCheckResult::UpToDate { .. })
    ));
    assert!(matches!(
        evaluate_release(
            "1.1.0",
            release("1.0.0", true),
            Some(UpdatePlatform::MacosArm64)
        ),
        Ok(UpdateCheckResult::UpToDate { .. })
    ));
    assert!(
        evaluate_release(
            "1.0.0",
            release("1.1.0-beta.1", true),
            Some(UpdatePlatform::MacosArm64)
        )
        .is_err()
    );
}

#[test]
fn cached_newer_version_requires_fresh_asset_metadata() {
    let available = evaluate_release(
        "1.0.0",
        release("1.1.0", true),
        Some(UpdatePlatform::MacosArm64),
    )
    .expect("newer release should evaluate");
    let notice = cached_notice("1.0.0", &available);
    let value = serde_json::to_string(&notice).expect("notice should serialize");

    assert!(
        parse_cached_result("1.0.0", &value)
            .expect("cached notice should parse")
            .is_none()
    );
}

#[test]
fn cached_update_notice_rejects_stale_or_noncanonical_versions() {
    let value = r#"{"current_version":"1.0.0","latest_version":"1.1.0"}"#;
    assert!(
        parse_cached_result("1.0.1", value)
            .expect("different app version should not be an error")
            .is_none()
    );

    let prerelease = r#"{"current_version":"1.0.0","latest_version":"1.1.0-beta.1"}"#;
    assert!(parse_cached_result("1.0.0", prerelease).is_err());
    assert!(parse_cached_result("1.0.0", &"x".repeat(257)).is_err());
}

#[tokio::test]
async fn automatic_preference_persists_and_failed_check_can_retry() {
    use async_trait::async_trait;
    use ramag_domain::entities::{UpdateCancellation, UpdateProgressFn};
    use ramag_domain::error::{DomainError, Result};
    use ramag_domain::traits::UpdateDriver;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct Driver {
        fail: AtomicBool,
        calls: AtomicUsize,
    }
    #[async_trait]
    impl UpdateDriver for Driver {
        async fn latest_stable_release(&self) -> Result<ReleaseInfo> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail.swap(false, Ordering::SeqCst) {
                return Err(DomainError::ConnectionFailed("offline".into()));
            }
            Ok(release("1.0.0", false))
        }
        async fn download_asset(
            &self,
            _: &ReleaseInfo,
            _: &ReleaseAsset,
            _: UpdateProgressFn,
            _: UpdateCancellation,
        ) -> Result<PathBuf> {
            Err(DomainError::Other("not used".into()))
        }
        fn reveal_download(&self, _: &Path) -> Result<()> {
            Ok(())
        }
        fn install_download(&self, _: &Path) -> Result<()> {
            Ok(())
        }
    }
    let dir = tempfile::tempdir().expect("isolated storage directory");
    let storage = Arc::new(
        ramag_infra_storage::RedbStorage::open_with_key(&dir.path().join("update.redb"), &[7; 32])
            .expect("storage"),
    );
    let driver = Arc::new(Driver {
        fail: AtomicBool::new(true),
        calls: AtomicUsize::new(0),
    });
    let service = super::UpdateService::new(driver.clone(), storage.clone(), "1.0.0");
    assert!(
        service
            .auto_check_enabled()
            .await
            .expect("default preference")
    );
    service
        .set_auto_check_enabled(false)
        .await
        .expect("save preference");
    let restarted = super::UpdateService::new(driver.clone(), storage, "1.0.0");
    assert!(
        !restarted
            .auto_check_enabled()
            .await
            .expect("reload preference")
    );
    assert!(restarted.check(false).await.is_err());
    assert!(matches!(
        restarted.check(false).await,
        Ok(UpdateCheckResult::UpToDate { .. })
    ));
    assert!(matches!(
        restarted.check(false).await,
        Ok(UpdateCheckResult::UpToDate { .. })
    ));
    assert_eq!(driver.calls.load(Ordering::SeqCst), 2);
}
