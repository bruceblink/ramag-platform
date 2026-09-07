use super::*;

#[test]
fn profile_rejects_option_injection_and_unsupported_key() {
    let mut profile = SshProfile::new("server", "-oProxyCommand=bad");
    assert!(profile.validate().is_err());

    profile.host = "server.example".into();
    profile.auth_mode = SshAuthMode::KeyFile;
    profile.key_path = Some(if cfg!(windows) {
        r"C:\\keys\\server.ppk".into()
    } else {
        "/keys/server.ppk".into()
    });
    assert!(matches!(
        profile.validate(),
        Err(error) if error.contains(".ppk")
    ));
}

#[test]
fn profile_requires_absolute_paths() {
    let mut profile = SshProfile::new("server", "example.com");
    profile.auth_mode = SshAuthMode::KeyFile;
    profile.key_path = Some("relative-key".into());
    assert!(matches!(
        profile.validate(),
        Err(error) if error.contains("绝对路径")
    ));

    profile.auth_mode = SshAuthMode::System;
    profile.key_path = None;
    profile.ssh_path = Some("ssh".into());
    assert!(matches!(
        profile.validate(),
        Err(error) if error.contains("绝对路径")
    ));
}

#[test]
fn password_profile_requires_a_single_line_secret() {
    let mut profile = SshProfile::new("server", "example.com");
    profile.auth_mode = SshAuthMode::Password;
    assert!(matches!(
        profile.validate(),
        Err(error) if error.contains("密码不能为空")
    ));

    profile.password = "secret#value".into();
    assert!(profile.validate().is_ok());

    profile.password = "bad\npassword".into();
    assert!(matches!(
        profile.validate(),
        Err(error) if error.contains("控制字符")
    ));
}

#[test]
fn profile_accepts_config_alias_and_hash_in_username() {
    let mut profile = SshProfile::new("jump", "private-jump");
    profile.username = "team#account#00000000-0000-0000-0000-000000000000".into();
    profile.port = None;

    assert!(profile.validate().is_ok());
}

#[test]
fn windows_sftp_compatibility_rejects_explicit_linux_platform() {
    let mut profile = SshProfile::new("windows", "server.example");
    profile.windows_sftp_compatibility = true;
    profile.remote_platform = RemotePlatformPreference::Linux;
    assert!(matches!(
        profile.validate(),
        Err(error) if error.contains("不能用于明确的 Linux")
    ));

    profile.remote_platform = RemotePlatformPreference::Auto;
    assert!(profile.validate().is_ok());
    profile.remote_platform = RemotePlatformPreference::Windows;
    assert!(profile.validate().is_ok());
}

#[test]
fn legacy_profile_without_origin_defaults_to_manual()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let profile = SshProfile::new("legacy", "server.example");
    let mut value = serde_json::to_value(&profile)?;
    let Some(object) = value.as_object_mut() else {
        return Err("SSH 配置应序列化为 JSON 对象".into());
    };
    object.remove("origin");
    object.remove("windows_sftp_compatibility");
    object.remove("rdp_web_enabled");
    object.remove("jumpserver_rdp_session");

    let decoded: SshProfile = serde_json::from_value(value)?;

    assert_eq!(decoded.origin, SshProfileOrigin::Manual);
    assert!(!decoded.windows_sftp_compatibility);
    assert_eq!(decoded.rdp_web_enabled, None);
    assert_eq!(decoded.jumpserver_rdp_session, None);
    Ok(())
}

#[test]
fn remote_paths_do_not_escape_through_names() {
    assert_eq!(
        join_remote_path("/home/user", "file.txt").as_deref(),
        Ok("/home/user/file.txt")
    );
    assert_eq!(
        parent_remote_path("/home/user/file.txt").as_deref(),
        Ok("/home/user")
    );
    assert_eq!(parent_remote_path("/").as_deref(), Ok("/"));
    assert!(join_remote_path("/home/user", "../secret").is_err());
    assert!(join_remote_path("/home/user", "dir/file").is_err());
}

#[test]
fn transfer_status_is_terminal_once_finished() {
    let mut task = TransferTask::new(
        SshProfileId::new(),
        TransferDirection::Download,
        "/tmp/local",
        "/remote/file",
    );
    task.mark_running();
    task.update_progress(5, 10);
    task.finish(Ok(()), false);
    task.finish(Err("late error".into()), false);

    assert_eq!(task.status, TransferStatus::Completed);
    assert_eq!(task.transferred_bytes, 5);
    assert!(task.error.is_none());
}

#[test]
fn port_forward_models_round_trip_open_ssh_arguments() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            SshPortForwardDirection::Local,
            "127.0.0.1:8080:db.internal:5432",
            SshPortForward::Local {
                bind_address: Some("127.0.0.1".into()),
                listen_port: 8080,
                target_host: "db.internal".into(),
                target_port: 5432,
            },
        ),
        (
            SshPortForwardDirection::Remote,
            "9000:127.0.0.1:9000",
            SshPortForward::Remote {
                bind_address: None,
                listen_port: 9000,
                target_host: "127.0.0.1".into(),
                target_port: 9000,
            },
        ),
        (
            SshPortForwardDirection::Dynamic,
            "[::1]:1080",
            SshPortForward::Dynamic {
                bind_address: Some("::1".into()),
                listen_port: 1080,
            },
        ),
    ];

    for (direction, argument, expected) in cases {
        let parsed = SshPortForward::parse_open_ssh_argument(direction, argument)?;
        assert_eq!(parsed, expected);
        assert_eq!(parsed.open_ssh_argument()?, argument);
    }
    Ok(())
}

#[test]
fn port_forward_model_rejects_unsafe_or_invalid_values() {
    let invalid = [
        SshPortForward::Local {
            bind_address: Some("127.0.0.1".into()),
            listen_port: 0,
            target_host: "db.internal".into(),
            target_port: 5432,
        },
        SshPortForward::Remote {
            bind_address: None,
            listen_port: 9000,
            target_host: "db internal".into(),
            target_port: 9000,
        },
        SshPortForward::Dynamic {
            bind_address: Some("[::1]".into()),
            listen_port: 1080,
        },
    ];
    assert!(
        invalid
            .iter()
            .all(|forwarding| forwarding.validate().is_err())
    );

    assert!(
        SshPortForward::parse_open_ssh_argument(SshPortForwardDirection::Local, "8080:db.internal")
            .is_err()
    );
}

#[test]
fn profile_keeps_legacy_port_forwarding_default_and_enforces_limit()
-> Result<(), Box<dyn std::error::Error>> {
    let profile = SshProfile::new("server", "example.com");
    let value = serde_json::to_value(&profile)?;
    assert_eq!(value["port_forwardings"], serde_json::json!([]));

    let mut legacy = value;
    legacy
        .as_object_mut()
        .ok_or("SSH 配置应序列化为 JSON 对象")?
        .remove("port_forwardings");
    let decoded: SshProfile = serde_json::from_value(legacy)?;
    assert!(decoded.port_forwardings.is_empty());

    let mut too_many = SshProfile::new("server", "example.com");
    too_many.port_forwardings = (0..=MAX_SSH_PORT_FORWARDINGS)
        .map(|port| SshPortForward::Dynamic {
            bind_address: Some("127.0.0.1".into()),
            listen_port: (1000 + port) as u16,
        })
        .collect();
    assert!(too_many.validate().is_err());
    Ok(())
}
