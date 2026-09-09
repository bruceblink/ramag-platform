use std::sync::{Arc, Mutex};

use super::{
    KafkaService, validate_admin_request, validate_cluster_metadata,
    validate_consumer_group_assignment_budget, validate_consumer_group_member_budget,
    validate_consumer_group_offset_budget, validate_consumer_groups, validate_message_page,
    validate_replica_id_budget, validate_topic_partition_budget, validate_topics,
};
use async_trait::async_trait;
use ramag_domain::entities::{
    KafkaBroker, KafkaBrokerMetricsSnapshot, KafkaBrokerRuntimeMetrics, KafkaClusterConfig,
    KafkaClusterMetadata, KafkaConfigEntry, KafkaConfigResource, KafkaConfigResourceType,
    KafkaConfigSource, KafkaConfigUpdateRequest, KafkaConsumerGroup,
    KafkaConsumerGroupOffsetResetRequest, KafkaMessagePage, KafkaMessageProduceRequest,
    KafkaMessageProduceResult, KafkaMessageRecord, KafkaMetricsSnapshot, KafkaMetricsSnapshotState,
    KafkaMetricsSource, KafkaPartition, KafkaReadOnlyState, KafkaTopic, KafkaTopicCreateRequest,
    KafkaTopicPartitionExpansion,
};
use ramag_domain::error::{DomainError, KafkaError, KafkaErrorCategory, Result};
use ramag_domain::traits::{
    KafkaAdminDriver, KafkaBrokerMetricsDriver, KafkaDriver, KafkaMonitoringDriver,
    KafkaProducerDriver, Storage,
};

struct NoopKafkaDriver;

#[async_trait]
impl KafkaDriver for NoopKafkaDriver {
    async fn test_connection(&self, _config: &KafkaClusterConfig) -> Result<()> {
        Ok(())
    }
}

struct NoopStorage;

#[async_trait]
impl Storage for NoopStorage {
    async fn list_connections(&self) -> Result<Vec<ramag_domain::entities::ConnectionConfig>> {
        Ok(Vec::new())
    }

    async fn get_connection(
        &self,
        _id: &ramag_domain::entities::ConnectionId,
    ) -> Result<Option<ramag_domain::entities::ConnectionConfig>> {
        Ok(None)
    }

    async fn save_connection(
        &self,
        _config: &ramag_domain::entities::ConnectionConfig,
    ) -> Result<()> {
        Ok(())
    }

    async fn delete_connection(&self, _id: &ramag_domain::entities::ConnectionId) -> Result<()> {
        Ok(())
    }

    async fn append_history(&self, _record: &ramag_domain::entities::QueryRecord) -> Result<()> {
        Ok(())
    }

    async fn list_history(
        &self,
        _connection_id: Option<&ramag_domain::entities::ConnectionId>,
        _limit: usize,
    ) -> Result<Vec<ramag_domain::entities::QueryRecord>> {
        Ok(Vec::new())
    }

    async fn delete_history(&self, _id: &ramag_domain::entities::QueryRecordId) -> Result<()> {
        Ok(())
    }

    async fn clear_history(
        &self,
        _connection_id: Option<&ramag_domain::entities::ConnectionId>,
    ) -> Result<()> {
        Ok(())
    }

    async fn get_preference(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn set_preference(&self, _key: &str, _value: &str) -> Result<()> {
        Ok(())
    }
}

struct RecordingAdminDriver {
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl KafkaAdminDriver for RecordingAdminDriver {
    async fn create_topic(
        &self,
        _config: &KafkaClusterConfig,
        request: &KafkaTopicCreateRequest,
    ) -> Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("create:{}", request.name));
        Ok(())
    }

    async fn delete_topic(&self, _config: &KafkaClusterConfig, topic: &str) -> Result<()> {
        self.calls.lock().unwrap().push(format!("delete:{topic}"));
        Ok(())
    }

    async fn increase_topic_partitions(
        &self,
        _config: &KafkaClusterConfig,
        request: &KafkaTopicPartitionExpansion,
    ) -> Result<()> {
        self.calls.lock().unwrap().push(format!(
            "expand:{}:{}",
            request.name, request.total_partitions
        ));
        Ok(())
    }

    async fn describe_configs(
        &self,
        _config: &KafkaClusterConfig,
        resource_type: KafkaConfigResourceType,
        resource_name: &str,
    ) -> Result<KafkaConfigResource> {
        self.calls.lock().unwrap().push(format!(
            "describe:{}:{resource_name}",
            resource_type.label()
        ));
        Ok(KafkaConfigResource {
            resource_type,
            resource_name: resource_name.into(),
            entries: vec![KafkaConfigEntry {
                key: "retention.ms".into(),
                value: Some("60000".into()),
                source: KafkaConfigSource::DynamicTopic,
                is_read_only: false,
                is_default: false,
                is_sensitive: false,
            }],
        })
    }

    async fn update_config(
        &self,
        _config: &KafkaClusterConfig,
        request: &KafkaConfigUpdateRequest,
    ) -> Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("update:{}", request.key));
        Ok(())
    }

    async fn reset_consumer_group_offsets(
        &self,
        _config: &KafkaClusterConfig,
        request: &KafkaConsumerGroupOffsetResetRequest,
    ) -> Result<()> {
        self.calls.lock().unwrap().push(format!(
            "reset:{}:{}",
            request.group_id,
            request.offsets.len()
        ));
        Ok(())
    }
}

struct FailingAdminDriver;

#[async_trait]
impl KafkaAdminDriver for FailingAdminDriver {
    async fn create_topic(
        &self,
        _config: &KafkaClusterConfig,
        _request: &KafkaTopicCreateRequest,
    ) -> Result<()> {
        Err(DomainError::Kafka(KafkaError::new(
            KafkaErrorCategory::PermissionDenied,
            "create_topic",
            "Broker 拒绝 Topic 创建",
        )))
    }
}

struct RecordingProducerDriver {
    calls: Arc<Mutex<Vec<KafkaMessageProduceRequest>>>,
}

#[async_trait]
impl KafkaProducerDriver for RecordingProducerDriver {
    async fn produce_message(
        &self,
        _config: &KafkaClusterConfig,
        request: &KafkaMessageProduceRequest,
    ) -> Result<KafkaMessageProduceResult> {
        self.calls.lock().unwrap().push(request.clone());
        Ok(KafkaMessageProduceResult::new(
            request.topic.clone(),
            request.partition.unwrap_or(0),
            12,
            None,
        ))
    }
}

struct MismatchedProducerDriver;

#[async_trait]
impl KafkaProducerDriver for MismatchedProducerDriver {
    async fn produce_message(
        &self,
        _config: &KafkaClusterConfig,
        _request: &KafkaMessageProduceRequest,
    ) -> Result<KafkaMessageProduceResult> {
        Ok(KafkaMessageProduceResult::new("other-topic", 0, 1, None))
    }
}

struct RecordingMonitoringDriver {
    snapshot: KafkaMetricsSnapshot,
}

#[async_trait]
impl KafkaMonitoringDriver for RecordingMonitoringDriver {
    async fn metrics_snapshot(&self, _config: &KafkaClusterConfig) -> Result<KafkaMetricsSnapshot> {
        Ok(self.snapshot.clone())
    }
}

struct RecordingBrokerMetricsDriver {
    snapshot: KafkaBrokerMetricsSnapshot,
}

#[async_trait]
impl KafkaBrokerMetricsDriver for RecordingBrokerMetricsDriver {
    async fn broker_metrics_snapshot(
        &self,
        _config: &KafkaClusterConfig,
    ) -> Result<KafkaBrokerMetricsSnapshot> {
        Ok(self.snapshot.clone())
    }
}

fn service_with_admin(admin: Arc<dyn KafkaAdminDriver>) -> KafkaService {
    KafkaService::new(Arc::new(NoopKafkaDriver), Arc::new(NoopStorage)).with_admin_driver(admin)
}

fn service_with_producer(producer: Arc<dyn KafkaProducerDriver>) -> KafkaService {
    KafkaService::new(Arc::new(NoopKafkaDriver), Arc::new(NoopStorage))
        .with_producer_driver(producer)
}

#[test]
fn metrics_service_forwards_and_validates_snapshot() {
    let config = KafkaClusterConfig::new("metrics", vec!["localhost:9092".into()]);
    let metadata = KafkaClusterMetadata {
        cluster_id: Some("cluster-a".into()),
        controller_id: Some(0),
        brokers: vec![KafkaBroker {
            id: 0,
            host: "localhost".into(),
            port: 9092,
            rack: None,
            version: None,
            is_controller: true,
        }],
        kafka_version: None,
    };
    let topic = KafkaTopic {
        name: "events".into(),
        partitions: vec![KafkaPartition {
            id: 0,
            leader: Some(0),
            replicas: vec![0],
            isr: vec![0],
            low_watermark: Some(0),
            high_watermark: Some(10),
        }],
        internal: false,
    };
    let snapshot = KafkaMetricsSnapshot::from_runtime(chrono::Utc::now(), &metadata, &[topic], &[]);
    let service = KafkaService::new(Arc::new(NoopKafkaDriver), Arc::new(NoopStorage))
        .with_monitoring_driver(Arc::new(RecordingMonitoringDriver {
            snapshot: snapshot.clone(),
        }));
    let result = smol::block_on(service.metrics_snapshot(&config)).expect("valid metrics snapshot");
    assert_eq!(result, snapshot);

    let mut invalid = snapshot;
    invalid.topics.push(invalid.topics[0].clone());
    let invalid_service = KafkaService::new(Arc::new(NoopKafkaDriver), Arc::new(NoopStorage))
        .with_monitoring_driver(Arc::new(RecordingMonitoringDriver { snapshot: invalid }));
    assert!(matches!(
        smol::block_on(invalid_service.metrics_snapshot(&config)),
        Err(DomainError::InvalidConfig(message)) if message.contains("重复")
    ));
}

#[test]
fn broker_metrics_service_keeps_external_source_separate() {
    let config = KafkaClusterConfig::new("metrics", vec!["localhost:9092".into()]);
    let snapshot = KafkaBrokerMetricsSnapshot {
        cluster_id: None,
        sampled_at: chrono::Utc::now(),
        source: KafkaMetricsSource::ExternalBrokerMetrics,
        state: KafkaMetricsSnapshotState::Ready,
        error: None,
        brokers: vec![KafkaBrokerRuntimeMetrics {
            broker_id: 0,
            cpu_usage_percent: Some(10.0),
            memory_used_bytes: Some(20.0),
            disk_used_bytes: Some(30.0),
            request_latency_ms: Some(2.0),
        }],
    };
    let service = KafkaService::new(Arc::new(NoopKafkaDriver), Arc::new(NoopStorage))
        .with_broker_metrics_driver(Arc::new(RecordingBrokerMetricsDriver {
            snapshot: snapshot.clone(),
        }));
    let result = smol::block_on(service.broker_metrics_snapshot(&config))
        .expect("valid external broker metrics snapshot");
    assert_eq!(result, snapshot);

    let invalid_service = KafkaService::new(Arc::new(NoopKafkaDriver), Arc::new(NoopStorage))
        .with_broker_metrics_driver(Arc::new(RecordingBrokerMetricsDriver {
            snapshot: KafkaBrokerMetricsSnapshot {
                source: KafkaMetricsSource::KafkaProtocol,
                ..snapshot
            },
        }));
    assert!(matches!(
        smol::block_on(invalid_service.broker_metrics_snapshot(&config)),
        Err(DomainError::InvalidConfig(message)) if message.contains("外部 Broker")
    ));
}

#[test]
fn service_exposes_transport_capabilities_without_client_types() {
    let service = service_with_admin(Arc::new(RecordingAdminDriver {
        calls: Arc::new(Mutex::new(Vec::new())),
    }));
    let capabilities = service.transport_capabilities();
    assert_eq!(
        capabilities.backend,
        ramag_domain::entities::KafkaTransportBackend::TestDouble
    );
    assert!(!capabilities.build_available);
    assert!(!capabilities.metadata);
}

#[test]
fn offset_reset_requires_admin_mode_and_forwards_explicit_targets() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let service = service_with_admin(Arc::new(RecordingAdminDriver {
        calls: calls.clone(),
    }));
    let mut config = KafkaClusterConfig::new("offset-reset", vec!["localhost:9092".into()]);
    let request = KafkaConsumerGroupOffsetResetRequest::new(
        "workers",
        vec![ramag_domain::entities::KafkaConsumerGroupOffsetReset::new(
            "events", 0, 7,
        )],
    );
    assert!(matches!(
        smol::block_on(service.reset_consumer_group_offsets(&config, &request)),
        Err(DomainError::Forbidden(_))
    ));

    config.read_only = KafkaReadOnlyState::ReadWrite;
    smol::block_on(service.reset_consumer_group_offsets(&config, &request))
        .expect("admin mode should forward offset reset");
    assert_eq!(calls.lock().unwrap().as_slice(), &["reset:workers:1"]);
}

#[test]
fn message_production_requires_admin_mode_and_forwards_request() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let service = service_with_producer(Arc::new(RecordingProducerDriver {
        calls: calls.clone(),
    }));
    let mut config = KafkaClusterConfig::new("message-produce", vec!["localhost:9092".into()]);
    let request = KafkaMessageProduceRequest::new("events", b"payload".to_vec())
        .with_partition(2)
        .with_key(b"key".to_vec())
        .with_headers(vec![ramag_domain::entities::KafkaMessageHeader {
            key: "trace-id".into(),
            value: Some(b"abc".to_vec()),
        }]);

    assert!(matches!(
        smol::block_on(service.produce_message(&config, &request)),
        Err(DomainError::Forbidden(message)) if message == ramag_domain::error::READ_ONLY_MESSAGE
    ));
    assert!(calls.lock().unwrap().is_empty());

    config.read_only = KafkaReadOnlyState::ReadWrite;
    let result = smol::block_on(service.produce_message(&config, &request))
        .expect("admin mode should forward message production");
    assert_eq!(result.topic, "events");
    assert_eq!(result.partition, 2);
    assert_eq!(result.offset, 12);
    assert_eq!(calls.lock().unwrap().as_slice(), &[request]);
}

#[test]
fn message_production_rejects_driver_result_for_another_topic() {
    let service = service_with_producer(Arc::new(MismatchedProducerDriver));
    let mut config = KafkaClusterConfig::new("message-produce", vec!["localhost:9092".into()]);
    config.read_only = KafkaReadOnlyState::ReadWrite;
    let request = KafkaMessageProduceRequest::new("events", b"payload".to_vec());

    let result = smol::block_on(service.produce_message(&config, &request));
    assert!(matches!(
        result,
        Err(DomainError::InvalidConfig(message)) if message.contains("Topic")
    ));
}

#[test]
fn application_boundary_rejects_invalid_driver_snapshots() {
    let metadata = KafkaClusterMetadata {
        cluster_id: None,
        controller_id: None,
        brokers: Vec::new(),
        kafka_version: None,
    };
    assert!(validate_cluster_metadata(metadata).is_err());

    let topic = KafkaTopic {
        name: "events".into(),
        partitions: vec![KafkaPartition {
            id: 0,
            leader: Some(0),
            replicas: vec![0],
            isr: vec![0],
            low_watermark: Some(0),
            high_watermark: Some(1),
        }],
        internal: false,
    };
    assert!(validate_topics(vec![topic.clone(), topic]).is_err());

    let page = KafkaMessagePage {
        records: vec![KafkaMessageRecord {
            topic: "events".into(),
            partition: 0,
            offset: 0,
            timestamp: None,
            key: None,
            value: Some(b"event".to_vec()),
            headers: Vec::new(),
        }],
        scanned_records: 0,
        scanned_bytes: 0,
        truncated: false,
    };
    assert!(validate_message_page(page).is_err());
}

#[test]
fn application_boundary_bounds_total_topic_partitions() {
    let mut total = ramag_domain::entities::MAX_KAFKA_PARTITIONS - 1;
    assert!(validate_topic_partition_budget(&mut total, 1).is_ok());
    assert_eq!(total, ramag_domain::entities::MAX_KAFKA_PARTITIONS);
    assert!(validate_topic_partition_budget(&mut total, 1).is_err());
}

#[test]
fn application_boundary_bounds_total_partition_replica_ids() {
    let mut total = ramag_domain::entities::MAX_KAFKA_PARTITION_REPLICA_IDS - 2;
    assert!(validate_replica_id_budget(&mut total, 2).is_ok());
    assert_eq!(
        total,
        ramag_domain::entities::MAX_KAFKA_PARTITION_REPLICA_IDS
    );
    assert!(validate_replica_id_budget(&mut total, 1).is_err());
}

#[test]
fn application_boundary_bounds_total_consumer_group_offsets() {
    let mut total = ramag_domain::entities::MAX_KAFKA_GROUP_OFFSETS - 1;
    assert!(validate_consumer_group_offset_budget(&mut total, 1).is_ok());
    assert_eq!(total, ramag_domain::entities::MAX_KAFKA_GROUP_OFFSETS);
    assert!(validate_consumer_group_offset_budget(&mut total, 1).is_err());
}

#[test]
fn application_boundary_bounds_total_consumer_group_members_and_assignments() {
    let mut members = ramag_domain::entities::MAX_KAFKA_GROUP_TOTAL_MEMBERS - 1;
    assert!(validate_consumer_group_member_budget(&mut members, 1).is_ok());
    assert_eq!(
        members,
        ramag_domain::entities::MAX_KAFKA_GROUP_TOTAL_MEMBERS
    );
    assert!(validate_consumer_group_member_budget(&mut members, 1).is_err());

    let mut assignments = ramag_domain::entities::MAX_KAFKA_GROUP_TOTAL_ASSIGNMENTS - 1;
    assert!(validate_consumer_group_assignment_budget(&mut assignments, 1).is_ok());
    assert_eq!(
        assignments,
        ramag_domain::entities::MAX_KAFKA_GROUP_TOTAL_ASSIGNMENTS
    );
    assert!(validate_consumer_group_assignment_budget(&mut assignments, 1).is_err());
}

#[test]
fn application_boundary_accepts_valid_metadata() {
    let metadata = KafkaClusterMetadata {
        cluster_id: Some("cluster".into()),
        controller_id: Some(0),
        brokers: vec![KafkaBroker {
            id: 0,
            host: "broker".into(),
            port: 9092,
            rack: None,
            version: None,
            is_controller: true,
        }],
        kafka_version: None,
    };
    assert!(validate_cluster_metadata(metadata).is_ok());
}

#[test]
fn application_boundary_rejects_duplicate_consumer_group_ids() {
    let group = KafkaConsumerGroup {
        group_id: "workers".into(),
        state: Some("Stable".into()),
        protocol: Some("range".into()),
        members: Vec::new(),
        offsets: Vec::new(),
    };
    assert!(validate_consumer_groups(vec![group.clone(), group]).is_err());
}

#[test]
fn topic_admin_boundary_requires_explicit_read_write_mode() {
    let mut config = KafkaClusterConfig::new("local", vec!["localhost:9092".into()]);
    let request = KafkaTopicCreateRequest::new("events", 1, 1);
    let result = validate_admin_request(&config, request.validate());
    assert!(matches!(
        result,
        Err(DomainError::Forbidden(message)) if message == ramag_domain::error::READ_ONLY_MESSAGE
    ));

    config.read_only = KafkaReadOnlyState::ReadWrite;
    assert!(validate_admin_request(&config, request.validate()).is_ok());
}

#[test]
fn topic_admin_service_blocks_all_writes_until_mode_is_enabled() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let service = service_with_admin(Arc::new(RecordingAdminDriver {
        calls: calls.clone(),
    }));
    let config = KafkaClusterConfig::new("local", vec!["localhost:9092".into()]);
    let create = KafkaTopicCreateRequest::new("events", 1, 1);
    let expand = KafkaTopicPartitionExpansion::new("events", 2);

    assert!(matches!(
        smol::block_on(service.create_topic(&config, &create)),
        Err(DomainError::Forbidden(message)) if message == ramag_domain::error::READ_ONLY_MESSAGE
    ));
    assert!(matches!(
        smol::block_on(service.delete_topic(&config, "events")),
        Err(DomainError::Forbidden(message)) if message == ramag_domain::error::READ_ONLY_MESSAGE
    ));
    assert!(matches!(
        smol::block_on(service.increase_topic_partitions(&config, &expand)),
        Err(DomainError::Forbidden(message)) if message == ramag_domain::error::READ_ONLY_MESSAGE
    ));
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn topic_admin_service_forwards_valid_operations_and_preserves_driver_errors() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let service = service_with_admin(Arc::new(RecordingAdminDriver {
        calls: calls.clone(),
    }));
    let mut config = KafkaClusterConfig::new("local", vec!["localhost:9092".into()]);
    config.read_only = KafkaReadOnlyState::ReadWrite;
    let create = KafkaTopicCreateRequest::new("events", 1, 1);
    let expand = KafkaTopicPartitionExpansion::new("events", 2);

    assert!(smol::block_on(service.create_topic(&config, &create)).is_ok());
    assert!(smol::block_on(service.delete_topic(&config, "events")).is_ok());
    assert!(smol::block_on(service.increase_topic_partitions(&config, &expand)).is_ok());
    assert_eq!(
        &*calls.lock().unwrap(),
        &["create:events", "delete:events", "expand:events:2"]
    );

    let failing = service_with_admin(Arc::new(FailingAdminDriver));
    let result = smol::block_on(failing.create_topic(&config, &create));
    assert!(matches!(
        result,
        Err(DomainError::Kafka(error))
            if error.category == KafkaErrorCategory::PermissionDenied
                && error.operation == "create_topic"
    ));
}

#[test]
fn config_service_allows_reads_in_read_only_mode_but_blocks_updates() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let service = service_with_admin(Arc::new(RecordingAdminDriver {
        calls: calls.clone(),
    }));
    let config = KafkaClusterConfig::new("local", vec!["localhost:9092".into()]);
    let resource =
        smol::block_on(service.describe_configs(&config, KafkaConfigResourceType::Topic, "events"))
            .expect("read-only mode should allow config reads");
    assert_eq!(resource.entries[0].key, "retention.ms");
    assert_eq!(&*calls.lock().unwrap(), &["describe:Topic:events"]);

    let request = KafkaConfigUpdateRequest::set(
        KafkaConfigResourceType::Topic,
        "events",
        "retention.ms",
        "120000",
    );
    let result = smol::block_on(service.update_config(&config, &request));
    assert!(matches!(
        result,
        Err(DomainError::Forbidden(message)) if message == ramag_domain::error::READ_ONLY_MESSAGE
    ));
    assert_eq!(&*calls.lock().unwrap(), &["describe:Topic:events"]);
}

#[test]
fn config_service_forwards_valid_update_after_admin_mode_is_enabled() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let service = service_with_admin(Arc::new(RecordingAdminDriver {
        calls: calls.clone(),
    }));
    let mut config = KafkaClusterConfig::new("local", vec!["localhost:9092".into()]);
    config.read_only = KafkaReadOnlyState::ReadWrite;
    let request = KafkaConfigUpdateRequest::set(
        KafkaConfigResourceType::Topic,
        "events",
        "retention.ms",
        "120000",
    );
    assert!(smol::block_on(service.update_config(&config, &request)).is_ok());
    assert_eq!(&*calls.lock().unwrap(), &["update:retention.ms"]);
}
