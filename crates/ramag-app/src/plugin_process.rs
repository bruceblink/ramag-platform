//! 进程外插件宿主；直接启动包内入口并通过协议层通信，不经过 Shell。

use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex as ParkingMutex;
use ramag_domain::traits::CURRENT_PLUGIN_API_VERSION;
use ramag_domain::{
    PluginApiVersion, PluginCapability, PluginPackageManifest, PluginPackageManifestError,
};
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::plugin_process_protocol::{
    CURRENT_PLUGIN_PROTOCOL_VERSION, PluginHello, PluginInvoke, PluginProcessMessage,
    PluginResponse, PluginShutdown, read_frame, write_frame,
};

/// 进程外插件宿主的默认握手、调用和关闭超时。
pub const DEFAULT_PLUGIN_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_PLUGIN_INVOKE_TIMEOUT: Duration = Duration::from_secs(60);
pub const DEFAULT_PLUGIN_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
const PLUGIN_STDERR_TAIL_BYTES: usize = 16 * 1024;

/// 启动一个已校验插件包所需的运行参数。
#[derive(Debug, Clone)]
pub struct PluginProcessConfig {
    pub package_root: PathBuf,
    pub manifest: PluginPackageManifest,
    pub args: Vec<String>,
    pub granted_capabilities: Vec<PluginCapability>,
    pub handshake_timeout: Duration,
    pub invoke_timeout: Duration,
    pub shutdown_timeout: Duration,
}

impl PluginProcessConfig {
    pub fn new(package_root: impl Into<PathBuf>, manifest: PluginPackageManifest) -> Self {
        Self {
            package_root: package_root.into(),
            manifest,
            args: Vec::new(),
            granted_capabilities: Vec::new(),
            handshake_timeout: DEFAULT_PLUGIN_HANDSHAKE_TIMEOUT,
            invoke_timeout: DEFAULT_PLUGIN_INVOKE_TIMEOUT,
            shutdown_timeout: DEFAULT_PLUGIN_SHUTDOWN_TIMEOUT,
        }
    }

    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_granted_capabilities<I>(mut self, capabilities: I) -> Self
    where
        I: IntoIterator<Item = PluginCapability>,
    {
        self.granted_capabilities = capabilities.into_iter().collect();
        self
    }
}

/// 已完成握手的插件进程会话；同一会话内的调用按顺序处理。
pub struct PluginProcessHost {
    manifest: PluginPackageManifest,
    session: Mutex<Option<PluginProcessSession>>,
    invoke_timeout: Duration,
    shutdown_timeout: Duration,
    stderr_tail: Arc<ParkingMutex<VecDeque<u8>>>,
}

struct PluginProcessSession {
    child: Child,
    writer: ChildStdin,
    reader: ChildStdout,
    stderr_task: JoinHandle<()>,
    next_request_id: u64,
    #[cfg(unix)]
    process_group: i32,
    #[cfg(windows)]
    job: PluginJob,
}

/// Windows 插件进程树的生命周期边界。
///
/// Job Object 在入口进程启动后立即接管该进程。关闭句柄会终止仍属于该
/// Job 的所有进程，因此协议错误、超时和宿主关闭都不会只留下子孙进程。
#[cfg(windows)]
struct PluginJob {
    handle: windows::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl PluginJob {
    fn attach(child: &Child) -> Result<Self, PluginProcessError> {
        let raw_handle = child
            .raw_handle()
            .ok_or_else(|| PluginProcessError::JobObject {
                stage: "process_handle",
                message: "插件进程已退出，无法获取进程句柄".into(),
            })?;
        let process_handle = windows::Win32::Foundation::HANDLE(raw_handle as isize);
        Self::attach_process(process_handle)
    }

    fn attach_process(
        process_handle: windows::Win32::Foundation::HANDLE,
    ) -> Result<Self, PluginProcessError> {
        use windows::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };
        use windows::core::PCWSTR;

        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(|error| {
            PluginProcessError::JobObject {
                stage: "create",
                message: error.to_string(),
            }
        })?;
        let job = Self { handle };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
        unsafe {
            SetInformationJobObject(
                job.handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size,
            )
        }
        .map_err(|error| PluginProcessError::JobObject {
            stage: "configure",
            message: error.to_string(),
        })?;
        unsafe { AssignProcessToJobObject(job.handle, process_handle) }.map_err(|error| {
            PluginProcessError::JobObject {
                stage: "assign",
                message: error.to_string(),
            }
        })?;
        Ok(job)
    }

    /// 终止 Job 中仍在运行的入口和子孙进程；进程已退出时忽略系统返回的失败。
    fn terminate(&self) {
        use windows::Win32::System::JobObjects::TerminateJobObject;
        let _ = unsafe { TerminateJobObject(self.handle, 1) };
    }
}

#[cfg(windows)]
impl Drop for PluginJob {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        let _ = unsafe { CloseHandle(self.handle) };
    }
}

impl PluginProcessHost {
    /// 校验包内入口、直接启动进程并完成协议握手。
    pub async fn start(config: PluginProcessConfig) -> Result<Self, PluginProcessError> {
        config
            .manifest
            .validate()
            .map_err(PluginProcessError::Manifest)?;
        validate_granted_capabilities(&config.granted_capabilities)?;
        let package_root = canonical_package_root(&config.package_root)?;
        verify_package_digest(&package_root, &config.manifest)?;
        let entrypoint = resolve_plugin_entrypoint(&package_root, &config.manifest)?;

        let mut command = Command::new(&entrypoint);
        command.args(&config.args).current_dir(&package_root);
        #[cfg(unix)]
        // 每个插件使用独立进程组；终止会话时可以一起结束入口创建的子进程。
        command.process_group(0);
        configure_plugin_environment(&mut command, &config.manifest, &package_root);
        command
            .env("RAMAG_PLUGIN_ID", config.manifest.id.as_str())
            .env(
                "RAMAG_PLUGIN_PROTOCOL_VERSION",
                CURRENT_PLUGIN_PROTOCOL_VERSION.to_string(),
            )
            .env("RAMAG_PLUGIN_PACKAGE_ROOT", &package_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(PluginProcessError::Spawn)?;
        #[cfg(windows)]
        let job = match PluginJob::attach(&child) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(error);
            }
        };
        #[cfg(unix)]
        let process_group = child
            .id()
            .and_then(|id| i32::try_from(id).ok())
            .unwrap_or_default();
        let writer = child
            .stdin
            .take()
            .ok_or(PluginProcessError::MissingPipe("stdin"))?;
        let reader = child
            .stdout
            .take()
            .ok_or(PluginProcessError::MissingPipe("stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(PluginProcessError::MissingPipe("stderr"))?;
        let stderr_tail = Arc::new(ParkingMutex::new(VecDeque::with_capacity(
            PLUGIN_STDERR_TAIL_BYTES,
        )));
        let stderr_task = tokio::spawn(drain_stderr(stderr, stderr_tail.clone()));
        let mut session = PluginProcessSession {
            child,
            writer,
            reader,
            stderr_task,
            next_request_id: 1,
            #[cfg(unix)]
            process_group,
            #[cfg(windows)]
            job,
        };

        let hello = PluginProcessMessage::Hello(PluginHello {
            protocol_version: CURRENT_PLUGIN_PROTOCOL_VERSION,
            plugin_id: config.manifest.id.clone(),
            api_version: config.manifest.api_version,
            granted_capabilities: config.granted_capabilities.clone(),
            max_frame_bytes: crate::MAX_PLUGIN_FRAME_BYTES,
        });
        let handshake = async {
            write_frame(&mut session.writer, &hello).await?;
            read_frame(&mut session.reader).await
        };
        let ready = match timeout(config.handshake_timeout, handshake).await {
            Ok(Ok(message)) => message,
            Ok(Err(error)) => {
                terminate_session(session, config.shutdown_timeout).await;
                return Err(PluginProcessError::Protocol(error));
            }
            Err(_) => {
                terminate_session(session, config.shutdown_timeout).await;
                return Err(PluginProcessError::HandshakeTimeout);
            }
        };
        if let Err(error) = validate_ready(&config.manifest, ready) {
            terminate_session(session, config.shutdown_timeout).await;
            return Err(error);
        }

        tracing::info!(
            operation = "plugin_process_start",
            plugin_id = %config.manifest.id,
            entrypoint = %entrypoint.display(),
            "external plugin process connected"
        );
        Ok(Self {
            manifest: config.manifest,
            session: Mutex::new(Some(session)),
            invoke_timeout: config.invoke_timeout,
            shutdown_timeout: config.shutdown_timeout,
            stderr_tail,
        })
    }

    pub fn manifest(&self) -> &PluginPackageManifest {
        &self.manifest
    }

    /// 提交一个入口调用；调用失败或超时后会结束当前会话，避免继续使用未知状态的进程。
    pub async fn invoke(
        &self,
        entry_id: impl Into<String>,
        input: Vec<u8>,
    ) -> Result<Vec<u8>, PluginProcessError> {
        let entry_id = entry_id.into();
        let mut session_guard = self.session.lock().await;
        let session = session_guard.as_mut().ok_or(PluginProcessError::Closed)?;
        let request_id = session.next_request_id;
        session.next_request_id = session.next_request_id.saturating_add(1);
        let invoke = PluginProcessMessage::Invoke(PluginInvoke {
            request_id,
            entry_id,
            input,
        });
        let result = timeout(self.invoke_timeout, async {
            write_frame(&mut session.writer, &invoke).await?;
            read_frame(&mut session.reader).await
        })
        .await;
        let message = match result {
            Ok(Ok(message)) => message,
            Ok(Err(error)) => {
                if let Some(session) = session_guard.take() {
                    terminate_session(session, self.shutdown_timeout).await;
                }
                return Err(PluginProcessError::Protocol(error));
            }
            Err(_) => {
                if let Some(session) = session_guard.take() {
                    terminate_session(session, self.shutdown_timeout).await;
                }
                return Err(PluginProcessError::InvokeTimeout);
            }
        };
        let PluginProcessMessage::Response(response) = message else {
            if let Some(session) = session_guard.take() {
                terminate_session(session, self.shutdown_timeout).await;
            }
            return Err(PluginProcessError::UnexpectedMessage);
        };
        if response.request_id != request_id {
            if let Some(session) = session_guard.take() {
                terminate_session(session, self.shutdown_timeout).await;
            }
            return Err(PluginProcessError::RequestMismatch {
                expected: request_id,
                actual: response.request_id,
            });
        }
        response_result(response)
    }

    /// 发送关闭请求并在超时后终止进程；关闭后不能再次调用。
    pub async fn shutdown(&self) -> Result<(), PluginProcessError> {
        let mut session_guard = self.session.lock().await;
        let Some(mut session) = session_guard.take() else {
            return Ok(());
        };
        let request_id = session.next_request_id;
        let shutdown = PluginProcessMessage::Shutdown(PluginShutdown { request_id });
        let send_result = timeout(
            self.shutdown_timeout,
            write_frame(&mut session.writer, &shutdown),
        )
        .await;
        match send_result {
            Err(_) => {
                terminate_session(session, self.shutdown_timeout).await;
                Err(PluginProcessError::ShutdownTimeout)
            }
            Ok(Err(error)) => {
                terminate_session(session, self.shutdown_timeout).await;
                Err(PluginProcessError::Protocol(error))
            }
            Ok(Ok(())) => wait_for_child(session, self.shutdown_timeout).await,
        }
    }

    /// 返回最近的有界 stderr 尾部，供诊断界面和日志使用。
    pub fn stderr_tail(&self) -> String {
        let tail = self.stderr_tail.lock().iter().copied().collect::<Vec<_>>();
        String::from_utf8_lossy(&tail).into_owned()
    }
}

/// 启动外部代码前重新核对包内容，避免安装后文件被替换仍按 enabled 状态执行。
fn verify_package_digest(
    package_root: &Path,
    manifest: &PluginPackageManifest,
) -> Result<(), PluginProcessError> {
    let actual =
        crate::plugin_package_installer::package_digest(package_root).map_err(|error| {
            PluginProcessError::PackageVerification {
                message: error.to_string(),
            }
        })?;
    if actual != manifest.package_sha256 {
        return Err(PluginProcessError::PackageDigestMismatch {
            expected: manifest.package_sha256.clone(),
            actual,
        });
    }
    Ok(())
}

/// 清空继承环境，只保留启动插件所需的搜索路径和宿主元数据。
fn configure_plugin_environment(
    command: &mut Command,
    _manifest: &PluginPackageManifest,
    _package_root: &Path,
) {
    let path = std::env::var_os("PATH");
    command.env_clear();
    if let Some(path) = path {
        command.env("PATH", path);
    }
    #[cfg(target_os = "windows")]
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system_root);
    }
}

/// 解析并验证插件包内入口，拒绝绝对路径、父目录和符号链接逃逸。
pub fn resolve_plugin_entrypoint(
    package_root: &Path,
    manifest: &PluginPackageManifest,
) -> Result<PathBuf, PluginProcessError> {
    let root = canonical_package_root(package_root)?;
    let relative = Path::new(&manifest.entrypoint);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(PluginProcessError::InvalidEntrypoint {
            entrypoint: manifest.entrypoint.clone(),
        });
    }
    let mut cursor = root.clone();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(PluginProcessError::InvalidEntrypoint {
                entrypoint: manifest.entrypoint.clone(),
            });
        };
        cursor.push(segment);
        let metadata = std::fs::symlink_metadata(&cursor)
            .map_err(|source| PluginProcessError::Path { source })?;
        if metadata.file_type().is_symlink() {
            return Err(PluginProcessError::SymlinkEntrypoint { path: cursor });
        }
    }
    let entrypoint = cursor
        .canonicalize()
        .map_err(|source| PluginProcessError::Path { source })?;
    if !entrypoint.starts_with(&root) {
        return Err(PluginProcessError::EntrypointEscape { path: entrypoint });
    }
    if !entrypoint.is_file() {
        return Err(PluginProcessError::EntrypointNotFile { path: entrypoint });
    }
    Ok(entrypoint)
}

fn canonical_package_root(package_root: &Path) -> Result<PathBuf, PluginProcessError> {
    if !package_root.is_absolute() {
        return Err(PluginProcessError::PackageRootNotAbsolute {
            path: package_root.to_path_buf(),
        });
    }
    let metadata = std::fs::symlink_metadata(package_root)
        .map_err(|source| PluginProcessError::Path { source })?;
    if metadata.file_type().is_symlink() {
        return Err(PluginProcessError::PackageRootSymlink {
            path: package_root.to_path_buf(),
        });
    }
    let root = package_root
        .canonicalize()
        .map_err(|source| PluginProcessError::Path { source })?;
    if !root.is_dir() {
        return Err(PluginProcessError::PackageRootNotDirectory { path: root });
    }
    Ok(root)
}

fn validate_granted_capabilities(
    capabilities: &[PluginCapability],
) -> Result<(), PluginProcessError> {
    let mut seen = HashSet::with_capacity(capabilities.len());
    for capability in capabilities {
        if !capability.is_known() {
            return Err(PluginProcessError::UnknownCapability {
                capability: capability.as_str().to_owned(),
            });
        }
        if !seen.insert(capability) {
            return Err(PluginProcessError::DuplicateCapability {
                capability: capability.as_str().to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_ready(
    manifest: &PluginPackageManifest,
    message: PluginProcessMessage,
) -> Result<(), PluginProcessError> {
    let PluginProcessMessage::Ready(ready) = message else {
        return Err(PluginProcessError::UnexpectedMessage);
    };
    if ready.plugin_id != manifest.id {
        return Err(PluginProcessError::HandshakeIdentity {
            expected: manifest.id.to_string(),
            actual: ready.plugin_id.to_string(),
        });
    }
    if ready.api_version.major != manifest.api_version.major
        || !ready
            .api_version
            .is_supported_by(CURRENT_PLUGIN_API_VERSION)
    {
        return Err(PluginProcessError::HandshakeApi {
            expected: manifest.api_version,
            actual: ready.api_version,
        });
    }
    let declared = manifest
        .capabilities
        .iter()
        .map(|capability| capability.as_str())
        .collect::<HashSet<_>>();
    for capability in &ready.capabilities {
        if !declared.contains(&capability.as_str()) {
            return Err(PluginProcessError::HandshakeCapability {
                capability: capability.as_str().to_owned(),
            });
        }
    }
    Ok(())
}

fn response_result(response: PluginResponse) -> Result<Vec<u8>, PluginProcessError> {
    if response.ok {
        Ok(response.output)
    } else {
        Err(PluginProcessError::Remote {
            message: response
                .error
                .unwrap_or_else(|| "插件返回未说明的失败".into()),
        })
    }
}

async fn drain_stderr(mut stderr: ChildStderr, tail: Arc<ParkingMutex<VecDeque<u8>>>) {
    let mut buffer = [0_u8; 4096];
    loop {
        match stderr.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(bytes) => {
                let mut guard = tail.lock();
                for byte in &buffer[..bytes] {
                    if guard.len() >= PLUGIN_STDERR_TAIL_BYTES {
                        guard.pop_front();
                    }
                    guard.push_back(*byte);
                }
            }
        }
    }
}

async fn terminate_session(mut session: PluginProcessSession, wait_timeout: Duration) {
    #[cfg(unix)]
    kill_process_group(session.process_group);
    #[cfg(windows)]
    session.job.terminate();
    let _ = session.child.start_kill();
    let _ = timeout(wait_timeout, session.child.wait()).await;
    session.stderr_task.abort();
}

async fn wait_for_child(
    mut session: PluginProcessSession,
    wait_timeout: Duration,
) -> Result<(), PluginProcessError> {
    match timeout(wait_timeout, session.child.wait()).await {
        Ok(Ok(_)) => {
            session.stderr_task.abort();
            Ok(())
        }
        Ok(Err(source)) => {
            session.stderr_task.abort();
            Err(PluginProcessError::Wait { source })
        }
        Err(_) => {
            #[cfg(unix)]
            kill_process_group(session.process_group);
            #[cfg(windows)]
            session.job.terminate();
            let _ = session.child.start_kill();
            let _ = timeout(wait_timeout, session.child.wait()).await;
            session.stderr_task.abort();
            Err(PluginProcessError::ShutdownTimeout)
        }
    }
}

/// 终止指定 Unix 进程组；进程组不存在时视为已完成清理。
#[cfg(unix)]
fn kill_process_group(process_group: i32) -> bool {
    if process_group <= 0 {
        return false;
    }
    // 负 PID 表示向整个进程组发送信号；入口子进程不会在宿主退出后继续运行。
    let result = unsafe { libc::kill(-process_group, libc::SIGKILL) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
}

#[derive(Debug, Error)]
pub enum PluginProcessError {
    #[error("插件清单无效：{0}")]
    Manifest(#[source] PluginPackageManifestError),
    #[error("插件入口路径无效：{entrypoint}")]
    InvalidEntrypoint { entrypoint: String },
    #[error("插件包根目录不是绝对路径：{path}")]
    PackageRootNotAbsolute { path: PathBuf },
    #[error("插件包根目录是符号链接：{path}")]
    PackageRootSymlink { path: PathBuf },
    #[error("插件包根目录不是目录：{path}")]
    PackageRootNotDirectory { path: PathBuf },
    #[error("插件入口包含符号链接：{path}")]
    SymlinkEntrypoint { path: PathBuf },
    #[error("插件入口逃逸包根目录：{path}")]
    EntrypointEscape { path: PathBuf },
    #[error("插件入口不是普通文件：{path}")]
    EntrypointNotFile { path: PathBuf },
    #[error("插件路径处理失败：{source}")]
    Path { source: std::io::Error },
    #[error("插件包摘要核验失败：{message}")]
    PackageVerification { message: String },
    #[error("插件包摘要不匹配：期望 {expected}，实际 {actual}")]
    PackageDigestMismatch { expected: String, actual: String },
    #[error("启动插件进程失败：{0}")]
    Spawn(#[source] std::io::Error),
    #[cfg(windows)]
    #[error("配置 Windows 插件 Job Object 失败（{stage}）：{message}")]
    JobObject {
        stage: &'static str,
        message: String,
    },
    #[error("插件进程没有可用的 {0} 管道")]
    MissingPipe(&'static str),
    #[error("插件协议失败：{0}")]
    Protocol(#[source] crate::PluginProcessProtocolError),
    #[error("插件握手超时")]
    HandshakeTimeout,
    #[error("插件调用超时")]
    InvokeTimeout,
    #[error("插件进程已关闭")]
    Closed,
    #[error("插件返回了意外消息类型")]
    UnexpectedMessage,
    #[error("插件响应请求 ID 不匹配：期望 {expected}，实际 {actual}")]
    RequestMismatch { expected: u64, actual: u64 },
    #[error("插件返回失败：{message}")]
    Remote { message: String },
    #[error("插件关闭超时")]
    ShutdownTimeout,
    #[error("等待插件进程失败：{source}")]
    Wait { source: std::io::Error },
    #[error("授予了未知插件能力：{capability}")]
    UnknownCapability { capability: String },
    #[error("重复授予插件能力：{capability}")]
    DuplicateCapability { capability: String },
    #[error("插件握手身份不匹配：期望 {expected}，实际 {actual}")]
    HandshakeIdentity { expected: String, actual: String },
    #[error("插件握手 API 不匹配：期望 {expected}，实际 {actual}")]
    HandshakeApi {
        expected: PluginApiVersion,
        actual: PluginApiVersion,
    },
    #[error("插件握手声明了未在清单中声明的能力：{capability}")]
    HandshakeCapability { capability: String },
}

#[cfg(test)]
#[path = "plugin_process_tests.rs"]
mod tests;
