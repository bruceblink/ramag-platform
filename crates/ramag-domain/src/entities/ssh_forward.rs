use serde::{Deserialize, Serialize};

pub const MAX_SSH_PORT_FORWARDINGS: usize = 16;
pub const MAX_SSH_FORWARD_ADDRESS_BYTES: usize = 256;

type ValidationResult<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SshSessionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Exited,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SshPortForwardDirection {
    Local,
    Remote,
    Dynamic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "direction")]
pub enum SshPortForward {
    Local {
        bind_address: Option<String>,
        listen_port: u16,
        target_host: String,
        target_port: u16,
    },
    Remote {
        bind_address: Option<String>,
        listen_port: u16,
        target_host: String,
        target_port: u16,
    },
    Dynamic {
        bind_address: Option<String>,
        listen_port: u16,
    },
}

impl SshPortForward {
    pub fn direction(&self) -> SshPortForwardDirection {
        match self {
            Self::Local { .. } => SshPortForwardDirection::Local,
            Self::Remote { .. } => SshPortForwardDirection::Remote,
            Self::Dynamic { .. } => SshPortForwardDirection::Dynamic,
        }
    }

    pub fn validate(&self) -> ValidationResult<()> {
        match self {
            Self::Local {
                bind_address,
                listen_port,
                target_host,
                target_port,
            }
            | Self::Remote {
                bind_address,
                listen_port,
                target_host,
                target_port,
            } => {
                validate_forward_bind_address(bind_address.as_deref())?;
                validate_forward_port("监听端口", *listen_port)?;
                validate_forward_host("目标主机", target_host)?;
                validate_forward_port("目标端口", *target_port)?;
            }
            Self::Dynamic {
                bind_address,
                listen_port,
            } => {
                validate_forward_bind_address(bind_address.as_deref())?;
                validate_forward_port("监听端口", *listen_port)?;
            }
        }
        Ok(())
    }

    /// 返回一个可以作为单独 argv 参数传给 OpenSSH 的 `-L/-R/-D` 值。
    pub fn open_ssh_argument(&self) -> ValidationResult<String> {
        self.validate()?;
        match self {
            Self::Local {
                bind_address,
                listen_port,
                target_host,
                target_port,
            }
            | Self::Remote {
                bind_address,
                listen_port,
                target_host,
                target_port,
            } => Ok(format!(
                "{}:{}",
                format_listen_endpoint(bind_address.as_deref(), *listen_port),
                format_host_port(target_host, *target_port)
            )),
            Self::Dynamic {
                bind_address,
                listen_port,
            } => Ok(format_listen_endpoint(
                bind_address.as_deref(),
                *listen_port,
            )),
        }
    }

    pub fn parse_open_ssh_argument(
        direction: SshPortForwardDirection,
        value: &str,
    ) -> ValidationResult<Self> {
        if value.is_empty() {
            return Err("SSH 端口转发参数不能为空".into());
        }
        let (bind_address, listen_port, remainder) =
            parse_listen_endpoint(value, direction != SshPortForwardDirection::Dynamic)?;
        let forwarding = match direction {
            SshPortForwardDirection::Local => {
                let (target_host, target_port) = parse_host_port(
                    remainder
                        .ok_or_else(|| "SSH 本地转发必须包含目标主机和目标端口".to_string())?,
                )?;
                Self::Local {
                    bind_address,
                    listen_port,
                    target_host,
                    target_port,
                }
            }
            SshPortForwardDirection::Remote => {
                let (target_host, target_port) = parse_host_port(
                    remainder
                        .ok_or_else(|| "SSH 远程转发必须包含目标主机和目标端口".to_string())?,
                )?;
                Self::Remote {
                    bind_address,
                    listen_port,
                    target_host,
                    target_port,
                }
            }
            SshPortForwardDirection::Dynamic => Self::Dynamic {
                bind_address,
                listen_port,
            },
        };
        forwarding.validate()?;
        Ok(forwarding)
    }
}

fn validate_forward_bind_address(address: Option<&str>) -> ValidationResult<()> {
    let Some(address) = address else {
        return Ok(());
    };
    validate_forward_component("监听地址", address)?;
    if address.starts_with('[') || address.ends_with(']') {
        return Err("监听地址不要包含 IPv6 方括号".into());
    }
    Ok(())
}

fn validate_forward_host(label: &str, host: &str) -> ValidationResult<()> {
    validate_forward_component(label, host)?;
    if host.starts_with('-') || host.starts_with('[') || host.ends_with(']') {
        return Err(format!("{label}格式无效"));
    }
    Ok(())
}

fn validate_forward_component(label: &str, value: &str) -> ValidationResult<()> {
    if value.is_empty() {
        return Err(format!("{label}不能为空"));
    }
    if value.len() > MAX_SSH_FORWARD_ADDRESS_BYTES {
        return Err(format!(
            "{label}不能超过 {MAX_SSH_FORWARD_ADDRESS_BYTES} 字节"
        ));
    }
    if value
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(format!("{label}不能包含空白或控制字符"));
    }
    Ok(())
}

fn validate_forward_port(label: &str, port: u16) -> ValidationResult<()> {
    if port == 0 {
        return Err(format!("{label}必须在 1 - 65535 之间"));
    }
    Ok(())
}

fn format_listen_endpoint(bind_address: Option<&str>, port: u16) -> String {
    match bind_address {
        Some(address) if address.contains(':') => format!("[{address}]:{port}"),
        Some(address) => format!("{address}:{port}"),
        None => port.to_string(),
    }
}

fn format_host_port(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn parse_listen_endpoint(
    value: &str,
    require_remainder: bool,
) -> ValidationResult<(Option<String>, u16, Option<&str>)> {
    let (bind_address, port_value) = if let Some(value) = value.strip_prefix('[') {
        let close = value
            .find(']')
            .ok_or_else(|| "SSH 端口转发监听地址缺少 ]".to_string())?;
        let address = &value[..close];
        let remainder = value
            .get(close + 1..)
            .and_then(|value| value.strip_prefix(':'))
            .ok_or_else(|| "SSH IPv6 监听地址必须使用 [地址]:端口".to_string())?;
        (Some(address.to_string()), remainder)
    } else if let Some((first, _)) = value.split_once(':') {
        if first.parse::<u16>().is_ok() {
            (None, value)
        } else {
            let (address, remainder) = value
                .split_once(':')
                .ok_or_else(|| "SSH 端口转发缺少监听端口".to_string())?;
            (Some(address.to_string()), remainder)
        }
    } else {
        (None, value)
    };

    let (port_text, remainder) = port_value
        .split_once(':')
        .map_or((port_value, None), |(port, remainder)| {
            (port, Some(remainder))
        });
    let listen_port = port_text
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| "SSH 监听端口必须在 1 - 65535 之间".to_string())?;
    if require_remainder && remainder.is_none_or(str::is_empty) {
        return Err("SSH 端口转发缺少目标地址".into());
    }
    if !require_remainder && remainder.is_some() {
        return Err("SSH 动态转发只能包含监听地址和监听端口".into());
    }
    Ok((bind_address, listen_port, remainder))
}

fn parse_host_port(value: &str) -> ValidationResult<(String, u16)> {
    let (host, port_text) = if let Some(value) = value.strip_prefix('[') {
        let close = value
            .find(']')
            .ok_or_else(|| "SSH 端口转发目标地址缺少 ]".to_string())?;
        let host = &value[..close];
        let port = value
            .get(close + 1..)
            .and_then(|value| value.strip_prefix(':'))
            .ok_or_else(|| "SSH IPv6 目标地址必须使用 [地址]:端口".to_string())?;
        (host, port)
    } else {
        let (host, port) = value
            .rsplit_once(':')
            .ok_or_else(|| "SSH 端口转发缺少目标端口".to_string())?;
        if host.contains(':') {
            return Err("SSH IPv6 目标地址必须使用 [地址]:端口".into());
        }
        (host, port)
    };
    let target_port = port_text
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| "SSH 目标端口必须在 1 - 65535 之间".to_string())?;
    let host = host.to_string();
    validate_forward_host("目标主机", &host)?;
    Ok((host, target_port))
}
