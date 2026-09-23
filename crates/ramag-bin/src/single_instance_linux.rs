//! Linux 单实例：在用户私有的 XDG runtime 目录监听 Unix socket。
//! 后启进程连接 socket 请求首实例唤起；失效 socket 仅在确认无法连接后清理。

use std::io::{self, Read as _, Write as _};
use std::os::unix::fs::{
    DirBuilderExt as _, FileTypeExt as _, MetadataExt as _, PermissionsExt as _,
};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TrySendError, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use tracing::{info, warn};

const SOCKET_NAME: &str = "ramag.sock";
const ACTIVATE_MESSAGE: &[u8] = b"activate\n";

pub(crate) enum InstanceRole {
    Primary(PrimaryGuard),
    Secondary,
}

pub(crate) struct PrimaryGuard {
    path: Option<PathBuf>,
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    rx: Option<Receiver<()>>,
}

impl PrimaryGuard {
    pub(crate) fn poll_activate(&self) -> bool {
        let Some(rx) = &self.rx else {
            return false;
        };
        let mut fired = false;
        while rx.try_recv().is_ok() {
            fired = true;
        }
        fired
    }

    fn degraded() -> Self {
        Self {
            path: None,
            stopping: Arc::new(AtomicBool::new(false)),
            thread: None,
            rx: None,
        }
    }
}

impl Drop for PrimaryGuard {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(path) = &self.path {
            let _ = UnixStream::connect(path);
        }
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            warn!(
                operation = "single_instance_shutdown",
                "single-instance listener thread panicked"
            );
        }
        if let Some(path) = &self.path
            && let Err(error) = remove_socket(path)
        {
            warn!(operation = "single_instance_shutdown", error = %error, path = %path.display(), "remove single-instance socket failed");
        }
    }
}

pub(crate) fn acquire() -> InstanceRole {
    let Some(path) = runtime_socket_path() else {
        warn!(
            operation = "single_instance_init",
            reason = "runtime_directory_unavailable",
            "no private runtime directory is available; single-instance protection disabled"
        );
        return InstanceRole::Primary(PrimaryGuard::degraded());
    };
    acquire_at(path)
}

fn runtime_socket_path() -> Option<PathBuf> {
    let uid = std::fs::metadata("/proc/self").ok()?.uid();
    let xdg_runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    resolve_runtime_socket_path(xdg_runtime.as_deref(), &std::env::temp_dir(), uid)
}

fn resolve_runtime_socket_path(
    xdg_runtime: Option<&Path>,
    temp_directory: &Path,
    uid: u32,
) -> Option<PathBuf> {
    if let Some(directory) = xdg_runtime {
        match validate_runtime_directory(directory, uid) {
            Ok(()) => return Some(directory.join(SOCKET_NAME)),
            Err(error) => warn!(
                operation = "single_instance_init",
                error = %error,
                path = %directory.display(),
                "XDG_RUNTIME_DIR is not a private user directory; trying a per-user temporary directory"
            ),
        }
    }

    let directory = temp_directory.join(format!("ramag-{uid}"));
    match ensure_private_runtime_directory(&directory, uid) {
        Ok(()) => {
            info!(
                operation = "single_instance_init",
                path = %directory.display(),
                "using a private per-user temporary directory for single-instance coordination"
            );
            Some(directory.join(SOCKET_NAME))
        }
        Err(error) => {
            warn!(
                operation = "single_instance_init",
                error = %error,
                path = %directory.display(),
                "create private per-user runtime directory failed"
            );
            None
        }
    }
}

fn validate_runtime_directory(path: &Path, uid: u32) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    let mode = metadata.mode() & 0o777;
    if !metadata.file_type().is_dir()
        || metadata.uid() != uid
        || mode & 0o077 != 0
        || mode & 0o700 != 0o700
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "runtime directory must be a real directory owned by the current user with mode 0700",
        ));
    }
    Ok(())
}

fn ensure_private_runtime_directory(path: &Path, uid: u32) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime directory path must be absolute",
        ));
    }

    let mut builder = std::fs::DirBuilder::new();
    builder.mode(0o700);
    match builder.create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }

    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir() || metadata.uid() != uid {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "runtime directory is not a real directory owned by the current user",
        ));
    }
    if metadata.mode() & 0o077 != 0 || metadata.mode() & 0o700 != 0o700 {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    validate_runtime_directory(path, uid)
}

fn acquire_at(path: PathBuf) -> InstanceRole {
    match UnixListener::bind(&path) {
        Ok(listener) => start_primary(path, listener),
        Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
            if notify_primary(&path) {
                return InstanceRole::Secondary;
            }
            match remove_stale_socket(&path).and_then(|()| UnixListener::bind(&path)) {
                Ok(listener) => start_primary(path, listener),
                Err(retry_error) => {
                    warn!(
                        operation = "single_instance_init",
                        error = %retry_error,
                        path = %path.display(),
                        "recover stale single-instance socket failed; protection disabled"
                    );
                    InstanceRole::Primary(PrimaryGuard::degraded())
                }
            }
        }
        Err(error) => {
            warn!(operation = "single_instance_init", stage = "bind", error = %error, path = %path.display(), "bind single-instance socket failed; protection disabled");
            InstanceRole::Primary(PrimaryGuard::degraded())
        }
    }
}

fn start_primary(path: PathBuf, listener: UnixListener) -> InstanceRole {
    let stopping = Arc::new(AtomicBool::new(false));
    let thread_stopping = stopping.clone();
    let (tx, rx) = sync_channel::<()>(1);
    let thread = std::thread::Builder::new()
        .name("ramag-single-instance".into())
        .spawn(move || {
            for connection in listener.incoming() {
                if thread_stopping.load(Ordering::Acquire) {
                    break;
                }
                let mut stream = match connection {
                    Ok(stream) => stream,
                    Err(error) => {
                        warn!(operation = "single_instance_listener", stage = "accept", error = %error, "accept single-instance activation failed");
                        break;
                    }
                };
                if let Err(error) = stream.set_read_timeout(Some(Duration::from_secs(1))) {
                    warn!(operation = "single_instance_listener", stage = "timeout", error = %error, "set single-instance socket timeout failed");
                    continue;
                }
                let mut message = [0_u8; ACTIVATE_MESSAGE.len()];
                if stream.read_exact(&mut message).is_ok() && message == ACTIVATE_MESSAGE {
                    match tx.try_send(()) {
                        Ok(()) | Err(TrySendError::Full(())) => {}
                        Err(TrySendError::Disconnected(())) => break,
                    }
                }
            }
        });

    match thread {
        Ok(thread) => InstanceRole::Primary(PrimaryGuard {
            path: Some(path),
            stopping,
            thread: Some(thread),
            rx: Some(rx),
        }),
        Err(error) => {
            warn!(operation = "single_instance_listener", stage = "start", error = %error, "start single-instance listener failed; protection disabled");
            if let Err(remove_error) = remove_socket(&path) {
                warn!(operation = "single_instance_init", stage = "cleanup", error = %remove_error, path = %path.display(), "remove unused single-instance socket failed");
            }
            InstanceRole::Primary(PrimaryGuard::degraded())
        }
    }
}

fn notify_primary(path: &Path) -> bool {
    match UnixStream::connect(path).and_then(|mut stream| stream.write_all(ACTIVATE_MESSAGE)) {
        Ok(()) => {
            info!(
                operation = "single_instance_notify",
                "existing instance notified to reveal its window"
            );
            true
        }
        Err(error) => {
            warn!(operation = "single_instance_notify", error = %error, path = %path.display(), "notify existing instance failed");
            false
        }
    }
}

fn remove_stale_socket(path: &Path) -> io::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_socket() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "single-instance path exists and is not a socket",
        ));
    }
    remove_socket(path)
}

fn remove_socket(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_socket(name: &str) -> io::Result<(PathBuf, PathBuf)> {
        let directory = std::env::temp_dir().join(format!(
            "ramag-single-instance-{}-{name}",
            std::process::id()
        ));
        match std::fs::remove_dir_all(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        std::fs::create_dir(&directory)?;
        Ok((directory.join(SOCKET_NAME), directory))
    }

    #[test]
    fn missing_xdg_runtime_uses_a_private_per_user_directory() -> io::Result<()> {
        let (_, temp_directory) = test_socket("fallback")?;
        let uid = std::fs::metadata("/proc/self")?.uid();

        let socket = resolve_runtime_socket_path(None, &temp_directory, uid)
            .ok_or_else(|| io::Error::other("private fallback directory was unavailable"))?;
        let fallback = temp_directory.join(format!("ramag-{uid}"));
        assert_eq!(socket, fallback.join(SOCKET_NAME));
        validate_runtime_directory(&fallback, uid)?;

        std::fs::remove_dir_all(temp_directory)
    }

    #[test]
    fn unsafe_xdg_runtime_uses_the_private_fallback() -> io::Result<()> {
        let (_, xdg_directory) = test_socket("unsafe-xdg")?;
        let (_, temp_directory) = test_socket("unsafe-fallback")?;
        let uid = std::fs::metadata("/proc/self")?.uid();

        let socket = resolve_runtime_socket_path(Some(&xdg_directory), &temp_directory, uid)
            .ok_or_else(|| io::Error::other("private fallback directory was unavailable"))?;
        assert_eq!(
            socket,
            temp_directory
                .join(format!("ramag-{uid}"))
                .join(SOCKET_NAME)
        );

        std::fs::remove_dir_all(xdg_directory)?;
        std::fs::remove_dir_all(temp_directory)
    }

    #[test]
    fn second_instance_notifies_primary() -> io::Result<()> {
        let (path, directory) = test_socket("notify")?;
        let primary = match acquire_at(path) {
            InstanceRole::Primary(guard) => guard,
            InstanceRole::Secondary => {
                return Err(io::Error::other("first instance was secondary"));
            }
        };
        assert!(matches!(
            acquire_at(directory.join(SOCKET_NAME)),
            InstanceRole::Secondary
        ));

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        let mut activated = false;
        while std::time::Instant::now() < deadline {
            if primary.poll_activate() {
                activated = true;
                break;
            }
            std::thread::yield_now();
        }
        assert!(activated);
        drop(primary);
        std::fs::remove_dir(directory)
    }

    #[test]
    fn stale_socket_is_replaced() -> io::Result<()> {
        let (path, directory) = test_socket("stale")?;
        drop(UnixListener::bind(&path)?);
        let primary = match acquire_at(path) {
            InstanceRole::Primary(guard) => guard,
            InstanceRole::Secondary => return Err(io::Error::other("stale socket looked active")),
        };
        drop(primary);
        std::fs::remove_dir(directory)
    }
}
