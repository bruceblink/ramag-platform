    use super::*;

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
    fn native_connection_reaches_live_broker() {
        let Some(host) = std::env::var_os("RAMAG_MQTT_LIVE_HOST") else {
            return;
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
                .expect("Native MQTT 应连接到显式配置的 Broker");
        });
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
