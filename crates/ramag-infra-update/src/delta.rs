//! 可选安装包差分：只使用匹配旧包，重建文件仍须通过完整包 SHA-256 校验。

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use qbsdiff::Bspatch;
use ramag_domain::entities::{ReleaseAsset, ReleaseInfo, UpdateCancellation, UpdateProgressFn};
use ramag_domain::error::{DomainError, Result};
use reqwest::Client;
use sha2::{Digest, Sha256};

use super::{cache, download_verified_asset, path_exists, prepare_cache_dir, verify_file};

// 旧包与补丁需要驻留内存；大包继续走现有流式全量下载。
pub(super) const MAX_DELTA_BASE_BYTES: u64 = 128 * 1024 * 1024;

pub(super) async fn try_delta(
    client: &Client,
    release: &ReleaseInfo,
    asset: &ReleaseAsset,
    current_version: &str,
    cache_dir: &Path,
    progress: &UpdateProgressFn,
    cancellation: &UpdateCancellation,
) -> Result<Option<PathBuf>> {
    let Some(delta) = &asset.delta else {
        return Ok(None);
    };
    if cancellation.is_cancelled() {
        return Err(DomainError::Other("更新下载已取消".into()));
    }
    if delta.from_version != current_version {
        return Ok(None);
    }
    let Some(expected_hash) = &asset.sha256 else {
        return Ok(None);
    };
    let destination = cache_dir.join(&asset.name);
    prepare_cache_dir(cache_dir).await?;
    cache::reject_symlink(&destination).await?;
    if path_exists(&destination).await?
        && verify_file(&destination, asset.size, expected_hash).await?
    {
        return Ok(Some(destination));
    }
    let base_dir = cache_dir
        .parent()
        .ok_or_else(|| DomainError::Storage("更新缓存目录缺少父目录".into()))?
        .join(&delta.from_version);
    let base = base_dir.join(&delta.base_name);
    if let Some(parent) = base_dir.parent() {
        cache::reject_symlink(parent).await?;
    }
    cache::reject_symlink(&base_dir).await?;
    cache::reject_symlink(&base).await?;
    if !path_exists(&base).await?
        || !verify_file(&base, delta.base_size, &delta.base_sha256).await?
    {
        return Ok(None);
    }
    let patch_asset = ReleaseAsset {
        name: delta.patch_name.clone(),
        download_url: format!(
            "{}{}/{}",
            super::RELEASE_DOWNLOAD_PREFIX,
            release.tag_name,
            delta.patch_name
        ),
        size: delta.patch_size,
        sha256: Some(delta.patch_sha256.clone()),
        delta: None,
    };
    let patch = download_verified_asset(
        client,
        release,
        &patch_asset,
        cache_dir.to_path_buf(),
        progress.clone(),
        cancellation.clone(),
    )
    .await?;
    let partial = cache_dir.join(format!("{}.delta-part", asset.name));
    cache::reject_symlink(&partial).await?;
    if path_exists(&partial).await? {
        tokio::fs::remove_file(&partial)
            .await
            .map_err(|error| DomainError::Storage(format!("清理差分临时文件失败：{error}")))?;
    }
    let worker_partial = partial.clone();
    let asset = asset.clone();
    let cancellation_for_worker = cancellation.clone();
    let result = tokio::task::spawn_blocking(move || {
        reconstruct(
            &base,
            &patch,
            &worker_partial,
            &asset,
            &cancellation_for_worker,
        )
    })
    .await
    .map_err(|error| DomainError::Other(format!("增量重建任务失败：{error}")))
    .and_then(|result| result);
    if result.is_err() || cancellation.is_cancelled() {
        if let Err(error) = tokio::fs::remove_file(&partial).await
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(operation = "update_delta_cleanup", error = %error, "cleanup delta output failed");
        }
        return match result {
            Err(error) => Err(error),
            Ok(()) => Err(DomainError::Other("更新下载已取消".into())),
        };
    }
    if path_exists(&destination).await? {
        tokio::fs::remove_file(&destination)
            .await
            .map_err(|error| DomainError::Storage(format!("清理旧更新缓存失败：{error}")))?;
    }
    tokio::fs::rename(&partial, &destination)
        .await
        .map_err(|error| DomainError::Storage(format!("保存增量安装包失败：{error}")))?;
    tracing::info!(operation = "update_delta_complete", asset = %destination.display(), "verified installer reconstructed from delta");
    Ok(Some(destination))
}

fn reconstruct(
    base: &Path,
    patch: &Path,
    output: &Path,
    asset: &ReleaseAsset,
    cancellation: &UpdateCancellation,
) -> Result<()> {
    let delta = asset
        .delta
        .as_ref()
        .ok_or_else(|| DomainError::Other("缺少增量元数据".into()))?;
    let source = read_bounded(base, MAX_DELTA_BASE_BYTES)?;
    let patch_bytes = read_bounded(patch, MAX_DELTA_BASE_BYTES)?;
    // 读取后核对实际字节，避免缓存被更改后仍使用先前文件校验的结果。
    if hex::encode(Sha256::digest(&source)) != delta.base_sha256
        || hex::encode(Sha256::digest(&patch_bytes)) != delta.patch_sha256
    {
        return Err(DomainError::Other("增量输入校验失败".into()));
    }
    let patcher = Bspatch::new(&patch_bytes).map_err(delta_error)?;
    if patcher.hint_target_size() != asset.size {
        return Err(DomainError::Other("增量目标大小与完整安装包不一致".into()));
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(delta_error)?;
    let mut writer = VerifiedOutput {
        file,
        hasher: Sha256::new(),
        remaining: asset.size,
        cancellation,
    };
    patcher.apply(&source, &mut writer).map_err(delta_error)?;
    if writer.remaining != 0
        || asset.sha256.as_deref() != Some(hex::encode(writer.hasher.finalize()).as_str())
    {
        return Err(DomainError::Other("增量重建安装包 SHA-256 校验失败".into()));
    }
    writer.file.sync_all().map_err(delta_error)?;
    Ok(())
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(delta_error)?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(delta_error)?;
    if bytes.len() as u64 > limit {
        return Err(DomainError::Other("增量输入超过内存预算".into()));
    }
    Ok(bytes)
}

fn delta_error(error: io::Error) -> DomainError {
    DomainError::Other(format!("安装包差分处理失败：{error}"))
}

struct VerifiedOutput<'a> {
    file: std::fs::File,
    hasher: Sha256,
    remaining: u64,
    cancellation: &'a UpdateCancellation,
}

impl Write for VerifiedOutput<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cancellation.is_cancelled() {
            return Err(io::Error::other("更新下载已取消"));
        }
        if bytes.len() as u64 > self.remaining {
            return Err(io::Error::other("增量输出超过完整包大小"));
        }
        let count = self.file.write(bytes)?;
        self.hasher.update(&bytes[..count]);
        self.remaining -= count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests;
