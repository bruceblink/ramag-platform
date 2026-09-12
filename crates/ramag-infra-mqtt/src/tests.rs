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
