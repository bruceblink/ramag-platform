//! Windows 上通过系统 OpenSSH 的标准输入输出访问 WSL Docker Engine。
//!
//! Bollard 的内置 SSH 适配器依赖 Unix 专用的 `openssh` crate。Windows 目标改用系统
//! `ssh.exe`，远端只执行 `docker system dial-stdio`，Docker HTTP 请求仍由 Bollard 编解码。

use std::{
    io,
    pin::Pin,
    process::Stdio,
    task::{Context, Poll},
};

use bollard::{BollardRequest, Docker, errors::Error as BollardError};
use hyper::{Response, body::Incoming, client::conn::http1};
use hyper_util::rt::TokioIo;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    process::{Child, ChildStdin, ChildStdout, Command},
};
use url::Url;

pub(super) fn connect(address: &str) -> Result<Docker, BollardError> {
    parse_target(address).map_err(invalid_target)?;
    let address = address.to_owned();
    let closure_address = address.clone();
    Ok(Docker::connect_with_custom_transport(
        move |request: BollardRequest| {
            let address = closure_address.clone();
            Box::pin(async move { request_over_ssh(&address, request).await })
        },
        Some(address),
        120,
        bollard::API_DEFAULT_VERSION,
    )?)
}

async fn request_over_ssh(
    address: &str,
    request: BollardRequest,
) -> Result<Response<Incoming>, BollardError> {
    let target = parse_target(address).map_err(invalid_target)?;
    let mut command = Command::new("ssh");
    command
        .arg("-T")
        .arg("-o")
        .arg("BatchMode=yes")
        .args(target.ssh_options())
        .arg(target.authority())
        .arg("docker")
        .arg("system")
        .arg("dial-stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(BollardError::from)?;
    let stdin = child.stdin.take().ok_or_else(|| {
        io::Error::new(io::ErrorKind::BrokenPipe, "Windows OpenSSH 标准输入不可用")
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        io::Error::new(io::ErrorKind::BrokenPipe, "Windows OpenSSH 标准输出不可用")
    })?;
    let io = ChildIo {
        child,
        stdin,
        stdout,
    };
    let (mut sender, connection) = http1::handshake(TokioIo::new(io)).await?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    sender
        .send_request(request)
        .await
        .map_err(BollardError::from)
}

struct ChildIo {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl Drop for ChildIo {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

impl AsyncRead for ChildIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buffer)
    }
}

impl AsyncWrite for ChildIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stdin).poll_write(cx, bytes)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stdin).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stdin).poll_shutdown(cx)
    }
}

#[derive(Debug)]
struct SshTarget {
    user: Option<String>,
    host: String,
    port: Option<u16>,
}

impl SshTarget {
    fn authority(&self) -> String {
        match &self.user {
            Some(user) => format!("{user}@{}", self.host_for_ssh()),
            None => self.host_for_ssh(),
        }
    }

    fn host_for_ssh(&self) -> String {
        if self.host.contains(':') && !self.host.starts_with('[') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        }
    }

    fn ssh_options(&self) -> Vec<String> {
        self.port
            .map(|port| vec!["-p".into(), port.to_string()])
            .unwrap_or_default()
    }
}

fn parse_target(address: &str) -> Result<SshTarget, &'static str> {
    let url = Url::parse(address).map_err(|_| "Docker SSH 地址格式无效")?;
    if url.scheme() != "ssh"
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err("Docker SSH 地址只允许 ssh://用户@主机[:端口]");
    }
    let host = url
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or("Docker SSH 地址缺少主机")?
        .to_owned();
    let user = (!url.username().is_empty()).then(|| url.username().to_owned());
    if user.as_deref().is_some_and(|value| {
        value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    }) {
        return Err("Docker SSH 用户名不能包含空白或控制字符");
    }
    Ok(SshTarget {
        user,
        host,
        port: url.port(),
    })
}

fn invalid_target(message: &'static str) -> BollardError {
    BollardError::from(io::Error::new(io::ErrorKind::InvalidInput, message))
}

#[cfg(test)]
mod tests {
    use super::parse_target;

    #[test]
    fn parses_wsl_ssh_target_without_credentials() {
        let target = parse_target("ssh://likanug@localhost:2222").expect("SSH 地址应有效");
        assert_eq!(target.authority(), "likanug@localhost");
        assert_eq!(target.ssh_options(), ["-p", "2222"]);
    }

    #[test]
    fn rejects_password_and_query_in_ssh_target() {
        assert!(parse_target("ssh://user:secret@localhost").is_err());
        assert!(parse_target("ssh://user@localhost?command=docker").is_err());
    }
}
