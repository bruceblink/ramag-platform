//! 不依赖 GitHub REST API 配额的 Release 元数据读取。

use std::collections::HashSet;

use reqwest::{Client, StatusCode, Url};
use semver::Version;
use serde::Deserialize;

use ramag_domain::entities::{ReleaseAsset, ReleaseDelta, ReleaseInfo};
use ramag_domain::error::{DomainError, Result};

use super::{
    MAX_DOWNLOAD_BYTES, RELEASE_DOWNLOAD_PREFIX, bounded_response_body, ensure_success,
    is_safe_asset_name, request_error, truncate_utf8, validate_download_url, validate_sha256,
};

const LATEST_MANIFEST_URL: &str =
    "https://github.com/bruceblink/ramag-platform/releases/latest/download/update-manifest.json";
const LATEST_RELEASE_URL: &str = "https://github.com/bruceblink/ramag-platform/releases/latest";
const RELEASE_PAGE_PREFIX: &str = "https://github.com/bruceblink/ramag-platform/releases/tag/";
const UPDATE_MANIFEST_SCHEMA_VERSION: u32 = 1;
const MAX_METADATA_BYTES: usize = 2 * 1024 * 1024;
const MAX_RELEASE_NOTES_BYTES: usize = 64 * 1024;
const MAX_RELEASE_ASSETS: usize = 64;

#[derive(Deserialize)]
struct UpdateManifest {
    schema_version: u32,
    version: String,
    tag_name: String,
    #[serde(default)]
    notes: String,
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<ManifestAsset>,
}

#[derive(Deserialize)]
struct ManifestAsset {
    name: String,
    size: u64,
    sha256: String,
    #[serde(default)]
    delta: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct ManifestDelta {
    from_version: String,
    base_name: String,
    base_size: u64,
    base_sha256: String,
    patch_name: String,
    patch_size: u64,
    patch_sha256: String,
}

pub(super) async fn fetch_latest_release(
    client: &Client,
    current_version: &str,
) -> Result<ReleaseInfo> {
    let latest = fetch_latest_release_permalink(client).await?;
    let current = Version::parse(current_version)
        .map_err(|error| DomainError::InvalidConfig(format!("当前应用版本无效：{error}")))?;
    let latest_semver = Version::parse(&latest.version)
        .map_err(|error| DomainError::Other(format!("GitHub Release 版本无效：{error}")))?;
    if latest_semver <= current {
        return Ok(latest);
    }
    Ok(fetch_latest_manifest(client).await?.unwrap_or(latest))
}

async fn fetch_latest_manifest(client: &Client) -> Result<Option<ReleaseInfo>> {
    let response = client
        .get(LATEST_MANIFEST_URL)
        .send()
        .await
        .map_err(|error| request_error("检查 GitHub Release", error))?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let body = bounded_response_body(response, "检查 GitHub Release", MAX_METADATA_BYTES).await?;
    parse_manifest(&body).map(Some)
}

async fn fetch_latest_release_permalink(client: &Client) -> Result<ReleaseInfo> {
    let response = client
        .head(LATEST_RELEASE_URL)
        .send()
        .await
        .map_err(|error| request_error("检查 GitHub Release", error))?;
    ensure_success(&response, "检查 GitHub Release")?;
    parse_release_permalink(response.url())
}

pub(super) fn parse_manifest(body: &[u8]) -> Result<ReleaseInfo> {
    let manifest: UpdateManifest = serde_json::from_slice(body)
        .map_err(|error| DomainError::Other(format!("更新清单格式无效：{error}")))?;
    if manifest.schema_version != UPDATE_MANIFEST_SCHEMA_VERSION {
        return Err(DomainError::Other(format!(
            "更新清单版本不受支持：{}",
            manifest.schema_version
        )));
    }
    let version = Version::parse(&manifest.version)
        .map_err(|error| DomainError::Other(format!("更新清单版本无效：{error}")))?;
    if !version.pre.is_empty() {
        return Err(DomainError::Other(format!(
            "更新清单不能指向预发布版本：{version}"
        )));
    }
    let version = version.to_string();
    let expected_tag = format!("v{version}");
    if manifest.version != version || manifest.tag_name != expected_tag {
        return Err(DomainError::Other(format!(
            "更新清单版本与标签不一致：{} / {}",
            manifest.version, manifest.tag_name
        )));
    }
    if manifest.assets.len() > MAX_RELEASE_ASSETS {
        return Err(DomainError::Other(format!(
            "更新清单资产数量超过上限 {MAX_RELEASE_ASSETS}"
        )));
    }

    let mut names = HashSet::with_capacity(manifest.assets.len());
    let mut assets = Vec::with_capacity(manifest.assets.len());
    for asset in manifest.assets {
        if asset.name.is_empty() || asset.name.len() > 255 || !is_safe_asset_name(&asset.name) {
            return Err(DomainError::Other(format!(
                "更新清单资产名称无效：{}",
                asset.name
            )));
        }
        if !names.insert(asset.name.clone()) {
            return Err(DomainError::Other(format!(
                "更新清单包含重复资产：{}",
                asset.name
            )));
        }
        if asset.size == 0 || asset.size > MAX_DOWNLOAD_BYTES {
            return Err(DomainError::Other(format!(
                "更新清单资产大小无效：{} ({} bytes)",
                asset.name, asset.size
            )));
        }
        validate_sha256(&asset.sha256)?;
        let download_url = format!(
            "{RELEASE_DOWNLOAD_PREFIX}{}/{}",
            manifest.tag_name, asset.name
        );
        validate_download_url(&download_url, &manifest.tag_name, &asset.name)?;
        let delta = asset.delta.and_then(|delta| {
            // 补丁为可选优化；元数据错误时仍保留已经校验的完整安装包。
            serde_json::from_value(delta)
                .ok()
                .and_then(|delta| parse_delta(delta, &version, &asset.name, asset.size).ok())
        });
        assets.push(ReleaseAsset {
            name: asset.name,
            download_url,
            size: asset.size,
            sha256: Some(asset.sha256.to_ascii_lowercase()),
            delta,
        });
    }

    if let Some(published_at) = &manifest.published_at
        && !is_safe_published_at(published_at)
    {
        return Err(DomainError::Other("更新清单发布时间无效".into()));
    }

    Ok(ReleaseInfo {
        version,
        tag_name: manifest.tag_name.clone(),
        release_url: format!("{RELEASE_PAGE_PREFIX}{}", manifest.tag_name),
        notes: truncate_utf8(manifest.notes.trim(), MAX_RELEASE_NOTES_BYTES).to_string(),
        published_at: manifest.published_at,
        assets,
    })
}

fn parse_delta(delta: ManifestDelta, version: &str, name: &str, size: u64) -> Result<ReleaseDelta> {
    let from = Version::parse(&delta.from_version)
        .map_err(|error| DomainError::Other(format!("增量基线版本无效：{error}")))?;
    let target = Version::parse(version)
        .map_err(|error| DomainError::Other(format!("增量目标版本无效：{error}")))?;
    if !name.starts_with(&format!("Ramag-{version}-")) {
        return Err(DomainError::Other("增量目标名称无效".into()));
    }
    let expected_base = name.replacen(&format!("Ramag-{version}-"), &format!("Ramag-{from}-"), 1);
    if !from.pre.is_empty()
        || from.to_string() != delta.from_version
        || from >= target
        || delta.base_name != expected_base
        || !is_safe_asset_name(&delta.base_name)
        || delta.patch_name != format!("{name}.from-{from}.bsdiff")
        || !is_safe_asset_name(&delta.patch_name)
        || delta.base_size == 0
        || delta.base_size > super::delta::MAX_DELTA_BASE_BYTES
        || size > super::delta::MAX_DELTA_BASE_BYTES
        || delta.patch_size == 0
        || delta.patch_size >= size
    {
        return Err(DomainError::Other("增量安装包元数据无效".into()));
    }
    validate_sha256(&delta.base_sha256)?;
    validate_sha256(&delta.patch_sha256)?;
    Ok(ReleaseDelta {
        from_version: delta.from_version,
        base_name: delta.base_name,
        base_size: delta.base_size,
        base_sha256: delta.base_sha256.to_ascii_lowercase(),
        patch_name: delta.patch_name,
        patch_size: delta.patch_size,
        patch_sha256: delta.patch_sha256.to_ascii_lowercase(),
    })
}

pub(super) fn parse_release_permalink(url: &Url) -> Result<ReleaseInfo> {
    let prefix = "/bruceblink/ramag-platform/releases/tag/";
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.path().starts_with(prefix)
    {
        return Err(DomainError::ConnectionFailed(
            "GitHub 最新版本跳转地址无效".into(),
        ));
    }
    let tag_name = &url.path()[prefix.len()..];
    let raw_version = tag_name
        .strip_prefix('v')
        .ok_or_else(|| DomainError::Other(format!("GitHub Release 标签格式无效：{tag_name}")))?;
    let version = Version::parse(raw_version)
        .map_err(|error| DomainError::Other(format!("GitHub Release 版本无效：{error}")))?;
    if !version.pre.is_empty() || version.to_string() != raw_version {
        return Err(DomainError::Other(format!(
            "GitHub 最新版本不是规范稳定版本：{tag_name}"
        )));
    }
    Ok(ReleaseInfo {
        version: version.to_string(),
        tag_name: tag_name.to_string(),
        release_url: url.to_string(),
        notes: String::new(),
        published_at: None,
        assets: Vec::new(),
    })
}

fn is_safe_published_at(value: &str) -> bool {
    (20..=35).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_digit() || matches!(byte, b'-' | b':' | b'T' | b'Z' | b'.' | b'+')
        })
}
