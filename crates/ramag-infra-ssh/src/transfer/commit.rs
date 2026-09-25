//! 临时文件路径与本地、远程原子提交。

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::protocol::FileAttributes;
use uuid::Uuid;

use ramag_domain::entities::{
    OverwritePolicy, RemotePath, SshTransferOutcome, infer_sftp_namespace,
};
use ramag_domain::error::{DomainError, Result};

use crate::session::{StructuredSftpSession, map_sftp_error};

#[async_trait]
trait RemoteCommitFileSystem: Send + Sync {
    async fn rename(&self, old: String, new: String) -> std::result::Result<(), SftpError>;
    async fn lstat(&self, path: String) -> std::result::Result<FileAttributes, SftpError>;
    async fn remove(&self, path: String) -> std::result::Result<(), SftpError>;
}

#[async_trait]
impl RemoteCommitFileSystem for StructuredSftpSession {
    async fn rename(&self, old: String, new: String) -> std::result::Result<(), SftpError> {
        self.raw.rename(old, new).await.map(|_| ())
    }

    async fn lstat(&self, path: String) -> std::result::Result<FileAttributes, SftpError> {
        self.raw.lstat(path).await.map(|metadata| metadata.attrs)
    }

    async fn remove(&self, path: String) -> std::result::Result<(), SftpError> {
        self.raw.remove(path).await.map(|_| ())
    }
}

pub(super) fn remote_sibling(target: &str, marker: &str) -> Result<String> {
    let target = RemotePath::parse_with_namespace(target, infer_sftp_namespace(target))
        .map_err(DomainError::InvalidConfig)?;
    target
        .temporary_sibling(marker, &Uuid::new_v4().simple().to_string())
        .map(|path| path.to_string())
        .map_err(DomainError::InvalidConfig)
}

pub(super) fn local_sibling(target: &Path) -> Result<PathBuf> {
    let parent = target
        .parent()
        .ok_or_else(|| DomainError::InvalidConfig("本地下载目标缺少父目录".into()))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| DomainError::InvalidConfig("本地下载目标缺少 UTF-8 文件名".into()))?;
    Ok(parent.join(format!(".{name}.ramag-download-{}.tmp", Uuid::new_v4())))
}

pub(super) async fn commit_remote(
    session: &StructuredSftpSession,
    temporary: &str,
    target: &str,
    target_existed: bool,
) -> Result<SshTransferOutcome> {
    commit_remote_with_file_system(session, temporary, target, target_existed).await
}

/// 提交远程覆盖文件：先保留旧目标，再替换目标，最后尽力清理备份。
///
/// 目标替换成功后，备份删除失败不再伪装成整个传输失败；调用方收到带警告的成功结果，
/// 可以继续使用新文件，同时由 UI 和结构化日志提示残留备份需要后续清理。
async fn commit_remote_with_file_system<F>(
    file_system: &F,
    temporary: &str,
    target: &str,
    target_existed: bool,
) -> Result<SshTransferOutcome>
where
    F: RemoteCommitFileSystem + ?Sized,
{
    if !target_existed {
        return file_system
            .rename(temporary.to_string(), target.to_string())
            .await
            .map(|_| SshTransferOutcome::default())
            .map_err(|error| map_sftp_error("提交远程上传文件", error));
    }
    let backup = remote_sibling(target, "ramag-backup")?;
    file_system
        .rename(target.to_string(), backup.clone())
        .await
        .map_err(|error| map_sftp_error("暂存被覆盖的远程文件", error))?;
    let backup_metadata = file_system.lstat(backup.clone()).await;
    if !matches!(backup_metadata, Ok(ref metadata) if metadata.is_regular()) {
        let rollback = file_system.rename(backup.clone(), target.to_string()).await;
        return match (backup_metadata, rollback) {
            (_, Err(rollback_error)) => Err(DomainError::Other(format!(
                "远程目标类型发生变化且恢复原目标失败：{rollback_error}"
            ))),
            (Err(error), Ok(_)) => Err(map_sftp_error("再次确认远程覆盖目标", error)),
            (Ok(_), Ok(_)) => Err(DomainError::Forbidden(
                "远程覆盖目标已不再是普通文件；已恢复原目标".into(),
            )),
        };
    }
    if let Err(error) = file_system
        .rename(temporary.to_string(), target.to_string())
        .await
    {
        let rollback = file_system.rename(backup.clone(), target.to_string()).await;
        cleanup_remote_with_file_system(file_system, temporary).await;
        return match rollback {
            Ok(_) => Err(map_sftp_error("提交远程上传文件", error)),
            Err(rollback_error) => Err(DomainError::Other(format!(
                "提交远程上传文件失败且恢复原文件也失败：{error}；{rollback_error}"
            ))),
        };
    }
    match file_system.remove(backup.clone()).await {
        Ok(()) => Ok(SshTransferOutcome::default()),
        Err(error) => {
            let warning = format!("远程文件已替换，但清理覆盖备份失败：{error}");
            tracing::warn!(
                operation = "ssh_transfer_commit_cleanup",
                stage = "remote_backup",
                target,
                backup = %backup,
                error = %error,
                "remote replacement committed but backup cleanup failed"
            );
            Ok(SshTransferOutcome {
                warnings: vec![warning],
            })
        }
    }
}

pub(super) async fn cleanup_remote(session: &StructuredSftpSession, path: &str) {
    cleanup_remote_with_file_system(session, path).await;
}

async fn cleanup_remote_with_file_system<F>(file_system: &F, path: &str)
where
    F: RemoteCommitFileSystem + ?Sized,
{
    if let Err(error) = file_system.remove(path.to_string()).await
        && !matches!(
            error,
            russh_sftp::client::error::Error::Status(ref status)
                if status.status_code == russh_sftp::protocol::StatusCode::NoSuchFile
        )
    {
        tracing::warn!(operation = "ssh_transfer_commit_cleanup", stage = "remote", error = %error, "cleanup ssh remote temporary file failed");
    }
}

pub(super) async fn cleanup_local(path: &Path) {
    if let Err(error) = tokio::fs::remove_file(path).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(operation = "ssh_transfer_commit_cleanup", stage = "local", error = %error, "cleanup ssh local temporary file failed");
    }
}

pub(super) async fn commit_local(
    temporary: &Path,
    target: &Path,
    overwrite: OverwritePolicy,
) -> Result<()> {
    let temporary = temporary.to_path_buf();
    let target = target.to_path_buf();
    tokio::task::spawn_blocking(move || commit_local_blocking(&temporary, &target, overwrite))
        .await
        .map_err(|error| DomainError::Other(format!("提交本地下载任务异常退出：{error}")))?
}

pub(super) fn commit_local_blocking(
    temporary: &Path,
    target: &Path,
    overwrite: OverwritePolicy,
) -> Result<()> {
    if overwrite == OverwritePolicy::Refuse {
        std::fs::hard_link(temporary, target).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                DomainError::Forbidden("本地目标已存在；未覆盖原文件".into())
            } else {
                DomainError::Other(format!("原子提交本地下载文件失败：{error}"))
            }
        })?;
        std::fs::remove_file(temporary)
            .map_err(|error| DomainError::Other(format!("清理本地下载临时链接失败：{error}")))?;
        return Ok(());
    }
    replace_local_file(temporary, target)
}

#[cfg(not(target_os = "windows"))]
fn replace_local_file(temporary: &Path, target: &Path) -> Result<()> {
    std::fs::rename(temporary, target)
        .map_err(|error| DomainError::Other(format!("原子替换本地下载文件失败：{error}")))
}

#[cfg(target_os = "windows")]
fn replace_local_file(temporary: &Path, target: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt as _;
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    use windows::core::PCWSTR;

    let source = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = target
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: 两个 UTF-16 缓冲区均以 NUL 结尾，并在调用期间保持有效。
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|error| DomainError::Other(format!("原子替换本地下载文件失败：{error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh_sftp::protocol::StatusCode;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct FakeRemote {
        files: Arc<Mutex<HashMap<String, FileAttributes>>>,
        refuse_backup_remove: bool,
        refuse_target_replace: bool,
    }

    impl FakeRemote {
        fn with_files(refuse_backup_remove: bool, refuse_target_replace: bool) -> Self {
            let mut files = HashMap::new();
            let mut old = FileAttributes::empty();
            old.set_regular(true);
            old.size = Some(3);
            let mut new = FileAttributes::empty();
            new.set_regular(true);
            new.size = Some(7);
            files.insert("/target.txt".into(), old);
            files.insert("/temporary.txt".into(), new);
            Self {
                files: Arc::new(Mutex::new(files)),
                refuse_backup_remove,
                refuse_target_replace,
            }
        }

        fn target_size(&self) -> u64 {
            self.files
                .lock()
                .unwrap()
                .get("/target.txt")
                .and_then(|attributes| attributes.size)
                .unwrap_or_default()
        }

        fn backup_count(&self) -> usize {
            self.files
                .lock()
                .unwrap()
                .keys()
                .filter(|path| path.contains(".target.txt.ramag-backup-"))
                .count()
        }
    }

    #[async_trait]
    impl RemoteCommitFileSystem for FakeRemote {
        async fn rename(&self, old: String, new: String) -> std::result::Result<(), SftpError> {
            if self.refuse_target_replace && old == "/temporary.txt" && new == "/target.txt" {
                return Err(SftpError::Status(russh_sftp::protocol::Status {
                    id: 1,
                    status_code: StatusCode::PermissionDenied,
                    error_message: "target replace refused".into(),
                    language_tag: String::new(),
                }));
            }
            let mut files = self.files.lock().unwrap();
            let file = files.remove(&old).ok_or_else(|| {
                SftpError::Status(russh_sftp::protocol::Status {
                    id: 1,
                    status_code: StatusCode::NoSuchFile,
                    error_message: "missing file".into(),
                    language_tag: String::new(),
                })
            })?;
            files.insert(new, file);
            Ok(())
        }

        async fn lstat(&self, path: String) -> std::result::Result<FileAttributes, SftpError> {
            self.files
                .lock()
                .unwrap()
                .get(&path)
                .cloned()
                .ok_or_else(|| {
                    SftpError::Status(russh_sftp::protocol::Status {
                        id: 1,
                        status_code: StatusCode::NoSuchFile,
                        error_message: "missing file".into(),
                        language_tag: String::new(),
                    })
                })
        }

        async fn remove(&self, path: String) -> std::result::Result<(), SftpError> {
            if self.refuse_backup_remove && path.contains(".ramag-backup-") {
                return Err(SftpError::Status(russh_sftp::protocol::Status {
                    id: 1,
                    status_code: StatusCode::PermissionDenied,
                    error_message: "backup cleanup refused".into(),
                    language_tag: String::new(),
                }));
            }
            self.files.lock().unwrap().remove(&path);
            Ok(())
        }
    }

    #[test]
    fn replacement_success_with_backup_cleanup_failure_is_a_warning() {
        let remote = FakeRemote::with_files(true, false);
        let outcome = futures::executor::block_on(commit_remote_with_file_system(
            &remote,
            "/temporary.txt",
            "/target.txt",
            true,
        ))
        .unwrap();

        assert_eq!(remote.target_size(), 7);
        assert_eq!(remote.backup_count(), 1);
        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].contains("清理覆盖备份失败"));
    }

    #[test]
    fn replacement_failure_rolls_back_old_target() {
        let remote = FakeRemote::with_files(false, true);
        let error = futures::executor::block_on(commit_remote_with_file_system(
            &remote,
            "/temporary.txt",
            "/target.txt",
            true,
        ))
        .unwrap_err();

        assert!(error.message().contains("提交远程上传文件"));
        assert_eq!(remote.target_size(), 3);
        assert_eq!(remote.backup_count(), 0);
    }
}
