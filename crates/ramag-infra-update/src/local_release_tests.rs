//! 本机 Docker HTTPS Release 集成测试。

use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;

use qbsdiff::Bsdiff;
use ramag_domain::entities::{UpdateCancellation, UpdateProgressFn};
use reqwest::Client;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::{download_with_delta, metadata, trusted_redirect_policy};

const BASE_ASSET: &str = "Ramag-1.0.0-windows-x64.exe";
const RELEASE_ASSET: &str = "Ramag-1.1.0-windows-x64.exe";
const TEST_CURRENT_VERSION: &str = "1.0.0";

#[tokio::test]
#[ignore = "requires the local Docker HTTPS Release fixture"]
async fn downloads_stable_release_rebuilds_delta_and_falls_back_to_full_asset() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let root = PathBuf::from(
        std::env::var_os("RAMAG_UPDATE_TEST_ROOT").expect("test script must set fixture root"),
    );
    let port: u16 = std::env::var("RAMAG_UPDATE_TEST_PORT")
        .expect("test script must set HTTPS port")
        .parse()
        .expect("HTTPS port must be a number");
    let html_root = root.join("html");
    let access_log = root.join("logs/access.log");
    let cache_root = root.join("cache");
    let cache_dir = cache_root.join("1.1.0");

    let (old_package, new_package) = package_bytes();
    let mut patch = Vec::new();
    Bsdiff::new(&old_package, &new_package)
        .compare(&mut patch)
        .expect("generate BSDIFF patch");
    assert!(!patch.is_empty());
    assert!(patch.len() < new_package.len());

    let base_hash = sha256(&old_package);
    let package_hash = sha256(&new_package);
    let patch_hash = sha256(&patch);
    let patch_name = format!("{RELEASE_ASSET}.from-1.0.0.bsdiff");
    let release_files = html_root.join("bruceblink/ramag-platform/releases/download/v1.1.0");
    let manifest_file =
        html_root.join("bruceblink/ramag-platform/releases/latest/download/update-manifest.json");
    std::fs::create_dir_all(&release_files).expect("create release fixture directory");
    std::fs::create_dir_all(manifest_file.parent().expect("manifest parent"))
        .expect("create manifest fixture directory");
    std::fs::create_dir_all(cache_root.join(TEST_CURRENT_VERSION))
        .expect("create old version cache directory");
    std::fs::write(
        cache_root.join(TEST_CURRENT_VERSION).join(BASE_ASSET),
        &old_package,
    )
    .expect("write cached old installer");
    std::fs::write(release_files.join(RELEASE_ASSET), &new_package)
        .expect("write full installer fixture");
    std::fs::write(release_files.join(&patch_name), &patch).expect("write delta fixture");

    let manifest = json!({
        "schema_version": 1,
        "version": "1.1.0",
        "tag_name": "v1.1.0",
        "notes": "Local HTTPS integration fixture",
        "published_at": "2026-10-31T00:00:00Z",
        "assets": [{
            "name": RELEASE_ASSET,
            "size": new_package.len() as u64,
            "sha256": package_hash,
            "delta": {
                "from_version": TEST_CURRENT_VERSION,
                "base_name": BASE_ASSET,
                "base_size": old_package.len() as u64,
                "base_sha256": base_hash,
                "patch_name": patch_name,
                "patch_size": patch.len() as u64,
                "patch_sha256": patch_hash,
            }
        }]
    });
    std::fs::write(
        &manifest_file,
        serde_json::to_vec(&manifest).expect("serialize release manifest"),
    )
    .expect("write release manifest fixture");

    let client = Client::builder()
        .resolve(
            "github.com",
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port),
        )
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .redirect(trusted_redirect_policy())
        .build()
        .expect("build test-only HTTPS client");

    let release = metadata::fetch_latest_release(&client, TEST_CURRENT_VERSION)
        .await
        .expect("read the latest stable release from local HTTPS");
    assert_eq!(release.version, "1.1.0");
    assert_eq!(release.tag_name, "v1.1.0");
    let asset = release
        .assets
        .first()
        .expect("manifest contains a full installer");
    assert_eq!(asset.name, RELEASE_ASSET);
    assert!(asset.delta.is_some(), "manifest delta should validate");

    let progress: UpdateProgressFn = std::sync::Arc::new(|_| {});
    let output = download_with_delta(
        &client,
        &release,
        asset,
        TEST_CURRENT_VERSION,
        cache_dir.clone(),
        progress.clone(),
        UpdateCancellation::default(),
    )
    .await
    .expect("download and reconstruct the installer from delta");
    assert_eq!(output, cache_dir.join(RELEASE_ASSET));
    assert_eq!(
        std::fs::read(&output).expect("read reconstructed installer"),
        new_package
    );

    let first_log = wait_for_log_counts(
        &access_log,
        &[
            ("HEAD /bruceblink/ramag-platform/releases/latest HTTP/1.1", 1),
            (
                "HEAD /bruceblink/ramag-platform/releases/tag/v1.1.0 HTTP/1.1",
                1,
            ),
            (
                "GET /bruceblink/ramag-platform/releases/latest/download/update-manifest.json HTTP/1.1",
                1,
            ),
            (
                "GET /bruceblink/ramag-platform/releases/download/v1.1.0/",
                1,
            ),
        ])
    .await;
    assert_eq!(
        count_log_lines(
            &first_log,
            &format!(
                "GET /bruceblink/ramag-platform/releases/download/v1.1.0/{RELEASE_ASSET} HTTP/1.1"
            )
        ),
        0,
        "valid delta should avoid downloading the full installer"
    );

    std::fs::remove_file(&output).expect("remove first reconstructed installer");
    std::fs::remove_file(cache_dir.join(&patch_name)).expect("remove cached patch");
    let mut corrupt_patch = patch;
    corrupt_patch[0] ^= 0x80;
    std::fs::write(release_files.join(&patch_name), &corrupt_patch)
        .expect("replace served patch with corrupted bytes");

    let fallback_output = download_with_delta(
        &client,
        &release,
        asset,
        TEST_CURRENT_VERSION,
        cache_dir.clone(),
        progress.clone(),
        UpdateCancellation::default(),
    )
    .await
    .expect("fall back to the full installer after patch verification fails");
    assert_eq!(
        std::fs::read(&fallback_output).expect("read fallback installer"),
        new_package
    );
    let fallback_log = wait_for_log_counts(
        &access_log,
        &[
            (
                "GET /bruceblink/ramag-platform/releases/download/v1.1.0/",
                3,
            ),
            (
                &format!(
                    "GET /bruceblink/ramag-platform/releases/download/v1.1.0/{RELEASE_ASSET} HTTP/1.1"
                ),
                1,
            ),
        ],
    )
    .await;
    assert_eq!(
        count_log_lines(
            &fallback_log,
            &format!(
                "GET /bruceblink/ramag-platform/releases/download/v1.1.0/{patch_name} HTTP/1.1"
            )
        ),
        2,
        "the patch endpoint should receive both delta requests"
    );

    std::fs::remove_file(&fallback_output).expect("remove fallback installer before cancel test");
    let log_before_cancel = std::fs::read_to_string(&access_log).expect("read access log");
    let cancellation = UpdateCancellation::default();
    cancellation.cancel();
    let canceled = download_with_delta(
        &client,
        &release,
        asset,
        TEST_CURRENT_VERSION,
        cache_dir.clone(),
        progress,
        cancellation,
    )
    .await;
    assert!(canceled.is_err(), "pre-canceled download should fail");
    assert!(!cache_dir.join(RELEASE_ASSET).exists());
    assert!(!cache_dir.join(format!("{RELEASE_ASSET}.part")).exists());
    assert_eq!(
        std::fs::read_to_string(&access_log).expect("read access log after cancel"),
        log_before_cancel,
        "pre-cancel should not send another network request"
    );

    let mut full_asset = asset.clone();
    full_asset.delta = None;
    let cancellation = UpdateCancellation::default();
    let cancel_on_progress = cancellation.clone();
    let progress: UpdateProgressFn = std::sync::Arc::new(move |value| {
        if value.downloaded > 0 {
            cancel_on_progress.cancel();
        }
    });
    assert!(
        download_with_delta(
            &client,
            &release,
            &full_asset,
            TEST_CURRENT_VERSION,
            cache_dir.clone(),
            progress,
            cancellation.clone(),
        )
        .await
        .is_err(),
        "canceling after receiving bytes must not produce an installer"
    );
    assert!(cancellation.is_cancelled(), "download must have started");
    assert!(!cache_dir.join(RELEASE_ASSET).exists());
    assert!(!cache_dir.join(format!("{RELEASE_ASSET}.part")).exists());

    full_asset.sha256 = Some("0".repeat(64));
    assert!(
        download_with_delta(
            &client,
            &release,
            &full_asset,
            TEST_CURRENT_VERSION,
            cache_dir.clone(),
            std::sync::Arc::new(|_| {}),
            UpdateCancellation::default(),
        )
        .await
        .is_err(),
        "a full installer with a mismatching digest must be rejected"
    );
    assert!(!cache_dir.join(RELEASE_ASSET).exists());
    assert!(!cache_dir.join(format!("{RELEASE_ASSET}.part")).exists());
}

fn package_bytes() -> (Vec<u8>, Vec<u8>) {
    let mut state = 0x91e1_0da5_u32;
    let old = (0..512 * 1024)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect::<Vec<_>>();
    let mut new = old.clone();
    for (offset, byte) in new[80_000..81_024].iter_mut().enumerate() {
        *byte = (*byte).wrapping_add((offset as u8).wrapping_mul(13));
    }
    for (offset, byte) in new[310_000..310_512].iter_mut().enumerate() {
        *byte ^= (offset as u8).wrapping_mul(7);
    }
    new.extend((0..4096).map(|offset| (offset as u8).wrapping_mul(29)));
    (old, new)
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

async fn wait_for_log_counts(path: &Path, expected: &[(&str, usize)]) -> String {
    for _ in 0..100 {
        let log = std::fs::read_to_string(path).unwrap_or_default();
        if expected
            .iter()
            .all(|(needle, count)| count_log_lines(&log, needle) >= *count)
        {
            return log;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let log = std::fs::read_to_string(path).unwrap_or_default();
    panic!("timed out waiting for Nginx access entries: {expected:?}\n{log}");
}

fn count_log_lines(log: &str, needle: &str) -> usize {
    log.lines().filter(|line| line.contains(needle)).count()
}
