//! 远程插件目录的受限获取与本地保存；不自动安装、执行或声明来源可信。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use reqwest::{Client, Response, StatusCode, Url, header, redirect};
use serde_json::to_vec_pretty;
use thiserror::Error;
use tokio::runtime::{Builder, Runtime};

use crate::{
    MAX_PLUGIN_CATALOG_MANIFEST_BYTES, PluginPackageCatalog, PluginPackageCatalogDocument,
    PluginPackageCatalogError, PluginPackageCatalogSignatureError, PluginPackageCatalogTrustPolicy,
};

/// 目录源连接建立的最长等待时间。
pub const PLUGIN_CATALOG_FETCH_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 目录源请求的最长总时间。
pub const PLUGIN_CATALOG_FETCH_TIMEOUT: Duration = Duration::from_secs(20);
const CATALOG_TEMP_FILE_PREFIX: &str = ".catalog.json.tmp";
static CATALOG_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 显式获取受限 HTTPS 插件目录；成功后才返回已校验的候选元数据。
#[derive(Clone)]
pub struct PluginPackageCatalogFetcher {
    client: Client,
}

impl PluginPackageCatalogFetcher {
    /// 创建禁止重定向的 HTTPS 客户端；调用方应在组合根先安装 TLS Provider。
    pub fn new() -> Result<Self, PluginPackageCatalogFetchError> {
        let client = Client::builder()
            .https_only(true)
            .connect_timeout(PLUGIN_CATALOG_FETCH_CONNECT_TIMEOUT)
            .timeout(PLUGIN_CATALOG_FETCH_TIMEOUT)
            .redirect(redirect::Policy::none())
            .user_agent("Ramag/plugin-package-catalog-fetcher")
            .build()
            .map_err(|source| PluginPackageCatalogFetchError::Client { source })?;
        Ok(Self { client })
    }

    /// 使用已配置客户端创建获取器；URL 与响应边界仍由获取器核对。
    pub fn with_client(client: Client) -> Self {
        Self { client }
    }

    /// 获取并校验一个远程目录；失败时不会写入本地文件。
    pub async fn fetch(
        &self,
        source_url: &str,
    ) -> Result<PluginPackageCatalog, PluginPackageCatalogFetchError> {
        let url = validate_source_url(source_url)?;
        tracing::info!(operation = "plugin_catalog_fetch", source = %url, "远程插件目录获取开始");
        let body = self.fetch_body(&url).await?;
        let catalog = PluginPackageCatalog::parse_json(&body)
            .map_err(PluginPackageCatalogFetchError::Catalog)?;
        tracing::info!(
            operation = "plugin_catalog_fetch",
            source = %url,
            count = catalog.entries().len(),
            "远程插件目录获取完成"
        );
        Ok(catalog)
    }

    /// 获取并核验 v1 兼容目录或 v2 signed envelope。
    pub async fn fetch_document(
        &self,
        source_url: &str,
        trust_policy: &PluginPackageCatalogTrustPolicy,
    ) -> Result<PluginPackageCatalogDocument, PluginPackageCatalogFetchError> {
        let url = validate_source_url(source_url)?;
        tracing::info!(operation = "plugin_catalog_fetch", source = %url, "远程插件目录获取开始");
        let body = self.fetch_body(&url).await?;
        let document = PluginPackageCatalogDocument::parse_json(&body, url.as_str(), trust_policy)
            .map_err(PluginPackageCatalogFetchError::Signature)?;
        tracing::info!(
            operation = "plugin_catalog_fetch",
            source = %url,
            count = document.entries().len(),
            verified = document.trust().is_verified(),
            "远程插件目录获取完成"
        );
        Ok(document)
    }

    /// 获取并原子保存目录；解析或保存失败时保留旧目录文件。
    pub async fn fetch_and_store(
        &self,
        source_url: &str,
        store: &PluginPackageCatalogStore,
    ) -> Result<PluginPackageCatalog, PluginPackageCatalogFetchError> {
        let catalog = self.fetch(source_url).await?;
        store
            .save(&catalog)
            .map_err(PluginPackageCatalogFetchError::Store)?;
        Ok(catalog)
    }

    /// 获取并原样保存已核验的目录文档，保留 signed envelope 的签名字段。
    pub async fn fetch_and_store_document(
        &self,
        source_url: &str,
        store: &PluginPackageCatalogStore,
        trust_policy: &PluginPackageCatalogTrustPolicy,
    ) -> Result<PluginPackageCatalogDocument, PluginPackageCatalogFetchError> {
        let document = self.fetch_document(source_url, trust_policy).await?;
        store
            .save_document(&document)
            .map_err(PluginPackageCatalogFetchError::Store)?;
        Ok(document)
    }

    /// 在目录获取器专用 Tokio runtime 中同步执行获取和保存，供 GPUI 后台执行器调用。
    ///
    /// 调用方必须已经位于后台线程；`block_on` 只阻塞该后台线程，不阻塞 UI 线程。
    pub fn fetch_and_store_on_managed_runtime(
        &self,
        source_url: &str,
        store: &PluginPackageCatalogStore,
    ) -> Result<PluginPackageCatalog, PluginPackageCatalogFetchError> {
        let fetcher = self.clone();
        let source_url = source_url.to_owned();
        let store = store.clone();
        catalog_runtime()?.block_on(fetcher.fetch_and_store(&source_url, &store))
    }

    /// 在专用 Tokio runtime 中获取并保存已签名目录文档。
    pub fn fetch_and_store_document_on_managed_runtime(
        &self,
        source_url: &str,
        store: &PluginPackageCatalogStore,
        trust_policy: &PluginPackageCatalogTrustPolicy,
    ) -> Result<PluginPackageCatalogDocument, PluginPackageCatalogFetchError> {
        let fetcher = self.clone();
        let source_url = source_url.to_owned();
        let store = store.clone();
        let trust_policy = trust_policy.clone();
        catalog_runtime()?.block_on(fetcher.fetch_and_store_document(
            &source_url,
            &store,
            &trust_policy,
        ))
    }

    async fn fetch_body(&self, url: &Url) -> Result<Vec<u8>, PluginPackageCatalogFetchError> {
        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|source| PluginPackageCatalogFetchError::Request { source })?;
        validate_response(&response, url)?;
        read_bounded_body(response).await
    }
}

fn catalog_runtime() -> Result<&'static Runtime, PluginPackageCatalogFetchError> {
    static RUNTIME: OnceLock<Result<Runtime, String>> = OnceLock::new();
    match RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("ramag-plugin-catalog-tokio")
            .enable_all()
            .build()
            .map_err(|error| format!("创建插件目录 runtime 失败：{error}"))
    }) {
        Ok(runtime) => Ok(runtime),
        Err(message) => Err(PluginPackageCatalogFetchError::Runtime {
            message: message.clone(),
        }),
    }
}

/// 将已解析目录保存到插件状态目录；原子替换失败时保留旧文件。
#[derive(Debug, Clone)]
pub struct PluginPackageCatalogStore {
    root: PathBuf,
}

impl PluginPackageCatalogStore {
    /// 创建目录存储；根路径必须是绝对路径。
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, PluginPackageCatalogStoreError> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(PluginPackageCatalogStoreError::RelativeRoot { path: root });
        }
        Ok(Self { root })
    }

    /// 返回本地目录清单路径。
    pub fn path(&self) -> PathBuf {
        self.root.join("catalog.json")
    }

    /// 读取并校验本地目录；目录不存在时返回空值。
    pub fn load(&self) -> Result<Option<PluginPackageCatalog>, PluginPackageCatalogStoreError> {
        if !ensure_root(&self.root, false)? {
            return Ok(None);
        }
        let path = self.path();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(io_error("读取插件目录清单", source)),
        };
        let catalog = PluginPackageCatalog::parse_json(&bytes)
            .map_err(PluginPackageCatalogStoreError::Catalog)?;
        Ok(Some(catalog))
    }

    /// 读取并重新核验目录文档；signed envelope 会保留其签名字段。
    pub fn load_document(
        &self,
        source_url: &str,
        trust_policy: &PluginPackageCatalogTrustPolicy,
    ) -> Result<Option<PluginPackageCatalogDocument>, PluginPackageCatalogStoreError> {
        if !ensure_root(&self.root, false)? {
            return Ok(None);
        }
        let path = self.path();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(io_error("读取插件目录清单", source)),
        };
        let document = PluginPackageCatalogDocument::parse_json(&bytes, source_url, trust_policy)
            .map_err(PluginPackageCatalogStoreError::Signature)?;
        Ok(Some(document))
    }

    /// 序列化并原子替换本地目录清单。
    pub fn save(
        &self,
        catalog: &PluginPackageCatalog,
    ) -> Result<(), PluginPackageCatalogStoreError> {
        catalog
            .validate()
            .map_err(PluginPackageCatalogStoreError::Catalog)?;
        let bytes = to_vec_pretty(catalog).map_err(PluginPackageCatalogStoreError::Serialize)?;
        if bytes.len() > MAX_PLUGIN_CATALOG_MANIFEST_BYTES {
            return Err(PluginPackageCatalogStoreError::FileTooLarge {
                bytes: bytes.len(),
                max: MAX_PLUGIN_CATALOG_MANIFEST_BYTES,
            });
        }
        ensure_root(&self.root, true)?;
        write_atomic(&self.path(), &bytes)
    }

    /// 原样保存已经核验的目录文档，避免重序列化破坏 signed envelope。
    pub fn save_document(
        &self,
        document: &PluginPackageCatalogDocument,
    ) -> Result<(), PluginPackageCatalogStoreError> {
        if document.serialized_json().len() > MAX_PLUGIN_CATALOG_MANIFEST_BYTES {
            return Err(PluginPackageCatalogStoreError::FileTooLarge {
                bytes: document.serialized_json().len(),
                max: MAX_PLUGIN_CATALOG_MANIFEST_BYTES,
            });
        }
        ensure_root(&self.root, true)?;
        write_atomic(&self.path(), document.serialized_json())
    }
}

fn validate_source_url(value: &str) -> Result<Url, PluginPackageCatalogFetchError> {
    if value.len() > crate::MAX_PLUGIN_CATALOG_URL_BYTES {
        return Err(invalid_source_url(value));
    }
    let parsed = Url::parse(value).map_err(|_| invalid_source_url(value))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(invalid_source_url(value));
    }
    Ok(parsed)
}

fn invalid_source_url(value: &str) -> PluginPackageCatalogFetchError {
    PluginPackageCatalogFetchError::InvalidSourceUrl {
        url: value.to_owned(),
    }
}

fn validate_response(
    response: &Response,
    expected: &Url,
) -> Result<(), PluginPackageCatalogFetchError> {
    if response.status().is_redirection() {
        return Err(PluginPackageCatalogFetchError::Redirect {
            expected: expected.to_string(),
            location: response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
        });
    }
    if response.url() != expected {
        return Err(PluginPackageCatalogFetchError::Redirect {
            expected: expected.to_string(),
            location: Some(response.url().to_string()),
        });
    }
    if response.status() != StatusCode::OK {
        return Err(PluginPackageCatalogFetchError::HttpStatus {
            status: response.status().as_u16(),
        });
    }
    Ok(())
}

fn validate_content_length(
    content_length: Option<u64>,
) -> Result<(), PluginPackageCatalogFetchError> {
    if let Some(bytes) = content_length
        && bytes > MAX_PLUGIN_CATALOG_MANIFEST_BYTES as u64
    {
        return Err(PluginPackageCatalogFetchError::TooLarge {
            bytes,
            max: MAX_PLUGIN_CATALOG_MANIFEST_BYTES,
        });
    }
    Ok(())
}

async fn read_bounded_body(
    mut response: Response,
) -> Result<Vec<u8>, PluginPackageCatalogFetchError> {
    validate_content_length(response.content_length())?;
    let mut body = Vec::new();
    while let Some(chunk) = tokio::time::timeout(PLUGIN_CATALOG_FETCH_TIMEOUT, response.chunk())
        .await
        .map_err(|_| PluginPackageCatalogFetchError::ResponseTimeout)?
        .map_err(|source| PluginPackageCatalogFetchError::Request { source })?
    {
        let next_size = body.len().saturating_add(chunk.len());
        if next_size > MAX_PLUGIN_CATALOG_MANIFEST_BYTES {
            return Err(PluginPackageCatalogFetchError::TooLarge {
                bytes: next_size as u64,
                max: MAX_PLUGIN_CATALOG_MANIFEST_BYTES,
            });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn ensure_root(path: &Path, create: bool) -> Result<bool, PluginPackageCatalogStoreError> {
    if !path.is_absolute() {
        return Err(PluginPackageCatalogStoreError::RelativeRoot {
            path: path.to_path_buf(),
        });
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|source| io_error("创建插件目录状态目录", source))?;
            fs::symlink_metadata(path).map_err(|source| io_error("检查插件目录状态目录", source))?
        }
        Err(source) => return Err(io_error("检查插件目录状态目录", source)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PluginPackageCatalogStoreError::InvalidRoot {
            path: path.to_path_buf(),
        });
    }
    Ok(true)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), PluginPackageCatalogStoreError> {
    let temp_path = path.with_file_name(format!(
        "{CATALOG_TEMP_FILE_PREFIX}-{}-{}",
        std::process::id(),
        CATALOG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|source| io_error("创建插件目录临时文件", source))?;
        file.write_all(bytes)
            .map_err(|source| io_error("写入插件目录临时文件", source))?;
        file.flush()
            .map_err(|source| io_error("刷新插件目录临时文件", source))?;
        file.sync_all()
            .map_err(|source| io_error("同步插件目录临时文件", source))?;
        drop(file);
        commit_temp(&temp_path, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

#[cfg(not(target_os = "windows"))]
fn commit_temp(temp_path: &Path, target: &Path) -> Result<(), PluginPackageCatalogStoreError> {
    fs::rename(temp_path, target).map_err(|source| io_error("替换插件目录清单", source))
}

#[cfg(target_os = "windows")]
fn commit_temp(temp_path: &Path, target: &Path) -> Result<(), PluginPackageCatalogStoreError> {
    let exists = match fs::symlink_metadata(target) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(source) => return Err(io_error("检查插件目录清单", source)),
    };
    if !exists {
        return fs::rename(temp_path, target)
            .map_err(|source| io_error("保存插件目录清单", source));
    }
    let backup = target.with_file_name(format!(
        ".catalog.json.backup-{}-{}",
        std::process::id(),
        CATALOG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::rename(target, &backup).map_err(|source| io_error("备份插件目录清单", source))?;
    match fs::rename(temp_path, target) {
        Ok(()) => {
            if let Err(error) = fs::remove_file(&backup) {
                tracing::warn!(
                    operation = "plugin_catalog_store_cleanup",
                    path = %backup.display(),
                    error = %error,
                    "插件目录旧文件清理失败"
                );
            }
            Ok(())
        }
        Err(source) => {
            let _ = fs::rename(&backup, target);
            Err(io_error("替换插件目录清单", source))
        }
    }
}

fn io_error(operation: &'static str, source: std::io::Error) -> PluginPackageCatalogStoreError {
    PluginPackageCatalogStoreError::Io { operation, source }
}

#[derive(Debug, Error)]
pub enum PluginPackageCatalogFetchError {
    #[error("插件目录源地址无效：{url}")]
    InvalidSourceUrl { url: String },
    #[error("创建插件目录获取客户端失败：{source}")]
    Client { source: reqwest::Error },
    #[error("插件目录获取请求失败：{source}")]
    Request { source: reqwest::Error },
    #[error("插件目录获取返回 HTTP 状态 {status}")]
    HttpStatus { status: u16 },
    #[error("插件目录获取禁止重定向：期望 {expected}，位置 {location:?}")]
    Redirect {
        expected: String,
        location: Option<String>,
    },
    #[error("插件目录响应超过 {max} bytes（实际 {bytes} bytes）")]
    TooLarge { bytes: u64, max: usize },
    #[error("插件目录响应读取超时")]
    ResponseTimeout,
    #[error("插件目录后台 runtime 初始化失败：{message}")]
    Runtime { message: String },
    #[error("插件目录清单校验失败：{0}")]
    Catalog(PluginPackageCatalogError),
    #[error("插件目录签名校验失败：{0}")]
    Signature(PluginPackageCatalogSignatureError),
    #[error("插件目录保存失败：{0}")]
    Store(PluginPackageCatalogStoreError),
}

#[derive(Debug, Error)]
pub enum PluginPackageCatalogStoreError {
    #[error("插件目录存储根路径必须是绝对路径：{path}")]
    RelativeRoot { path: PathBuf },
    #[error("插件目录存储根路径必须是普通目录：{path}")]
    InvalidRoot { path: PathBuf },
    #[error("插件目录存储操作失败（{operation}）：{source}")]
    Io {
        operation: &'static str,
        source: std::io::Error,
    },
    #[error("插件目录清单 JSON 无法序列化：{0}")]
    Serialize(serde_json::Error),
    #[error("插件目录清单超过 {max} 字节（实际 {bytes} 字节）")]
    FileTooLarge { bytes: usize, max: usize },
    #[error("插件目录清单校验失败：{0}")]
    Catalog(PluginPackageCatalogError),
    #[error("插件目录签名校验失败：{0}")]
    Signature(PluginPackageCatalogSignatureError),
}

#[cfg(test)]
#[path = "plugin_package_catalog_fetcher_tests.rs"]
mod tests;
