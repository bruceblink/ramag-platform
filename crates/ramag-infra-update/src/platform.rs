//! 使用系统程序显示或打开已校验的安装包。

use std::path::Path;
use std::process::{Command, Stdio};

use ramag_domain::error::{DomainError, Result};

pub(super) fn reveal_download(path: &Path) -> Result<()> {
    require_download(path)?;
    let parent = path.parent().ok_or_else(|| {
        DomainError::InvalidConfig(format!("安装包路径缺少父目录：{}", path.display()))
    })?;
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer");
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let mut command = Command::new("xdg-open");
    command.arg(parent).spawn().map_err(|error| {
        DomainError::Other(format!("打开安装包目录失败 {}：{error}", parent.display()))
    })?;
    Ok(())
}

pub(super) fn install_download(path: &Path) -> Result<()> {
    require_download(path)?;
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = Command::new(path);
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let mut command = Command::new("xdg-open");
    #[cfg(not(target_os = "windows"))]
    command.arg(path);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            DomainError::Other(format!("启动安装包失败 {}：{error}", path.display()))
        })?;
    Ok(())
}

fn require_download(path: &Path) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        Err(DomainError::NotFound(format!(
            "已下载安装包不存在：{}",
            path.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::install_download;
    #[cfg(target_os = "windows")]
    use std::path::PathBuf;

    #[test]
    fn install_rejects_a_missing_download() {
        let error = install_download(std::path::Path::new("missing-installer.exe"))
            .expect_err("missing installers must not be launched");
        assert!(error.to_string().contains("已下载安装包不存在"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn install_starts_an_existing_executable() {
        let system_root = std::env::var_os("SystemRoot").expect("Windows system root");
        let where_exe = PathBuf::from(system_root)
            .join("System32")
            .join("where.exe");
        install_download(&where_exe).expect("an existing installer executable should launch");
    }
}
