    use super::*;
    use crate::entities::MqttSubscription;

    #[test]
    fn mqtt_profile_defaults_are_valid_and_debug_redacts_secrets() {
        let mut profile = MqttProfile::new("local", "127.0.0.1", DEFAULT_MQTT_PORT);
        profile.username = Some("operator".into());
        profile.password = Some("secret-password".into());

        assert!(profile.validate().is_ok());
        let debug = format!("{profile:?}");
        assert!(!debug.contains("secret-password"));
        assert!(!debug.contains("operator"));
    }

    #[test]
    fn mqtt_profile_rejects_incomplete_auth_and_plaintext_tls_paths() {
        let mut profile = MqttProfile::new("local", "localhost", DEFAULT_MQTT_PORT);
        profile.password = Some("secret".into());
        assert!(profile.validate().is_err());

        profile.password = None;
        profile.tls.ca_cert_path = Some("ca.pem".into());
        assert!(profile.validate().is_err());

        profile.transport = MqttTransport::Tls;
        profile.tls.ca_cert_path = None;
        profile.tls.client_cert_path = Some("client.pem".into());
        assert!(profile.validate().is_err());

        profile.tls.client_key_path = Some("client.key".into());
        profile.keep_alive_seconds = 1;
        assert!(profile.validate().is_err());
    }

    #[test]
    fn topic_name_and_filter_rules_match_mqtt_wildcard_boundaries() {
        assert!(validate_mqtt_topic_name("devices/one/state").is_ok());
        assert!(validate_mqtt_topic_name("devices/+/state").is_err());
        assert!(validate_mqtt_topic_filter("devices/+/state").is_ok());
        assert!(validate_mqtt_topic_filter("devices/#").is_ok());
        assert!(validate_mqtt_topic_filter("devices/#/state").is_err());
        assert!(validate_mqtt_topic_filter("$share/workers/devices/#").is_ok());
        assert!(validate_mqtt_topic_filter("$share//devices/#").is_err());
    }

    #[test]
    fn subscription_no_local_defaults_off_for_legacy_records() {
        let result: Result<MqttSubscription, _> = serde_json::from_str(
            r#"{"filter":"devices/#","qos":"AtLeastOnce"}"#,
        );
        assert!(
            result
                .as_ref()
                .is_ok_and(|subscription| !subscription.no_local),
            "旧订阅记录应读取为关闭 No Local"
        );
        if let Ok(subscription) = result {
            assert!(subscription.validate().is_ok());
        }
    }

    #[test]
    fn mqtt_profile_persists_subscriptions_and_keeps_legacy_defaults()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut profile = MqttProfile::new("local", "localhost", DEFAULT_MQTT_PORT);
        profile.subscriptions = vec![MqttSubscription {
            filter: "devices/#".into(),
            qos: MqttQos::ExactlyOnce,
            no_local: true,
        }];
        assert!(profile.validate().is_ok());

        let mut legacy = serde_json::to_value(&profile)?;
        legacy
            .as_object_mut()
            .ok_or("配置应序列化为对象")?
            .remove("subscriptions");
        let decoded: MqttProfile = serde_json::from_value(legacy)?;
        assert_eq!(decoded.subscriptions, default_mqtt_subscriptions());

        profile.subscriptions.push(profile.subscriptions[0].clone());
        assert!(profile.validate().is_err(), "重复 Topic Filter 不能保存");
        Ok(())
    }

    #[test]
    fn mqtt_profile_rejects_no_local_for_mqtt311_subscriptions() {
        let mut profile = MqttProfile::new("local", "localhost", DEFAULT_MQTT_PORT);
        profile.protocol_version = MqttProtocolVersion::V311;
        profile.subscriptions[0].no_local = true;
        assert!(profile.validate().is_err());
    }

    #[test]
    fn local_server_defaults_are_valid_and_invalid_bindings_are_rejected() {
        let config = MqttLocalServerConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.bind_host, DEFAULT_MQTT_LOCAL_SERVER_HOST);
        assert!(!MqttLocalServerStatus::stopped(&config).running);

        let mut invalid = config.clone();
        invalid.bind_host = "mqtt://127.0.0.1".into();
        assert!(invalid.validate().is_err());
        invalid.bind_host = "127.0.0.1".into();
        invalid.port = 0;
        assert!(invalid.validate().is_err());

        let locked_out = MqttLocalServerConfig {
            allow_anonymous: false,
            ..config.clone()
        };
        assert!(locked_out.validate().is_err());

        let mut secured = MqttLocalServerConfig {
            allow_anonymous: false,
            users: vec![MqttLocalServerUser {
                username: "operator".into(),
                password: "secret".into(),
            }],
            ..config
        };
        assert!(secured.validate().is_ok());
        let debug = format!("{secured:?}");
        assert!(!debug.contains("secret"));
        secured.users.push(MqttLocalServerUser {
            username: "operator".into(),
            password: "other".into(),
        });
        assert!(secured.validate().is_err());
    }

    #[test]
    fn mosquitto_acl_and_role_validation_bounds_priority_and_acl_count() {
        let acl = MosquittoAcl {
            acl_type: MosquittoAclType::SubscribePattern,
            topic: "devices/%u/#".into(),
            decision: MosquittoAclDecision::Allow,
            priority: 10,
        };
        let role = MosquittoRole {
            role_name: "operator".into(),
            text_name: None,
            text_description: None,
            allow_wildcards_subscriptions: false,
            acls: vec![acl],
        };
        assert!(role.validate().is_ok());

        let invalid = MosquittoAcl {
            priority: 100_001,
            ..role.acls[0].clone()
        };
        assert!(invalid.validate().is_err());

        let unsupported = MosquittoAcl {
            acl_type: MosquittoAclType::Subscribe,
            topic: "devices/#".into(),
            decision: MosquittoAclDecision::Allow,
            priority: -1,
        };
        assert!(unsupported.validate().is_err());
    }
