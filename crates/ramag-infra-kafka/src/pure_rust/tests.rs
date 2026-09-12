    use super::*;
    use ramag_domain::entities::{KafkaSaslMechanism, KafkaSecurityProtocol};

    #[test]
    fn capabilities_only_advertise_the_pure_rust_reading_slice() {
        let capabilities = PureRustTransport::new().capabilities();
        assert_eq!(capabilities.backend, KafkaTransportBackend::PureRust);
        assert!(capabilities.build_available);
        assert!(capabilities.fetch);
        assert!(capabilities.list_offsets);
        assert!(!capabilities.metadata);
        assert!(!capabilities.consumer_groups);
        assert!(!capabilities.topic_admin);
        assert!(!capabilities.tls);
        assert!(capabilities.sasl);
    }

    #[test]
    fn tls_is_rejected_before_network_access() {
        let mut config = KafkaClusterConfig::new("secure", vec!["broker:9093".into()]);
        config.security_protocol = KafkaSecurityProtocol::Ssl;
        let result = smol::block_on(PureRustTransport::new().test_connection(&config));
        assert!(matches!(
            result,
            Err(DomainError::Kafka(error)) if error.category == KafkaErrorCategory::Tls
        ));
    }

    #[test]
    fn unsupported_sasl_mechanisms_are_rejected_without_exposing_credentials() {
        let mut config = KafkaClusterConfig::new("secure", vec!["broker:9092".into()]);
        config.security_protocol = KafkaSecurityProtocol::SaslPlaintext;
        config.sasl_mechanism = Some(KafkaSaslMechanism::Gssapi);
        config.sasl_username = Some("user".into());
        config.sasl_password = Some("password".into());
        let result = smol::block_on(PureRustTransport::new().test_connection(&config));
        assert!(matches!(
            result,
            Err(DomainError::Kafka(error))
                if error.category == KafkaErrorCategory::Unsupported
                    && !error.safe_message.contains("password")
        ));
    }

    #[test]
    fn cancelled_requests_stop_before_creating_the_client() {
        let config = KafkaClusterConfig::new("local", vec!["broker:9092".into()]);
        let cancelled = Arc::new(AtomicBool::new(true));
        let result = smol::block_on(
            PureRustTransport::new().test_connection_with_cancel(&config, cancelled),
        );
        assert!(matches!(
            result,
            Err(DomainError::Kafka(error)) if error.category == KafkaErrorCategory::Cancelled
        ));
    }
