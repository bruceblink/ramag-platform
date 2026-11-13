//! 插件包候选的有界 HTTPS 下载和摘要核验；不自动安装或执行下载内容。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures::StreamExt as _;
use reqwest::{Client, StatusCode, Url, redirect};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::AsyncWriteExt as _;
use tokio::runtime::{Builder, Runtime};

use crate::{PluginPackageCatalogEntry, PluginPackageCatalogError};

/// 单个插件包允许下载的压缩归档字节数。
pub const MAX_PLUGIN_DOWNLOAD_BYTES: u64 = 64 * 1024 * 1024;
const MAX_REDIRECTS: usize = 0;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
static DOWNLOAD_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 负责把已校验目录候选下载到本地缓存；不会调用安装器。
#[derive(Clone)]
pub struct PluginPackageDownloader {
    client: Client,
}

impl PluginPackageDownloader {
    /// 创建禁止重定向的 HTTPS 客户端；调用方应在应用启动时先安装 TLS Provider。
    pub fn new() -> Result<Self, PluginPackageDownloadError> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(DOWNLOAD_TIMEOUT)
            .redirect(redirect::Policy::limited(MAX_REDIRECTS))
            .user_agent("Ramag/plugin-package-downloader")
            .build()
            .map_err(|source| PluginPackageDownloadError::Client { source })?;
        Ok(Self { client })
    }

    /// 使用已配置客户端创建下载器；重定向和 TLS 策略由调用方负责。
    pub fn with_client(client: Client) -> Self {
        Self { client }
    }

    /// 在下载模块专用 Tokio runtime 中执行下载，供 GPUI 等非 Tokio 执行器调用。
    pub async fn download_to_managed_runtime(
        &self,
        entry: &PluginPackageCatalogEntry,
        destination: &Path,
    ) -> Result<PathBuf, PluginPackageDownloadError> {
        let downloader = self.clone();
        let entry = entry.clone();
        let destination = destination.to_path_buf();
        download_runtime()?
            .spawn(async move { downloader.download_to(&entry, &destination).await })
            .await
            .map_err(|source| PluginPackageDownloadError::TaskJoin {
                message: source.to_string(),
            })?
    }

    /// 下载一个已校验候选到新文件，并在原子发布前核对 SHA-256。
    pub async fn download_to(
        &self,
        entry: &PluginPackageCatalogEntry,
        destination: &Path,
    ) -> Result<PathBuf, PluginPackageDownloadError> {
        entry
            .validate()
            .map_err(PluginPackageDownloadError::Catalog)?;
        validate_destination(destination)?;
        let url = Url::parse(&entry.package_url).map_err(|_| {
            PluginPackageDownloadError::Catalog(PluginPackageCatalogError::InvalidUrl {
                index: 0,
                url: entry.package_url.clone(),
            })
        })?;
        let parent =
            destination
                .parent()
                .ok_or_else(|| PluginPackageDownloadError::DestinationInvalid {
                    path: destination.to_path_buf(),
                })?;
        tracing::info!(
            operation = "plugin_package_download",
            plugin_id = %entry.id,
            version = %entry.version,
            destination = %destination.display(),
            "插件包下载开始"
        );
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|source| PluginPackageDownloadError::Io { source })?;
        reject_symlink(parent)?;
        if tokio::fs::try_exists(destination)
            .await
            .map_err(|source| PluginPackageDownloadError::Io { source })?
        {
            return Err(PluginPackageDownloadError::DestinationExists {
                path: destination.to_path_buf(),
            });
        }
        let partial = parent.join(format!(
            ".{}.part-{}-{}",
            destination
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("plugin"),
            std::process::id(),
            DOWNLOAD_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|source| PluginPackageDownloadError::Request { source })?;
        if response.status() != StatusCode::OK {
            return Err(PluginPackageDownloadError::HttpStatus {
                status: response.status().as_u16(),
            });
        }
        if response.url() != &url {
            return Err(PluginPackageDownloadError::Redirect {
                expected: url.to_string(),
                actual: response.url().to_string(),
            });
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_PLUGIN_DOWNLOAD_BYTES)
        {
            return Err(PluginPackageDownloadError::TooLarge {
                bytes: response.content_length().unwrap_or_default(),
                max: MAX_PLUGIN_DOWNLOAD_BYTES,
            });
        }
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
            .await
            .map_err(|source| PluginPackageDownloadError::Io { source })?;
        // 只有成功创建当前临时文件后才登记清理，避免 create_new 失败时删除其他任务留下的文件。
        let mut cleanup = PartialDownloadGuard::new(partial.clone());
        let mut writer = tokio::io::BufWriter::new(file);
        let mut hasher = Sha256::new();
        let mut downloaded = 0_u64;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|source| PluginPackageDownloadError::Request { source })?;
            downloaded = downloaded.saturating_add(chunk.len() as u64);
            if downloaded > MAX_PLUGIN_DOWNLOAD_BYTES {
                return Err(PluginPackageDownloadError::TooLarge {
                    bytes: downloaded,
                    max: MAX_PLUGIN_DOWNLOAD_BYTES,
                });
            }
            hasher.update(&chunk);
            writer
                .write_all(&chunk)
                .await
                .map_err(|source| PluginPackageDownloadError::Io { source })?;
        }
        writer
            .flush()
            .await
            .map_err(|source| PluginPackageDownloadError::Io { source })?;
        let file = writer.into_inner();
        file.sync_all()
            .await
            .map_err(|source| PluginPackageDownloadError::Io { source })?;
        let actual = hex::encode(hasher.finalize());
        if actual != entry.archive_sha256 {
            return Err(PluginPackageDownloadError::DigestMismatch {
                expected: entry.archive_sha256.clone(),
                actual,
            });
        }
        tokio::fs::hard_link(&partial, destination)
            .await
            .map_err(|source| PluginPackageDownloadError::Io { source })?;
        // 硬链接创建不会覆盖已出现的目标文件，删除临时链接后保留同一份完整内容。
        match tokio::fs::remove_file(&partial).await {
            Ok(()) => cleanup.disarm(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => cleanup.disarm(),
            Err(error) => tracing::warn!(
                operation = "plugin_package_download_cleanup",
                path = %partial.display(),
                error = %error,
                "插件包下载临时链接清理失败"
            ),
        }
        tracing::info!(
            operation = "plugin_package_download",
            plugin_id = %entry.id,
            version = %entry.version,
            destination = %destination.display(),
            bytes = downloaded,
            "插件包下载完成"
        );
        Ok(destination.to_path_buf())
    }
}

fn download_runtime() -> Result<&'static Runtime, PluginPackageDownloadError> {
    static RUNTIME: OnceLock<Result<Runtime, String>> = OnceLock::new();
    match RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("ramag-plugin-download-tokio")
            .enable_all()
            .build()
            .map_err(|error| format!("创建插件包下载 runtime 失败：{error}"))
    }) {
        Ok(runtime) => Ok(runtime),
        Err(message) => Err(PluginPackageDownloadError::Runtime {
            message: message.clone(),
        }),
    }
}

fn validate_destination(path: &Path) -> Result<(), PluginPackageDownloadError> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(PluginPackageDownloadError::DestinationInvalid {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), PluginPackageDownloadError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|source| PluginPackageDownloadError::Io { source })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PluginPackageDownloadError::DestinationInvalid {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

struct PartialDownloadGuard {
    path: PathBuf,
    armed: bool,
}

impl PartialDownloadGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PartialDownloadGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if let Err(error) = std::fs::remove_file(&self.path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(
                operation = "plugin_package_download_cleanup",
                path = %self.path.display(),
                error = %error,
                "插件包下载临时文件清理失败"
            );
        }
    }
}

#[derive(Debug, Error)]
pub enum PluginPackageDownloadError {
    #[error("插件目录清单校验失败：{0}")]
    Catalog(PluginPackageCatalogError),
    #[error("创建插件包下载客户端失败：{source}")]
    Client { source: reqwest::Error },
    #[error("插件包下载请求失败：{source}")]
    Request { source: reqwest::Error },
    #[error("创建插件包下载 runtime 失败：{message}")]
    Runtime { message: String },
    #[error("插件包下载任务异常退出：{message}")]
    TaskJoin { message: String },
    #[error("插件包下载返回 HTTP 状态 {status}")]
    HttpStatus { status: u16 },
    #[error("插件包下载发生未授权重定向：期望 {expected}，实际 {actual}")]
    Redirect { expected: String, actual: String },
    #[error("插件包下载超过 {max} 字节（实际 {bytes} 字节）")]
    TooLarge { bytes: u64, max: u64 },
    #[error("插件包下载目标无效：{path}")]
    DestinationInvalid { path: PathBuf },
    #[error("插件包下载目标已存在：{path}")]
    DestinationExists { path: PathBuf },
    #[error("插件包下载文件操作失败：{source}")]
    Io { source: std::io::Error },
    #[error("插件包下载摘要不匹配：期望 {expected}，实际 {actual}")]
    DigestMismatch { expected: String, actual: String },
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::path::PathBuf;

    use super::*;
    use ramag_domain::{PluginApiVersion, PluginCapability, PluginId, PluginPackageRuntime};

    fn entry() -> PluginPackageCatalogEntry {
        PluginPackageCatalogEntry {
            id: PluginId::new("download.example").expect("valid ID"),
            name: "下载测试插件".into(),
            version: "1.0.0".into(),
            api_version: PluginApiVersion::new(1, 0),
            runtime: PluginPackageRuntime::Process,
            package_url: "https://plugins.example.test/package.tar.gz".into(),
            archive_sha256: "a".repeat(64),
            capabilities: vec![PluginCapability::new("ui.entry")],
        }
    }

    #[test]
    fn destination_validation_rejects_relative_paths() {
        assert!(matches!(
            validate_destination(Path::new("package.tar.gz")),
            Err(PluginPackageDownloadError::DestinationInvalid { .. })
        ));
    }

    #[test]
    fn digest_mismatch_is_reported_without_network_access() {
        let actual = hex::encode(Sha256::digest(b"package"));
        assert_ne!(actual, entry().archive_sha256);
        let error = PluginPackageDownloadError::DigestMismatch {
            expected: entry().archive_sha256,
            actual,
        };
        assert!(error.to_string().contains("摘要不匹配"));
    }

    #[tokio::test]
    #[ignore = "requires the local Docker HTTPS plugin download fixture"]
    async fn downloads_plugin_package_over_local_https_fixture() {
        // 本测试使用自签 fixture 证书验证生产 runtime 入口；生产客户端仍保持严格证书校验。
        let _ = rustls::crypto::ring::default_provider().install_default();
        let root = PathBuf::from(
            std::env::var_os("RAMAG_PLUGIN_DOWNLOAD_TEST_ROOT")
                .expect("test script must set fixture root"),
        );
        let port: u16 = std::env::var("RAMAG_PLUGIN_DOWNLOAD_TEST_PORT")
            .expect("test script must set HTTPS port")
            .parse()
            .expect("HTTPS port must be a number");
        let digest = std::env::var("RAMAG_PLUGIN_DOWNLOAD_TEST_SHA256")
            .expect("test script must set package digest");
        let client = Client::builder()
            .https_only(true)
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(10))
            .resolve(
                "github.com",
                SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port),
            )
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .redirect(redirect::Policy::none())
            .build()
            .expect("build test-only HTTPS client");
        let downloader = PluginPackageDownloader::with_client(client);
        let destination = root.join("downloads/remote-fixture.tar.gz");
        let entry = fixture_entry(
            "https://github.com/plugins/remote-fixture.tar.gz",
            digest.clone(),
        );

        let output = downloader
            .download_to_managed_runtime(&entry, &destination)
            .await
            .expect("download valid plugin package over local HTTPS");
        assert_eq!(output, destination);
        let package_bytes = fs::read(&destination).expect("read downloaded package");
        assert_eq!(hex::encode(Sha256::digest(&package_bytes)), digest);
        assert_no_partial_files(&destination);

        let not_found_destination = root.join("downloads/not-found.tar.gz");
        let not_found = fixture_entry(
            "https://github.com/plugins/package-not-found.tar.gz",
            "0".repeat(64),
        );
        let not_found_result = downloader
            .download_to_managed_runtime(&not_found, &not_found_destination)
            .await;
        assert!(matches!(
            not_found_result,
            Err(PluginPackageDownloadError::HttpStatus { status: 404 })
        ));
        assert!(!not_found_destination.exists());
        assert_no_partial_files(&not_found_destination);

        let redirect_destination = root.join("downloads/redirect.tar.gz");
        let redirect_entry = fixture_entry(
            "https://github.com/plugins/package-redirect.tar.gz",
            digest.clone(),
        );
        let redirect_result = downloader
            .download_to_managed_runtime(&redirect_entry, &redirect_destination)
            .await;
        assert!(matches!(
            redirect_result,
            Err(PluginPackageDownloadError::HttpStatus { status: 302 })
        ));
        assert!(!redirect_destination.exists());
        assert_no_partial_files(&redirect_destination);

        let oversized_destination = root.join("downloads/oversized.tar.gz");
        let oversized_entry = fixture_entry(
            "https://github.com/plugins/package-oversized.tar.gz",
            "0".repeat(64),
        );
        let oversized_result = downloader
            .download_to_managed_runtime(&oversized_entry, &oversized_destination)
            .await;
        assert!(matches!(
            oversized_result,
            Err(PluginPackageDownloadError::TooLarge { .. })
        ));
        assert!(!oversized_destination.exists());
        assert_no_partial_files(&oversized_destination);

        let mismatch_destination = root.join("downloads/mismatch.tar.gz");
        let mismatch_entry = fixture_entry(
            "https://github.com/plugins/remote-fixture.tar.gz",
            "f".repeat(64),
        );
        let mismatch_result = downloader
            .download_to_managed_runtime(&mismatch_entry, &mismatch_destination)
            .await;
        assert!(matches!(
            mismatch_result,
            Err(PluginPackageDownloadError::DigestMismatch { .. })
        ));
        assert!(!mismatch_destination.exists());
        assert_no_partial_files(&mismatch_destination);
        assert_eq!(
            fs::read(&destination).expect("successful package remains available"),
            package_bytes
        );
    }

    fn fixture_entry(package_url: &str, archive_sha256: String) -> PluginPackageCatalogEntry {
        PluginPackageCatalogEntry {
            id: PluginId::new("download.fixture").expect("valid ID"),
            name: "本地下载测试插件".into(),
            version: "1.0.0".into(),
            api_version: PluginApiVersion::new(1, 0),
            runtime: PluginPackageRuntime::Process,
            package_url: package_url.into(),
            archive_sha256,
            capabilities: vec![PluginCapability::new("ui.entry")],
        }
    }

    fn assert_no_partial_files(destination: &Path) {
        let parent = destination.parent().expect("destination parent");
        let prefix = format!(
            ".{}.part-",
            destination
                .file_name()
                .and_then(|name| name.to_str())
                .expect("destination filename")
        );
        assert!(
            !fs::read_dir(parent)
                .expect("read destination parent")
                .map(|entry| entry.expect("read destination entry").file_name())
                .any(|name| name.to_string_lossy().starts_with(&prefix))
        );
    }

    #[test]
    fn managed_runtime_keeps_validation_outside_tokio() {
        // 测试客户端也必须遵守正式组合根的 rustls-no-provider 初始化约定。
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = Client::builder().build().expect("构造测试客户端");
        let downloader = PluginPackageDownloader::with_client(client);
        let result = futures::executor::block_on(
            downloader.download_to_managed_runtime(&entry(), Path::new("relative-package.tar.gz")),
        );
        assert!(matches!(
            result,
            Err(PluginPackageDownloadError::DestinationInvalid { .. })
        ));
    }
}
