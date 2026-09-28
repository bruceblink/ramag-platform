//! 真实 OpenSSH/SFTP 集成测试。
//!
//! 未配置 `RAMAG_TEST_SSH_HOST` 时跳过；测试目录必须由
//! `RAMAG_TEST_SSH_ROOT` 指向名称含 `ramag` 的专用绝对目录。

use std::net::TcpListener;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_compression::futures::bufread::GzipDecoder;
use futures::io::{AsyncReadExt as _, BufReader};
use futures::stream::StreamExt as _;
use ramag_domain::entities::{
    OverwritePolicy, RemoteEntryKind, RemoteFileChunkPosition, SshAuthMode, SshPortForward,
    SshProfile, SshProgressFn, TransferCancellation, join_remote_path, validate_remote_path,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::SshDriver;
use ramag_infra_ssh::OpenSshDriver;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::{sleep, timeout};

struct Fixture {
    profile: SshProfile,
    root: String,
}

fn fixture_from_env() -> Result<Option<Fixture>> {
    let Some(host) = std::env::var("RAMAG_TEST_SSH_HOST").ok() else {
        return Ok(None);
    };
    let root = std::env::var("RAMAG_TEST_SSH_ROOT").map_err(|_| {
        DomainError::InvalidConfig(
            "设置 RAMAG_TEST_SSH_HOST 时也必须设置 RAMAG_TEST_SSH_ROOT".into(),
        )
    })?;
    validate_remote_path(&root).map_err(DomainError::InvalidConfig)?;
    if !root.starts_with('/') || root == "/" || !root.to_ascii_lowercase().contains("ramag") {
        return Err(DomainError::InvalidConfig(
            "RAMAG_TEST_SSH_ROOT 必须是名称含 ramag 的专用绝对目录，且不能是根目录".into(),
        ));
    }

    let mut profile = SshProfile::new("integration-test", host);
    if let Ok(port) = std::env::var("RAMAG_TEST_SSH_PORT") {
        profile.port = Some(port.parse::<u16>().map_err(|error| {
            DomainError::InvalidConfig(format!("RAMAG_TEST_SSH_PORT 无效：{error}"))
        })?);
    }
    profile.username = std::env::var("RAMAG_TEST_SSH_USER").unwrap_or_default();
    if let Ok(password) = std::env::var("RAMAG_TEST_SSH_PASSWORD") {
        if password.is_empty() {
            return Err(DomainError::InvalidConfig(
                "RAMAG_TEST_SSH_PASSWORD 不能为空".into(),
            ));
        }
        profile.auth_mode = SshAuthMode::Password;
        profile.password = password;
    } else if let Ok(key_path) = std::env::var("RAMAG_TEST_SSH_KEY_PATH") {
        profile.auth_mode = SshAuthMode::KeyFile;
        profile.key_path = Some(key_path);
    }
    profile.ssh_path = std::env::var("RAMAG_TEST_SSH_PATH").ok();
    profile.initial_directory = Some(root.clone());
    profile.validate().map_err(DomainError::InvalidConfig)?;
    Ok(Some(Fixture { profile, root }))
}

#[tokio::test]
async fn openssh_sftp_round_trip_is_streamed_and_cleaned() -> Result<()> {
    let Some(fixture) = fixture_from_env()? else {
        return Ok(());
    };
    let driver = integration_driver();
    let case_name = format!("case-{}", uuid::Uuid::new_v4());
    let case_directory =
        join_remote_path(&fixture.root, &case_name).map_err(DomainError::InvalidConfig)?;
    let source_path =
        join_remote_path(&case_directory, "source.bin").map_err(DomainError::InvalidConfig)?;
    let renamed_path =
        join_remote_path(&case_directory, "renamed.bin").map_err(DomainError::InvalidConfig)?;
    let local_directory = tempfile::tempdir()
        .map_err(|error| DomainError::Other(format!("创建本地测试目录失败：{error}")))?;
    let local_source = local_directory.path().join("source.bin");
    let local_download = local_directory.path().join("download.bin");
    let local_archive = local_directory.path().join("case.tar.gz");
    let payload = (0..256 * 1024)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let edited = b"edited through Ramag\n".to_vec();
    std::fs::write(&local_source, &payload)
        .map_err(|error| DomainError::Other(format!("写入本地测试文件失败：{error}")))?;
    let uploaded = Arc::new(AtomicU64::new(0));
    let uploaded_for_progress = uploaded.clone();
    let upload_progress: SshProgressFn = Arc::new(move |transferred, _| {
        uploaded_for_progress.store(transferred, Ordering::Release);
    });

    let result = async {
        driver.probe(fixture.profile.ssh_path.as_deref()).await?;
        driver.test_connection(&fixture.profile).await?;
        driver
            .create_directory(&fixture.profile, &case_directory)
            .await?;
        driver
            .upload(
                &fixture.profile,
                &local_source,
                &source_path,
                OverwritePolicy::Refuse,
                TransferCancellation::default(),
                upload_progress,
            )
            .await?;
        let entries = driver
            .list_directory(&fixture.profile, &case_directory)
            .await?;
        if !entries
            .entries
            .iter()
            .any(|entry| entry.path == source_path && entry.size == payload.len() as u64)
        {
            return Err(DomainError::Other(
                "上传后的远程文件元数据不符合预期".into(),
            ));
        }
        let preview = driver
            .read_file_preview(&fixture.profile, &source_path)
            .await?;
        if preview.bytes != payload || preview.truncated {
            return Err(DomainError::Other("远程文件预览内容不符合预期".into()));
        }
        let chunk = driver
            .read_file_chunk(
                &fixture.profile,
                &source_path,
                RemoteFileChunkPosition::Tail,
            )
            .await?;
        if chunk.bytes != payload || chunk.offset != 0 || chunk.total_bytes != payload.len() as u64
        {
            return Err(DomainError::Other("远程文件分段内容不符合预期".into()));
        }
        driver
            .save_file(&fixture.profile, &source_path, &payload, &edited)
            .await?;
        driver
            .download(
                &fixture.profile,
                &source_path,
                &local_download,
                OverwritePolicy::Refuse,
                TransferCancellation::default(),
                Arc::new(|_, _| {}),
            )
            .await?;
        let downloaded = std::fs::read(&local_download)
            .map_err(|error| DomainError::Other(format!("读取下载结果失败：{error}")))?;
        if downloaded != edited {
            return Err(DomainError::Other("下载内容与上传源不一致".into()));
        }
        driver
            .rename(&fixture.profile, &source_path, &renamed_path)
            .await?;
        let archived = Arc::new(AtomicU64::new(0));
        let archived_for_progress = archived.clone();
        driver
            .download_directory(
                &fixture.profile,
                &case_directory,
                &local_archive,
                OverwritePolicy::Refuse,
                TransferCancellation::default(),
                Arc::new(move |transferred, _| {
                    archived_for_progress.store(transferred, Ordering::Release);
                }),
            )
            .await?;
        let archive_size = std::fs::metadata(&local_archive)
            .map_err(|error| DomainError::Other(format!("读取目录归档信息失败：{error}")))?
            .len();
        if archive_size == 0 || archived.load(Ordering::Acquire) != edited.len() as u64 {
            return Err(DomainError::Other("目录归档结果不符合预期".into()));
        }
        let archive_file = async_std::fs::File::open(&local_archive)
            .await
            .map_err(|error| DomainError::Other(format!("打开目录归档失败：{error}")))?;
        let mut archive = async_tar::Archive::new(GzipDecoder::new(BufReader::new(archive_file)))
            .entries()
            .map_err(|error| DomainError::Other(format!("读取目录归档失败：{error}")))?;
        let expected_archive_path = format!("{case_name}/renamed.bin");
        let mut archived_contents = None;
        while let Some(entry) = archive.next().await {
            let mut entry = entry
                .map_err(|error| DomainError::Other(format!("读取目录归档项目失败：{error}")))?;
            let path = entry
                .path()
                .map_err(|error| DomainError::Other(format!("读取归档路径失败：{error}")))?;
            if path.to_string_lossy() == expected_archive_path {
                let mut contents = Vec::new();
                entry
                    .read_to_end(&mut contents)
                    .await
                    .map_err(|error| DomainError::Other(format!("读取归档文件失败：{error}")))?;
                archived_contents = Some(contents);
            }
        }
        if archived_contents.as_deref() != Some(edited.as_slice()) {
            return Err(DomainError::Other("目录归档文件内容不符合预期".into()));
        }
        driver
            .remove(&fixture.profile, &renamed_path, RemoteEntryKind::File)
            .await?;
        Ok(())
    }
    .await;

    let cleanup = driver
        .remove(
            &fixture.profile,
            &case_directory,
            RemoteEntryKind::Directory,
        )
        .await;
    let shutdown = driver.shutdown().await;
    result?;
    cleanup?;
    shutdown?;
    if uploaded.load(Ordering::Acquire) != payload.len() as u64 {
        return Err(DomainError::Other("上传进度未到达文件总大小".into()));
    }
    Ok(())
}

#[tokio::test]
async fn openssh_terminal_and_port_forward_round_trip_on_remote_server() -> Result<()> {
    let Some(test_context) = fixture_from_env()? else {
        return Ok(());
    };
    let driver = integration_driver();
    let terminal_marker = format!("ramag-terminal-ok-{}", uuid::Uuid::new_v4().simple());
    let terminal_command = format!("printf '%s\\n' '{terminal_marker}'; exit 0");
    let terminal_output =
        run_remote_command(&driver, &test_context.profile, &terminal_command).await?;
    if !terminal_output
        .windows(terminal_marker.len())
        .any(|window| window == terminal_marker.as_bytes())
    {
        return Err(DomainError::Other("远端终端输出未包含预期标记".into()));
    }

    let case_name = format!("ramag-forward-{}", uuid::Uuid::new_v4().simple());
    let remote_log = format!("/tmp/{case_name}.log");
    let remote_port = 40_000
        + (u16::from_le_bytes(
            uuid::Uuid::new_v4().as_bytes()[..2]
                .try_into()
                .map_err(|_| DomainError::Other("生成远端端口失败".into()))?,
        ) % 10_000);
    let server_command = format!(
        "nohup python3 -c 'import socket; s=socket.socket(); s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1); s.bind((\"127.0.0.1\",{remote_port})); s.listen(1); c,_=s.accept(); data=c.recv(1024); c.sendall(b\"ramag-forward-ok:\"+data); c.close(); s.close()' >{remote_log} 2>&1 </dev/null & echo $!; exit 0"
    );
    let server_pid = parse_remote_pid(
        &run_remote_command_without_pty(&driver, &test_context.profile, &server_command).await?,
    )?;
    let local_port = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| DomainError::Other(format!("分配本地转发端口失败：{error}")))?
        .local_addr()
        .map_err(|error| DomainError::Other(format!("读取本地转发端口失败：{error}")))?
        .port();

    let mut forwarding_profile = test_context.profile.clone();
    forwarding_profile.port_forwardings = vec![SshPortForward::Local {
        bind_address: Some("127.0.0.1".into()),
        listen_port: local_port,
        target_host: "127.0.0.1".into(),
        target_port: remote_port,
    }];
    let result = async {
        let command = driver.port_forward_command(&forwarding_profile).await?;
        let mut process = Command::new(&command.program)
            .args(&command.args)
            .envs(&command.env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| DomainError::Other(format!("启动本地端口转发失败：{error}")))?;
        let response = async {
            for _ in 0..100 {
                if process
                    .try_wait()
                    .map_err(|error| DomainError::Other(format!("读取端口转发状态失败：{error}")))?
                    .is_some()
                {
                    return Err(DomainError::ConnectionFailed(
                        "SSH 端口转发进程在连接前退出".into(),
                    ));
                }
                if let Ok(Ok(mut stream)) = timeout(
                    Duration::from_millis(300),
                    TcpStream::connect(("127.0.0.1", local_port)),
                )
                .await
                {
                    stream.write_all(b"ping").await.map_err(|error| {
                        DomainError::Other(format!("写入转发连接失败：{error}"))
                    })?;
                    let mut bytes = Vec::new();
                    timeout(Duration::from_secs(3), stream.read_to_end(&mut bytes))
                        .await
                        .map_err(|_| DomainError::ConnectionFailed("读取端口转发结果超时".into()))?
                        .map_err(|error| {
                            DomainError::Other(format!("读取转发结果失败：{error}"))
                        })?;
                    if bytes != b"ramag-forward-ok:ping" {
                        return Err(DomainError::Other("端口转发回读内容不符合预期".into()));
                    }
                    return Ok(());
                }
                sleep(Duration::from_millis(100)).await;
            }
            Err(DomainError::ConnectionFailed(
                "本地端口转发在 30 秒内没有接受连接".into(),
            ))
        }
        .await;
        let _ = process.kill().await;
        let output = process
            .wait_with_output()
            .await
            .map_err(|error| DomainError::Other(format!("读取端口转发进程结果失败：{error}")))?;
        if let Err(error) = response {
            let stderr = bounded_process_output(&output.stderr);
            if !stderr.is_empty() {
                return Err(DomainError::ConnectionFailed(format!(
                    "{error}；OpenSSH：{stderr}"
                )));
            }
            return Err(error);
        }
        response
    }
    .await;
    let cleanup_command =
        format!("kill {server_pid} >/dev/null 2>&1 || true; rm -f {remote_log}; exit 0");
    let cleanup =
        run_remote_command_without_pty(&driver, &test_context.profile, &cleanup_command).await;
    let shutdown = driver.shutdown().await;
    result?;
    cleanup?;
    shutdown
}

fn integration_driver() -> OpenSshDriver {
    std::env::var_os("RAMAG_TEST_SSH_ASKPASS_EXECUTABLE")
        .map(OpenSshDriver::with_askpass_executable)
        .unwrap_or_default()
}

async fn run_remote_command(
    driver: &OpenSshDriver,
    profile: &SshProfile,
    remote_command: &str,
) -> Result<Vec<u8>> {
    run_remote_command_with_pty(driver, profile, remote_command, true).await
}

async fn run_remote_command_without_pty(
    driver: &OpenSshDriver,
    profile: &SshProfile,
    remote_command: &str,
) -> Result<Vec<u8>> {
    run_remote_command_with_pty(driver, profile, remote_command, false).await
}

async fn run_remote_command_with_pty(
    driver: &OpenSshDriver,
    profile: &SshProfile,
    remote_command: &str,
    allocate_pty: bool,
) -> Result<Vec<u8>> {
    let mut command = driver.terminal_command(profile, None).await?;
    if !allocate_pty {
        command.args.retain(|argument| argument != "-tt");
    }
    command.args.push(remote_command.to_string());
    let output = timeout(
        Duration::from_secs(20),
        Command::new(&command.program)
            .args(&command.args)
            .envs(&command.env)
            .output(),
    )
    .await
    .map_err(|_| DomainError::ConnectionFailed("远端 SSH 命令执行超时".into()))?
    .map_err(|error| DomainError::ConnectionFailed(format!("启动远端 SSH 命令失败：{error}")))?;
    if !output.status.success() {
        return Err(DomainError::ConnectionFailed(
            "远端 SSH 命令执行失败".into(),
        ));
    }
    if output.stdout.len() > 16 * 1024 {
        return Err(DomainError::Other("远端 SSH 命令输出超过限制".into()));
    }
    Ok(output.stdout)
}

fn parse_remote_pid(output: &[u8]) -> Result<u32> {
    String::from_utf8_lossy(output)
        .split(|character: char| !character.is_ascii_digit())
        .filter_map(|value| value.parse::<u32>().ok())
        .find(|pid| *pid > 0)
        .ok_or_else(|| DomainError::Other("远端临时服务没有返回进程号".into()))
}

fn bounded_process_output(output: &[u8]) -> String {
    String::from_utf8_lossy(&output[..output.len().min(4096)])
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect::<String>()
        .trim()
        .to_string()
}
