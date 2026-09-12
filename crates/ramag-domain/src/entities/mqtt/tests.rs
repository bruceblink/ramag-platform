    use super::*;

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
