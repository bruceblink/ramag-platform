//! 启动器使用的系统动作；这些函数不包含业务状态或工具装配逻辑。

/// 在后台线程中打开应用日志目录，返回系统文件管理器的退出状态。
pub(super) fn open_path_in_file_manager(dir: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(dir);
    let status = cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "系统文件管理器退出状态：{status}"
        )))
    }
}

/// 显示 SSH 主机指纹确认框，并只在用户明确选择 Yes 时允许连接。
pub(super) fn confirm_ssh_host(prompt: &str) -> bool {
    let description = if prompt.trim().is_empty() {
        "OpenSSH 请求确认远程主机指纹。请仅在你确认目标服务器身份后继续。"
    } else {
        prompt
    };
    matches!(
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("确认 SSH 主机指纹")
            .set_description(description)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show(),
        rfd::MessageDialogResult::Yes
    )
}
