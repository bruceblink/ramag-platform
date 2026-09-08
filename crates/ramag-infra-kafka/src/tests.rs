#[cfg(feature = "cmake-build")]
use super::consumer_groups::decode_member_assignment;
#[cfg(feature = "cmake-build")]
use super::consumer_groups::{validate_group_assignment_budget, validate_group_member_budget};
use super::*;
#[cfg(feature = "cmake-build")]
use ramag_domain::entities::{
    KafkaAclFilter, KafkaConfigResourceType, MAX_KAFKA_GROUP_ASSIGNMENT_BYTES,
    MAX_KAFKA_GROUP_TOTAL_ASSIGNMENTS, MAX_KAFKA_GROUP_TOTAL_MEMBERS,
};
#[cfg(feature = "cmake-build")]
use ramag_domain::entities::{KafkaMessageQuery, KafkaMessageSearchQuery};
use ramag_domain::error::KafkaErrorCategory;
#[cfg(not(feature = "cmake-build"))]
use ramag_domain::traits::KafkaMonitoringDriver;
use ramag_domain::traits::KafkaTransport;
#[cfg(feature = "cmake-build")]
use ramag_domain::traits::{KafkaAdminDriver, KafkaDriver};
#[cfg(feature = "cmake-build")]
use std::sync::{Arc, atomic::AtomicBool};

#[test]
fn transport_capabilities_follow_compiled_features() {
    let capabilities = RdkafkaDriver::new().capabilities();
    assert_eq!(
        capabilities.backend,
        ramag_domain::entities::KafkaTransportBackend::NativeRdkafka
    );
    assert_eq!(capabilities.build_available, cfg!(feature = "cmake-build"));
    assert_eq!(
        capabilities.tls,
        cfg!(feature = "cmake-build") && cfg!(feature = "kafka-tls")
    );
    assert_eq!(
        capabilities.sasl,
        cfg!(feature = "cmake-build") && cfg!(feature = "kafka-sasl")
    );
    assert_eq!(capabilities.metadata, capabilities.build_available);
    assert_eq!(capabilities.acl_admin, capabilities.build_available);
    assert_eq!(capabilities.metrics_snapshot, capabilities.build_available);
}

#[test]
fn request_timeout_must_be_positive() {
    assert!(RdkafkaDriver::with_request_timeout(Duration::ZERO).is_err());
    assert!(RdkafkaDriver::with_request_timeout(Duration::from_nanos(1)).is_err());
    assert!(RdkafkaDriver::with_request_timeout(Duration::from_millis(1)).is_ok());
}

#[test]
fn async_connection_test_rejects_invalid_config_before_network() {
    let config = KafkaClusterConfig::new("invalid", vec!["localhost".into()]);
    let driver = RdkafkaDriver::new();
    let result = smol::block_on(driver.test_connection(&config));
    assert!(result.is_err());
    let error = match result {
        Ok(()) => return,
        Err(error) => error,
    };
    assert!(matches!(error, DomainError::InvalidConfig(_)));
}

#[cfg(not(feature = "kafka-tls"))]
#[test]
fn tls_connection_requires_build_feature() {
    let mut config = KafkaClusterConfig::new("secure", vec!["broker:9093".into()]);
    config.security_protocol = ramag_domain::entities::KafkaSecurityProtocol::Ssl;
    let result = RdkafkaDriver::new().test_connection_blocking(&config);
    assert!(result.is_err());
    let error = match result {
        Ok(()) => return,
        Err(error) => error,
    };
    assert!(matches!(
        error,
        DomainError::Kafka(error) if error.category == KafkaErrorCategory::Tls
    ));
}

#[cfg(not(feature = "kafka-sasl"))]
#[test]
fn sasl_connection_requires_build_feature() {
    let mut config = KafkaClusterConfig::new("secure", vec!["broker:9092".into()]);
    config.security_protocol = ramag_domain::entities::KafkaSecurityProtocol::SaslPlaintext;
    config.sasl_mechanism = Some(ramag_domain::entities::KafkaSaslMechanism::Plain);
    config.sasl_username = Some("user".into());
    config.sasl_password = Some("password".into());
    let result = RdkafkaDriver::new().test_connection_blocking(&config);
    assert!(result.is_err());
    let error = match result {
        Ok(()) => return,
        Err(error) => error,
    };
    assert!(matches!(
        error,
        DomainError::Kafka(error) if error.category == KafkaErrorCategory::Unsupported
    ));
}

#[cfg(feature = "cmake-build")]
#[test]
fn connection_error_category_is_preserved() {
    let error = errors::map_kafka_error(
        rdkafka::error::KafkaError::MetadataFetch(
            rdkafka::error::RDKafkaErrorCode::SaslAuthenticationFailed,
        ),
        "测试 Kafka 连接",
    );
    assert!(matches!(
        error,
        DomainError::Kafka(ref error)
            if error.category == KafkaErrorCategory::Authentication
                && error.operation == "测试 Kafka 连接"
    ));
}

#[cfg(not(feature = "cmake-build"))]
#[test]
fn default_build_reports_missing_native_client_without_network_access() {
    let config = KafkaClusterConfig::new("local", vec!["broker:9092".into()]);
    let result = RdkafkaDriver::new().test_connection_blocking(&config);
    assert!(matches!(
        result,
        Err(DomainError::Kafka(error))
            if error.category == KafkaErrorCategory::Unsupported
                && error.safe_message.contains("cmake-build")
    ));
}

#[cfg(not(feature = "cmake-build"))]
#[test]
fn default_build_reports_missing_metrics_client_without_network_access() {
    let config = KafkaClusterConfig::new("local", vec!["broker:9092".into()]);
    let result = smol::block_on(RdkafkaDriver::new().metrics_snapshot(&config));
    assert!(matches!(
        result,
        Err(DomainError::Kafka(error))
            if error.category == KafkaErrorCategory::Unsupported
                && error.safe_message.contains("cmake-build")
    ));
}

#[cfg(feature = "cmake-build")]
#[test]
fn metrics_snapshot_rejects_invalid_config_before_network() {
    let config = KafkaClusterConfig::new("invalid", vec!["localhost".into()]);
    let result = smol::block_on(RdkafkaDriver::new().metrics_snapshot(&config));
    assert!(matches!(result, Err(DomainError::InvalidConfig(_))));
}

#[cfg(feature = "cmake-build")]
#[test]
fn topic_partition_budget_is_bounded_across_topics() {
    let mut total = MAX_KAFKA_PARTITIONS - 1;
    assert!(validate_partition_budget(&mut total, "events", 1).is_ok());
    assert_eq!(total, MAX_KAFKA_PARTITIONS);
    assert!(validate_partition_budget(&mut total, "payments", 1).is_err());

    let mut total = 0;
    assert!(validate_partition_budget(&mut total, "oversized", MAX_KAFKA_PARTITIONS + 1).is_err());
}

#[cfg(feature = "cmake-build")]
#[test]
fn topic_replica_id_budget_is_bounded_across_partitions() {
    let mut total = MAX_KAFKA_PARTITION_REPLICA_IDS - 2;
    assert!(validate_partition_replica_budget(&mut total, "events", 0, 1, 1).is_ok());
    assert_eq!(total, MAX_KAFKA_PARTITION_REPLICA_IDS);
    assert!(validate_partition_replica_budget(&mut total, "events", 1, 1, 1).is_err());
    assert!(
        validate_partition_replica_budget(&mut 0, "events", 2, MAX_KAFKA_REPLICAS + 1, 1).is_err()
    );
}

#[cfg(feature = "cmake-build")]
#[test]
fn consumer_group_nested_budgets_are_exported() {
    let mut members = MAX_KAFKA_GROUP_TOTAL_MEMBERS - 1;
    assert!(validate_group_member_budget(&mut members, 1).is_ok());
    assert_eq!(members, MAX_KAFKA_GROUP_TOTAL_MEMBERS);
    assert!(validate_group_member_budget(&mut members, 1).is_err());

    let mut assignments = MAX_KAFKA_GROUP_TOTAL_ASSIGNMENTS - 1;
    assert!(validate_group_assignment_budget(&mut assignments, 1).is_ok());
    assert_eq!(assignments, MAX_KAFKA_GROUP_TOTAL_ASSIGNMENTS);
    assert!(validate_group_assignment_budget(&mut assignments, 1).is_err());
}

#[cfg(feature = "cmake-build")]
#[test]
fn consumer_assignment_decoder_accepts_valid_payload_and_rejects_malformed_data() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&1_i16.to_be_bytes());
    payload.extend_from_slice(&1_i32.to_be_bytes());
    payload.extend_from_slice(&6_i16.to_be_bytes());
    payload.extend_from_slice(b"events");
    payload.extend_from_slice(&2_i32.to_be_bytes());
    payload.extend_from_slice(&0_i32.to_be_bytes());
    payload.extend_from_slice(&3_i32.to_be_bytes());
    payload.extend_from_slice(&(-1_i32).to_be_bytes());
    let assignments = decode_member_assignment(&payload);
    assert_eq!(assignments.len(), 2);
    assert_eq!(assignments[0].topic, "events");
    assert_eq!(assignments[1].partition, 3);

    let mut truncated = payload.clone();
    truncated.pop();
    assert!(decode_member_assignment(&truncated).is_empty());

    let oversized = vec![0_u8; MAX_KAFKA_GROUP_ASSIGNMENT_BYTES + 1];
    assert!(decode_member_assignment(&oversized).is_empty());
}

#[cfg(feature = "cmake-build")]
#[test]
fn cancelled_message_reads_stop_before_creating_a_consumer() {
    let config = KafkaClusterConfig::new("local", vec!["broker:9092".into()]);
    let query = KafkaMessageQuery::by_offset("events", vec![0], 0, Some(1));
    let cancelled = Arc::new(AtomicBool::new(true));
    let result = smol::block_on(RdkafkaDriver::new().read_messages_with_cancel(
        &config,
        &query,
        cancelled.clone(),
    ));
    assert!(matches!(
        result,
        Err(DomainError::Kafka(error)) if error.category == KafkaErrorCategory::Cancelled
    ));

    let search = KafkaMessageSearchQuery::new("event", query);
    let result = smol::block_on(
        RdkafkaDriver::new().search_messages_with_cancel(&config, &search, cancelled),
    );
    assert!(matches!(
        result,
        Err(DomainError::Kafka(error)) if error.category == KafkaErrorCategory::Cancelled
    ));
}

#[cfg(feature = "cmake-build")]
#[test]
fn cancelled_consumer_group_reads_stop_before_creating_a_consumer() {
    let config = KafkaClusterConfig::new("local", vec!["broker:9092".into()]);
    let cancelled = Arc::new(AtomicBool::new(true));
    let result =
        smol::block_on(RdkafkaDriver::new().list_consumer_groups_with_cancel(&config, cancelled));
    assert!(matches!(
        result,
        Err(DomainError::Kafka(error)) if error.category == KafkaErrorCategory::Cancelled
    ));
}

#[cfg(feature = "cmake-build")]
#[test]
fn cancelled_runtime_reads_stop_before_creating_a_consumer() {
    let config = KafkaClusterConfig::new("local", vec!["broker:9092".into()]);
    let cancelled = Arc::new(AtomicBool::new(true));
    let result = smol::block_on(
        RdkafkaDriver::new().cluster_metadata_with_cancel(&config, cancelled.clone()),
    );
    assert!(matches!(
        result,
        Err(DomainError::Kafka(error)) if error.category == KafkaErrorCategory::Cancelled
    ));

    let result = smol::block_on(RdkafkaDriver::new().list_topics_with_cancel(&config, cancelled));
    assert!(matches!(
        result,
        Err(DomainError::Kafka(error)) if error.category == KafkaErrorCategory::Cancelled
    ));
}

#[cfg(feature = "cmake-build")]
#[test]
fn cancelled_acl_reads_stop_before_creating_an_admin_client() {
    let config = KafkaClusterConfig::new("local", vec!["broker:9092".into()]);
    let cancelled = Arc::new(AtomicBool::new(true));
    let result = smol::block_on(RdkafkaDriver::new().list_acls_with_cancel(
        &config,
        &KafkaAclFilter::default(),
        cancelled,
    ));
    assert!(matches!(
        result,
        Err(DomainError::Kafka(error)) if error.category == KafkaErrorCategory::Cancelled
    ));
}

#[cfg(feature = "cmake-build")]
#[test]
fn cancelled_config_reads_stop_before_creating_an_admin_client() {
    let config = KafkaClusterConfig::new("local", vec!["broker:9092".into()]);
    let cancelled = Arc::new(AtomicBool::new(true));
    let result = smol::block_on(RdkafkaDriver::new().describe_configs_with_cancel(
        &config,
        KafkaConfigResourceType::Topic,
        "events",
        cancelled,
    ));
    assert!(matches!(
        result,
        Err(DomainError::Kafka(error)) if error.category == KafkaErrorCategory::Cancelled
    ));
}
