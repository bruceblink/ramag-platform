    use super::*;
    use ramag_domain::traits::MqttLocalServerDriver;

    #[test]
    fn default_build_exposes_native_capability_without_claiming_runtime_support() {
        let capabilities = NativeMqttTransport::new().transport_capabilities();
        assert_eq!(capabilities.backend, MqttTransportBackend::Native);
        assert_eq!(capabilities.build_available, cfg!(feature = "native"));
    }

    #[cfg(feature = "native")]
    #[test]
    fn management_profile_prefers_dedicated_management_credentials() {
        let mut profile = MqttProfile::new("local", "127.0.0.1", 1883);
        profile.username = Some("data-user".into());
        profile.password = Some("data-password".into());
        profile.management.admin_username = Some("admin".into());
        profile.management.admin_password = Some("admin-password".into());

        let connection = management_profile(&profile);
        assert_eq!(connection.username.as_deref(), Some("admin"));
        assert_eq!(connection.password.as_deref(), Some("admin-password"));
    }

    #[cfg(feature = "native")]
    #[test]
    fn native_runtime_executes_operations_inside_tokio_context() {
        let result = smol::block_on(run_native(|| async { Ok::<_, DomainError>(()) }));
        assert!(result.is_ok());
    }

    /// Run manually with `RAMAG_MQTT_LIVE_HOST` and optional port/protocol env vars.
    #[cfg(feature = "native")]
    #[test]
    #[ignore = "requires an explicitly configured live MQTT broker"]
    fn native_connection_reaches_live_broker() -> std::result::Result<(), String> {
        let Some(host) = std::env::var_os("RAMAG_MQTT_LIVE_HOST") else {
            return Ok(());
        };
        let port = std::env::var("RAMAG_MQTT_LIVE_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1883);
        let protocol = match std::env::var("RAMAG_MQTT_LIVE_PROTOCOL").as_deref() {
            Ok("311") => ramag_domain::entities::MqttProtocolVersion::V311,
            _ => ramag_domain::entities::MqttProtocolVersion::V5,
        };
        let profile = {
            let mut profile = MqttProfile::new("live-test", host.to_string_lossy(), port);
            profile.protocol_version = protocol;
            profile
        };

        smol::block_on(async {
            NativeMqttTransport::new()
                .test_connection(&profile)
                .await
                .map_err(|error| format!("Native MQTT 应连接到显式配置的 Broker: {error}"))
        })
    }

    #[test]
    fn local_static_driver_round_trips_a_configured_file() -> std::result::Result<(), String> {
        let path = std::env::temp_dir().join(format!(
            "ramag-mosquitto-{}-{}.conf",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos()
        ));
        std::fs::write(&path, "old-content\n").map_err(|error| error.to_string())?;

        let mut profile = MqttProfile::new("local", "127.0.0.1", 1883);
        profile.management.enabled = true;
        profile.management.static_config = Some(ramag_domain::entities::MosquittoStaticConfig {
            target: ramag_domain::entities::MosquittoConfigTarget::Local,
            password_file: Some(path.to_string_lossy().into_owned()),
            acl_file: None,
        });
        let file = ramag_domain::entities::MosquittoStaticFile {
            kind: MosquittoStaticFileKind::Password,
            path: path.to_string_lossy().into_owned(),
            content: "new-content\n".into(),
        };
        let driver = LocalMosquittoStaticConfigDriver::new();
        let loaded = smol::block_on(async {
            driver
                .write_file(&profile, &file)
                .await
                .map_err(|error| error.user_message())?;
            driver
                .read_file(&profile, MosquittoStaticFileKind::Password)
                .await
                .map_err(|error| error.user_message())
        })?;
        assert_eq!(loaded.content, "new-content\n");
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[cfg(feature = "native")]
    #[test]
    fn local_server_enforces_configured_credentials() -> std::result::Result<(), String> {
        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| error.to_string())?
            .local_addr()
            .map_err(|error| error.to_string())?
            .port();
        let config = ramag_domain::entities::MqttLocalServerConfig {
            bind_host: "127.0.0.1".into(),
            port,
            allow_anonymous: false,
            users: vec![ramag_domain::entities::MqttLocalServerUser {
                username: "operator".into(),
                password: "correct-password".into(),
            }],
        };
        let server = NativeMqttLocalServer::new();
        smol::block_on(server.start(&config)).map_err(|error| error.to_string())?;

        let client = NativeMqttTransport::new();
        let mut no_credentials = MqttProfile::new("no-credentials", "127.0.0.1", port);
        no_credentials.keep_alive_seconds = 5;
        let no_credentials_result = smol::block_on(client.test_connection(&no_credentials));

        let mut wrong_credentials = no_credentials.clone();
        wrong_credentials.username = Some("operator".into());
        wrong_credentials.password = Some("wrong-password".into());
        let wrong_credentials_result = smol::block_on(client.test_connection(&wrong_credentials));

        let mut correct_credentials = no_credentials;
        correct_credentials.username = Some("operator".into());
        correct_credentials.password = Some("correct-password".into());
        let correct_credentials_result =
            smol::block_on(client.test_connection(&correct_credentials));

        smol::block_on(server.stop()).map_err(|error| error.to_string())?;
        assert!(no_credentials_result.is_err(), "未提供账号时不应连接成功");
        assert!(wrong_credentials_result.is_err(), "错误账号不应连接成功");
        assert!(correct_credentials_result.is_ok(), "正确账号应连接成功");
        Ok(())
    }

    #[cfg(feature = "native")]
    #[test]
    fn local_server_allows_anonymous_alongside_configured_credentials() -> std::result::Result<(), String> {
        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| error.to_string())?
            .local_addr()
            .map_err(|error| error.to_string())?
            .port();
        let config = ramag_domain::entities::MqttLocalServerConfig {
            bind_host: "127.0.0.1".into(),
            port,
            allow_anonymous: true,
            users: vec![ramag_domain::entities::MqttLocalServerUser {
                username: "operator".into(),
                password: "correct-password".into(),
            }],
        };
        let server = NativeMqttLocalServer::new();
        smol::block_on(server.start(&config)).map_err(|error| error.to_string())?;

        let client = NativeMqttTransport::new();
        let mut anonymous = MqttProfile::new("anonymous", "127.0.0.1", port);
        anonymous.keep_alive_seconds = 5;
        let anonymous_result = smol::block_on(client.test_connection(&anonymous));

        let mut correct_credentials = anonymous.clone();
        correct_credentials.username = Some("operator".into());
        correct_credentials.password = Some("correct-password".into());
        let correct_credentials_result =
            smol::block_on(client.test_connection(&correct_credentials));

        let mut wrong_credentials = anonymous;
        wrong_credentials.username = Some("operator".into());
        wrong_credentials.password = Some("wrong-password".into());
        let wrong_credentials_result = smol::block_on(client.test_connection(&wrong_credentials));

        smol::block_on(server.stop()).map_err(|error| error.to_string())?;
        assert!(anonymous_result.is_ok(), "允许匿名时未提供账号应连接成功");
        assert!(
            correct_credentials_result.is_ok(),
            "固定账号仍应连接成功"
        );
        assert!(wrong_credentials_result.is_err(), "错误账号不应连接成功");
        Ok(())
    }

    #[cfg(feature = "native")]
    #[test]
    fn local_server_starts_and_stops_on_a_free_port() -> std::result::Result<(), String> {
        let driver = NativeMqttLocalServer::new();
        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| error.to_string())?
            .local_addr()
            .map_err(|error| error.to_string())?
            .port();
        let config = ramag_domain::entities::MqttLocalServerConfig {
            bind_host: "127.0.0.1".into(),
            port,
            allow_anonymous: true,
            users: Vec::new(),
        };
        let (started, stopped) = smol::block_on(async {
            let started = driver
                .start(&config)
                .await
                .map_err(|error| error.to_string())?;
            let stopped = driver.stop().await.map_err(|error| error.to_string())?;
            Ok::<_, String>((started, stopped))
        })?;
        assert!(started.running);
        assert!(!stopped.running);
        Ok(())
    }

    #[cfg(feature = "native")]
    #[test]
    fn local_server_supports_the_native_client_publish_subscribe_loop() -> std::result::Result<(), String> {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{TrySendError, sync_channel},
        };
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        use ramag_domain::entities::{
            MqttMessageSinkResult, MqttQos, MqttSubscribeRequest, MqttSubscription,
        };

        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| error.to_string())?
            .local_addr()
            .map_err(|error| error.to_string())?
            .port();
        let config = ramag_domain::entities::MqttLocalServerConfig {
            bind_host: "127.0.0.1".into(),
            port,
            allow_anonymous: true,
            users: Vec::new(),
        };
        let server = NativeMqttLocalServer::new();
        smol::block_on(server.start(&config)).map_err(|error| error.to_string())?;

        let mut profile = MqttProfile::new("local-loop", "127.0.0.1", port);
        profile.keep_alive_seconds = 5;
        let client = NativeMqttTransport::new();
        let mut connected = false;
        for _ in 0..20 {
            if smol::block_on(client.test_connection(&profile)).is_ok() {
                connected = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        if !connected {
            let _ = smol::block_on(server.stop());
            return Err("本地 MQTT Broker 未能接受 Native Client 连接".into());
        }

        let topic = format!(
            "ramag/local/{}/{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos()
        );
        let cancelled = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = sync_channel(1);
        let sink: MqttMessageSink = Arc::new(move |message| match sender.try_send(message) {
            Ok(()) => MqttMessageSinkResult::Accepted,
            Err(TrySendError::Full(_)) => MqttMessageSinkResult::Backpressured,
            Err(TrySendError::Disconnected(_)) => MqttMessageSinkResult::Closed,
        });
        let subscribe_profile = profile.clone();
        let subscribe_cancelled = cancelled.clone();
        let subscribe_topic = topic.clone();
        let subscription = std::thread::spawn(move || {
            smol::block_on(NativeMqttTransport::new().subscribe(
                &subscribe_profile,
                &MqttSubscribeRequest {
                    subscriptions: vec![MqttSubscription {
                        filter: subscribe_topic,
                        qos: MqttQos::AtLeastOnce,
                    }],
                },
                sink,
                subscribe_cancelled,
            ))
        });
        std::thread::sleep(Duration::from_millis(200));

        let payload = b"local broker payload".to_vec();
        smol::block_on(client.publish(
            &profile,
            &MqttPublishRequest {
                topic: topic.clone(),
                payload: payload.clone(),
                qos: MqttQos::AtLeastOnce,
                retain: false,
                user_properties: Vec::new(),
            },
        ))
        .map_err(|error| format!("发布阶段失败：{error}"))?;
        let message = receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        assert_eq!(message.topic, topic);
        assert_eq!(message.payload, payload);

        cancelled.store(true, Ordering::Release);
        smol::block_on(server.stop()).map_err(|error| error.to_string())?;
        subscription
            .join()
            .map_err(|_| "本地 MQTT 订阅线程不应 panic".to_string())?
            .map_err(|error| error.to_string())?;
        Ok(())
    }
