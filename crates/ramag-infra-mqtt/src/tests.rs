    use super::*;
    #[cfg(feature = "native")]
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
    fn local_server_rejects_conflicting_running_configuration() -> std::result::Result<(), String> {
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
        let conflicting = ramag_domain::entities::MqttLocalServerConfig {
            allow_anonymous: false,
            ..config.clone()
        };
        smol::block_on(driver.start(&config)).map_err(|error| error.to_string())?;
        let same = smol::block_on(driver.start(&config)).map_err(|error| error.to_string())?;
        let result = smol::block_on(driver.start(&conflicting));
        let stopped = smol::block_on(driver.stop()).map_err(|error| error.to_string())?;
        assert!(same.running, "相同配置重复启动应返回运行中状态");
        assert!(result.is_err(), "运行中的 Broker 不应静默接受冲突配置");
        assert!(!stopped.running);
        Ok(())
    }

    #[cfg(feature = "native")]
    #[test]
    fn local_server_supports_client_and_broker_publish_subscribe_loop() -> std::result::Result<(), String> {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{TrySendError, sync_channel},
        };
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        use ramag_domain::entities::{
            MqttMessageSinkResult, MqttQos, MqttSubscribeRequest, MqttSubscription,
            MqttSubscriptionCommand,
            MqttSubscriptionState, MqttUserProperty,
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
        let (status_sender, status_receiver) = sync_channel(1);
        let status_sink = Arc::new(move |status| {
            let _ = status_sender.try_send(status);
        });
        let (command_sender, command_receiver) = async_channel::bounded(4);
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
                        no_local: false,
                    }],
                },
                sink,
                status_sink,
                command_receiver,
                subscribe_cancelled,
            ))
        });
        std::thread::sleep(Duration::from_millis(200));
        let status = status_receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        assert_eq!(status.state, MqttSubscriptionState::Subscribed);

        let snapshot = smol::block_on(server.snapshot())
            .map_err(|error| format!("读取本地 Broker 快照失败：{error}"))?;
        assert!(snapshot.online_clients_complete);
        assert_eq!(snapshot.online_clients.len(), 1);
        assert_eq!(snapshot.online_clients[0].subscriptions.len(), 1);
        assert_eq!(snapshot.online_clients[0].subscriptions[0].filter, topic);

        command_sender
            .try_send(MqttSubscriptionCommand::Unsubscribe {
                filter: topic.clone(),
            })
            .map_err(|error| error.to_string())?;
        let status = status_receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        assert_eq!(status.filter, topic);
        assert_eq!(status.state, MqttSubscriptionState::Pending);

        command_sender
            .try_send(MqttSubscriptionCommand::Subscribe(MqttSubscription {
                filter: topic.clone(),
                qos: MqttQos::AtLeastOnce,
                no_local: false,
            }))
            .map_err(|error| error.to_string())?;
        let status = status_receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        assert_eq!(status.state, MqttSubscriptionState::Subscribed);

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

        let injected_payload = b"injected broker payload".to_vec();
        let injected_result = smol::block_on(server.publish(&MqttPublishRequest {
            topic: topic.clone(),
            payload: injected_payload.clone(),
            qos: MqttQos::ExactlyOnce,
            retain: true,
            user_properties: vec![MqttUserProperty {
                name: "source".into(),
                value: "local-broker".into(),
            }],
        }))
        .map_err(|error| format!("Broker 注入发布失败：{error}"))?;
        assert_eq!(injected_result.topic, topic);
        assert_eq!(injected_result.qos, MqttQos::ExactlyOnce);
        let injected_message = receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        assert_eq!(injected_message.payload, injected_payload);
        assert_eq!(
            injected_message.user_properties,
            vec![MqttUserProperty {
                name: "source".into(),
                value: "local-broker".into(),
            }]
        );

        command_sender
            .try_send(MqttSubscriptionCommand::Unsubscribe {
                filter: topic.clone(),
            })
            .map_err(|error| error.to_string())?;
        let status = status_receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        assert_eq!(status.state, MqttSubscriptionState::Pending);

        command_sender
            .try_send(MqttSubscriptionCommand::Subscribe(MqttSubscription {
                filter: topic.clone(),
                qos: MqttQos::AtLeastOnce,
                no_local: false,
            }))
            .map_err(|error| error.to_string())?;
        let status = status_receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        assert_eq!(status.state, MqttSubscriptionState::Subscribed);
        let retained_message = receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        assert_eq!(retained_message.payload, injected_payload);
        assert!(retained_message.retain);

        cancelled.store(true, Ordering::Release);
        smol::block_on(server.stop()).map_err(|error| error.to_string())?;
        subscription
            .join()
            .map_err(|_| "本地 MQTT 订阅线程不应 panic".to_string())?
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    #[cfg(feature = "native")]
    #[test]
    fn local_server_emits_lifecycle_and_publish_events() -> std::result::Result<(), String> {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{TrySendError, sync_channel},
        };
        use std::time::{Duration, Instant};
        use ramag_domain::entities::{
            MqttLocalServerEvent, MqttLocalServerEventSink, MqttLocalServerEventSinkResult,
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

        let event_cancelled = Arc::new(AtomicBool::new(false));
        let (event_sender, event_receiver) = sync_channel(64);
        let event_sink: MqttLocalServerEventSink = Arc::new(move |event| {
            match event_sender.try_send(event) {
                Ok(()) => MqttLocalServerEventSinkResult::Accepted,
                Err(TrySendError::Full(_)) => MqttLocalServerEventSinkResult::Backpressured,
                Err(TrySendError::Disconnected(_)) => MqttLocalServerEventSinkResult::Closed,
            }
        });
        let event_driver = server.clone();
        let event_cancelled_thread = event_cancelled.clone();
        let event_task = std::thread::spawn(move || {
            smol::block_on(event_driver.subscribe_events(event_sink, event_cancelled_thread))
        });

        let mut profile = MqttProfile::new("local-events", "127.0.0.1", port);
        profile.keep_alive_seconds = 5;
        let topic = format!("ramag/events/{}", std::process::id());
        let cancelled = Arc::new(AtomicBool::new(false));
        let (message_sender, _message_receiver) = sync_channel(1);
        let sink = Arc::new(move |_| {
            let _ = message_sender.try_send(());
            MqttMessageSinkResult::Accepted
        });
        let status_sink = Arc::new(|_| {});
        let (_command_sender, command_receiver) = async_channel::bounded(4);
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
                        no_local: false,
                    }],
                },
                sink,
                status_sink,
                command_receiver,
                subscribe_cancelled,
            ))
        });

        std::thread::sleep(Duration::from_millis(400));
        smol::block_on(NativeMqttTransport::new().publish(
            &profile,
            &MqttPublishRequest {
                topic: topic.clone(),
                payload: b"client event".to_vec(),
                qos: MqttQos::AtLeastOnce,
                retain: false,
                user_properties: Vec::new(),
            },
        ))
        .map_err(|error| format!("客户端发布失败：{error}"))?;
        smol::block_on(server.publish(&MqttPublishRequest {
            topic: topic.clone(),
            payload: b"broker event".to_vec(),
            qos: MqttQos::AtMostOnce,
            retain: false,
            user_properties: Vec::new(),
        }))
        .map_err(|error| format!("Broker 注入发布失败：{error}"))?;

        cancelled.store(true, Ordering::Release);
        subscription
            .join()
            .map_err(|_| "本地 MQTT 订阅线程不应 panic".to_string())?
            .map_err(|error| error.to_string())?;

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut connected = false;
        let mut subscribed = false;
        let mut client_published = false;
        let mut broker_published = false;
        let mut disconnected = false;
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let event = match event_receiver.recv_timeout(remaining.min(Duration::from_millis(250)))
            {
                Ok(event) => event,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(error) => return Err(error.to_string()),
            };
            match &event {
                MqttLocalServerEvent::ClientConnected { .. } => connected = true,
                MqttLocalServerEvent::ClientSubscribed { subscription, .. }
                    if subscription.filter == topic => subscribed = true,
                MqttLocalServerEvent::ClientPublished { message, .. }
                    if message.topic == topic => client_published = true,
                MqttLocalServerEvent::BrokerPublished { message, .. }
                    if message.topic == topic => broker_published = true,
                MqttLocalServerEvent::ClientDisconnected { .. } => disconnected = true,
                _ => {}
            }
            if connected && subscribed && client_published && broker_published && disconnected {
                break;
            }
        }

        event_cancelled.store(true, Ordering::Release);
        event_task
            .join()
            .map_err(|_| "本地 MQTT 事件线程不应 panic".to_string())?
            .map_err(|error| error.to_string())?;
        smol::block_on(server.stop()).map_err(|error| error.to_string())?;

        assert!(connected, "应产生客户端连接事件");
        assert!(subscribed, "应产生成功订阅事件");
        assert!(client_published, "应产生客户端发布事件");
        assert!(broker_published, "应产生 Broker 注入发布事件");
        assert!(disconnected, "应产生客户端断开事件");
        Ok(())
    }
