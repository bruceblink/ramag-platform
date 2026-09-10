#![cfg(feature = "cmake-build")]
#![allow(clippy::panic)]

use chrono::{Duration, Utc};
use ramag_domain::entities::{
    KafkaClusterConfig, KafkaConsumerGroupOffsetReset, KafkaConsumerGroupOffsetResetRequest,
    KafkaMessageHeader, KafkaMessageProduceRequest, KafkaMessageQuery, KafkaMessageSearchField,
    KafkaMessageSearchQuery, KafkaReadOnlyState, KafkaTopicCreateRequest,
    KafkaTopicPartitionExpansion,
};
use ramag_domain::error::{DomainError, READ_ONLY_MESSAGE};
use ramag_domain::traits::{
    KafkaAdminDriver, KafkaConnectDriver, KafkaDriver, KafkaProducerDriver,
};
use ramag_infra_kafka::{KafkaConnectHttpDriver, RdkafkaDriver};
use rdkafka::ClientConfig;
use rdkafka::consumer::{BaseConsumer, CommitMode, Consumer};
use std::env;
use std::time::{Duration as StdDuration, Instant};

const FIXTURE_TOPIC: &str = "ramag.integration.messages";

/// Returns the Docker broker address, or skips the test when the dedicated
/// integration environment has not been configured for this process.
fn docker_bootstrap() -> Option<String> {
    match env::var("RAMAG_TEST_KAFKA_BOOTSTRAP") {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!(
                "Skipping Docker Kafka integration test; set RAMAG_TEST_KAFKA_BOOTSTRAP or run scripts/kafka-test/kafka-test.ps1 test."
            );
            None
        }
    }
}

fn docker_connect_endpoint() -> Option<String> {
    match env::var("RAMAG_TEST_KAFKA_CONNECT") {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!(
                "Skipping Docker Kafka Connect integration test; set RAMAG_TEST_KAFKA_CONNECT or run scripts/kafka-test/kafka-test.ps1 test."
            );
            None
        }
    }
}

fn install_tls_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    assert!(
        rustls::crypto::CryptoProvider::get_default().is_some(),
        "the integration test must install a rustls crypto provider before creating reqwest clients"
    );
}

/// Requests the real Kafka Connect REST API exposed by the dedicated fixture.
#[test]
fn docker_kafka_connect_lists_connectors() -> Result<(), Box<dyn std::error::Error>> {
    install_tls_crypto_provider();
    let Some(bootstrap) = docker_bootstrap() else {
        return Ok(());
    };
    let Some(endpoint) = docker_connect_endpoint() else {
        return Ok(());
    };
    let mut config = KafkaClusterConfig::new("ramag-docker-kafka", vec![bootstrap]);
    config.connect.endpoint = Some(endpoint);
    let driver = KafkaConnectHttpDriver::new()?;

    let connectors = smol::block_on(driver.list_connectors(&config))?;
    assert!(
        connectors.is_empty(),
        "the empty dedicated Connect fixture should not contain connectors"
    );
    Ok(())
}

/// Connects to the broker started by the Docker test runner and requests metadata.
#[test]
fn docker_kafka_accepts_metadata_request() {
    let Some(bootstrap) = docker_bootstrap() else {
        return;
    };
    let config = KafkaClusterConfig::new("ramag-docker-kafka", vec![bootstrap]);
    let driver = RdkafkaDriver::new();

    let result = smol::block_on(driver.test_connection(&config));
    assert!(
        result.is_ok(),
        "Docker Kafka metadata request failed; run scripts/kafka-test/kafka-test.ps1 up first: {result:?}"
    );
}

/// Reads the seeded fixture through the production driver, including metadata, watermarks,
/// offset ranges, time ranges, bounded search, and the no-commit read path.
#[test]
fn docker_kafka_reads_and_searches_seeded_fixture() {
    let Some(bootstrap) = docker_bootstrap() else {
        return;
    };
    let config = KafkaClusterConfig::new("ramag-docker-kafka", vec![bootstrap]);
    let driver = RdkafkaDriver::new();

    let metadata_result = smol::block_on(driver.cluster_metadata(&config));
    assert!(
        metadata_result.is_ok(),
        "Docker Kafka cluster metadata should be available: {metadata_result:?}"
    );
    let Ok(metadata) = metadata_result else {
        return;
    };
    assert!(!metadata.brokers.is_empty());

    let topics_result = smol::block_on(driver.list_topics(&config));
    assert!(
        topics_result.is_ok(),
        "Docker Kafka topic metadata should be available: {topics_result:?}"
    );
    let Ok(topics) = topics_result else {
        return;
    };
    let fixture = topics.iter().find(|topic| topic.name == FIXTURE_TOPIC);
    assert!(fixture.is_some(), "the Docker fixture topic should exist");
    let Some(fixture) = fixture else {
        return;
    };
    assert_eq!(fixture.partitions.len(), 3);
    let total_messages = fixture
        .partitions
        .iter()
        .map(|partition| partition.high_watermark.unwrap_or_default())
        .sum::<i64>();
    assert!(
        total_messages >= 3,
        "the fixture topic should contain at least the three seeded messages"
    );

    let scan = KafkaMessageQuery::by_offset(FIXTURE_TOPIC, vec![0, 1, 2], 0, None).with_limits(
        10,
        1024 * 1024,
        30,
        3,
    );
    let page_result = smol::block_on(driver.read_messages(&config, &scan));
    assert!(
        page_result.is_ok(),
        "the Docker fixture should be readable by offset: {page_result:?}"
    );
    let Ok(page) = page_result else {
        return;
    };
    assert_eq!(
        page.records.len(),
        10,
        "the bounded scan should return the requested first ten messages"
    );
    assert_eq!(page.scanned_records, 10);
    assert!(
        page.records
            .iter()
            .all(|record| record.topic == FIXTURE_TOPIC)
    );

    let time_scan = KafkaMessageQuery::by_time(
        FIXTURE_TOPIC,
        vec![0, 1, 2],
        Utc::now() - Duration::minutes(5),
        Some(Utc::now() + Duration::minutes(5)),
    )
    .with_limits(10, 1024 * 1024, 30, 3);
    let time_page_result = smol::block_on(driver.read_messages(&config, &time_scan));
    assert!(
        time_page_result.is_ok(),
        "the Docker fixture should be readable by time: {time_page_result:?}"
    );
    let Ok(time_page) = time_page_result else {
        return;
    };
    assert!((1..=10).contains(&time_page.records.len()));
    assert!(
        time_page
            .records
            .iter()
            .all(|record| record.timestamp.is_some())
    );

    let search_scan = KafkaMessageQuery::by_offset(FIXTURE_TOPIC, vec![0, 1, 2], 0, None)
        .with_limits(5_000, 32 * 1024 * 1024, 60, 3);
    let search = KafkaMessageSearchQuery::new("updated", search_scan)
        .with_fields(vec![KafkaMessageSearchField::Value]);
    let search_page_result = smol::block_on(driver.search_messages(&config, &search));
    assert!(
        search_page_result.is_ok(),
        "the Docker fixture should support bounded value search: {search_page_result:?}"
    );
    let Ok(search_page) = search_page_result else {
        return;
    };
    assert_eq!(search_page.records.len(), 1);
    assert_eq!(
        search_page.records[0].key.as_deref(),
        Some(&b"event-002"[..])
    );

    let bounded = scan.with_limits(1, 1024 * 1024, 30, 3);
    let bounded_page_result = smol::block_on(driver.read_messages(&config, &bounded));
    assert!(
        bounded_page_result.is_ok(),
        "bounded fixture read should succeed: {bounded_page_result:?}"
    );
    let Ok(bounded_page) = bounded_page_result else {
        return;
    };
    assert!(bounded_page.records.len() <= 1);
    assert!(bounded_page.scanned_records <= 1);

    // The default Docker fixture contains thousands of records so this path
    // exercises the same bounded scan and search workload used by the UI.
    let bulk_scan = KafkaMessageQuery::by_offset(FIXTURE_TOPIC, vec![0, 1, 2], 0, None)
        .with_limits(5_000, 32 * 1024 * 1024, 60, 3);
    let bulk_started = Instant::now();
    let bulk_page_result = smol::block_on(driver.read_messages(&config, &bulk_scan));
    assert!(
        bulk_page_result.is_ok(),
        "the Docker fixture should support a 5000-message scan: {bulk_page_result:?}"
    );
    let Ok(bulk_page) = bulk_page_result else {
        return;
    };
    assert_eq!(bulk_page.records.len(), 5_000);
    assert_eq!(bulk_page.scanned_records, 5_000);
    assert!(
        bulk_page
            .records
            .iter()
            .any(|record| record.key.as_deref() == Some(b"event-5000")),
        "the bulk scan should reach the final fixture record"
    );
    eprintln!(
        "Read and validated {} Docker fixture messages in {:?}",
        bulk_page.records.len(),
        bulk_started.elapsed()
    );

    let bulk_search = KafkaMessageSearchQuery::new("message-5000", bulk_scan)
        .with_fields(vec![KafkaMessageSearchField::Value]);
    let search_started = Instant::now();
    let bulk_search_result = smol::block_on(driver.search_messages(&config, &bulk_search));
    assert!(
        bulk_search_result.is_ok(),
        "the Docker fixture should support a bounded search over 5000 messages: {bulk_search_result:?}"
    );
    let Ok(bulk_search_page) = bulk_search_result else {
        return;
    };
    assert_eq!(bulk_search_page.records.len(), 1);
    assert_eq!(
        bulk_search_page.records[0].key.as_deref(),
        Some(&b"event-5000"[..])
    );
    eprintln!(
        "Searched {} Docker fixture messages in {:?}",
        bulk_search_page.scanned_records,
        search_started.elapsed()
    );
}

/// Joins a real consumer group, commits one fixture offset, and verifies the
/// production read path exposes its member assignment and lag snapshot.
#[test]
fn docker_kafka_lists_consumer_groups_and_offsets() {
    let Some(bootstrap) = docker_bootstrap() else {
        return;
    };
    let config = KafkaClusterConfig::new("ramag-docker-kafka", vec![bootstrap.clone()]);
    let group_id = "ramag.integration.consumer";
    let consumer: BaseConsumer = match ClientConfig::new()
        .set("bootstrap.servers", &bootstrap)
        .set("group.id", group_id)
        .set("enable.auto.commit", "false")
        .set("auto.offset.reset", "earliest")
        .create()
    {
        Ok(consumer) => consumer,
        Err(error) => panic!("Docker Kafka should create the fixture consumer: {error}"),
    };
    if let Err(error) = consumer.subscribe(&[FIXTURE_TOPIC]) {
        panic!("fixture consumer should subscribe: {error}");
    }
    let deadline = Instant::now() + StdDuration::from_secs(15);
    let mut committed = false;
    while Instant::now() < deadline {
        if let Some(Ok(message)) = consumer.poll(StdDuration::from_millis(250)) {
            if let Err(error) = consumer.commit_message(&message, CommitMode::Sync) {
                panic!("fixture consumer should commit one offset: {error}");
            }
            committed = true;
            break;
        }
    }
    assert!(
        committed,
        "fixture consumer should join and receive a message before timeout"
    );

    let driver = RdkafkaDriver::new();
    let groups_result = smol::block_on(driver.list_consumer_groups(&config));
    assert!(
        groups_result.is_ok(),
        "Docker Kafka consumer group metadata should be available: {groups_result:?}"
    );
    let Some(group) = groups_result
        .ok()
        .and_then(|groups| groups.into_iter().find(|group| group.group_id == group_id))
    else {
        panic!("the fixture consumer group should appear in the group list");
    };
    assert!(
        !group.members.is_empty(),
        "the active group should expose a member"
    );
    assert!(
        group.members.iter().any(|member| member
            .assigned_partitions
            .iter()
            .any(|assignment| assignment.topic == FIXTURE_TOPIC)),
        "the active group should expose its fixture assignment"
    );
    assert!(
        group
            .offsets
            .iter()
            .any(|offset| offset.topic == FIXTURE_TOPIC && offset.committed_offset.is_some()),
        "the committed fixture offset should be returned"
    );
}

/// Commits a real group offset, resets it through the production Admin API,
/// and reads the committed value back from the broker.
#[test]
fn docker_kafka_resets_consumer_group_offsets() -> Result<(), Box<dyn std::error::Error>> {
    let Some(bootstrap) = docker_bootstrap() else {
        return Ok(());
    };
    let group_id = "ramag.integration.offset-reset";
    let mut config =
        KafkaClusterConfig::new("ramag-docker-kafka-offset-reset", vec![bootstrap.clone()]);
    config.read_only = KafkaReadOnlyState::ReadWrite;
    {
        let consumer: BaseConsumer = ClientConfig::new()
            .set("bootstrap.servers", &bootstrap)
            .set("group.id", group_id)
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .create()?;
        consumer.subscribe(&[FIXTURE_TOPIC])?;
        let deadline = Instant::now() + StdDuration::from_secs(15);
        let mut committed = false;
        while Instant::now() < deadline {
            if let Some(Ok(message)) = consumer.poll(StdDuration::from_millis(250)) {
                consumer.commit_message(&message, CommitMode::Sync)?;
                committed = true;
                break;
            }
        }
        assert!(
            committed,
            "offset reset consumer should commit before reset"
        );
    }

    let driver = RdkafkaDriver::new();
    let request = KafkaConsumerGroupOffsetResetRequest::new(
        group_id,
        vec![KafkaConsumerGroupOffsetReset::new(FIXTURE_TOPIC, 0, 0)],
    );
    let reset_result = smol::block_on(driver.reset_consumer_group_offsets(&config, &request));
    assert!(
        reset_result.is_ok(),
        "Docker Kafka should reset consumer group offsets: {reset_result:?}"
    );
    if let Err(error) = reset_result {
        return Err(error.into());
    }

    let groups = smol::block_on(driver.list_consumer_groups(&config))?;
    let group = groups
        .into_iter()
        .find(|group| group.group_id == group_id)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "reset group should remain visible after committing an offset",
            )
        })?;
    assert!(group.offsets.iter().any(|offset| {
        offset.topic == FIXTURE_TOPIC && offset.partition == 0 && offset.committed_offset == Some(0)
    }));
    Ok(())
}

/// Produces one managed message with key/header/partition fields, then reads it
/// back by the Broker-returned offset to verify the native producer contract.
#[test]
fn docker_kafka_produces_and_reads_back_one_managed_message()
-> Result<(), Box<dyn std::error::Error>> {
    let Some(bootstrap) = docker_bootstrap() else {
        return Ok(());
    };
    let topic = FIXTURE_TOPIC;
    let request = KafkaMessageProduceRequest::new(topic, b"ramag-producer-integration".to_vec())
        .with_partition(1)
        .with_key(b"ramag-producer-key".to_vec())
        .with_headers(vec![KafkaMessageHeader {
            key: "trace-id".into(),
            value: Some(b"ramag-producer-header".to_vec()),
        }]);
    let driver = RdkafkaDriver::new();

    let read_only_config = KafkaClusterConfig::new(
        "ramag-docker-kafka-producer-read-only",
        vec![bootstrap.clone()],
    );
    let read_only_result = smol::block_on(driver.produce_message(&read_only_config, &request));
    assert!(matches!(
        read_only_result,
        Err(DomainError::Forbidden(message)) if message == READ_ONLY_MESSAGE
    ));

    let mut config = KafkaClusterConfig::new("ramag-docker-kafka-producer", vec![bootstrap]);
    config.read_only = KafkaReadOnlyState::ReadWrite;
    let result = smol::block_on(driver.produce_message(&config, &request))?;
    assert_eq!(result.topic, topic);
    assert_eq!(result.partition, 1);
    assert!(result.offset >= 0);

    let query = KafkaMessageQuery::by_offset(
        topic,
        vec![result.partition],
        result.offset,
        Some(result.offset.saturating_add(1)),
    )
    .with_limits(1, 1024 * 1024, 30, 1);
    let page = smol::block_on(driver.read_messages(&config, &query))?;
    assert_eq!(page.records.len(), 1);
    let record = &page.records[0];
    assert_eq!(record.topic, topic);
    assert_eq!(record.partition, result.partition);
    assert_eq!(record.offset, result.offset);
    assert_eq!(record.key.as_deref(), Some(&b"ramag-producer-key"[..]));
    assert_eq!(
        record.value.as_deref(),
        Some(&b"ramag-producer-integration"[..])
    );
    assert_eq!(record.headers.len(), 1);
    assert_eq!(record.headers[0].key, "trace-id");
    assert_eq!(
        record.headers[0].value.as_deref(),
        Some(&b"ramag-producer-header"[..])
    );
    Ok(())
}

/// Executes the production Admin API against a unique Docker topic and verifies
/// create, increase-partitions, metadata refresh, and delete as one sequence.
#[test]
fn docker_kafka_manages_topic_with_admin_api() {
    let Some(bootstrap) = docker_bootstrap() else {
        return;
    };
    let driver = RdkafkaDriver::new();

    let read_only_config = KafkaClusterConfig::new("ramag-docker-kafka", vec![bootstrap.clone()]);
    let read_only_request = KafkaTopicCreateRequest::new("ramag.integration.read-only", 1, 1);
    let read_only_result =
        smol::block_on(driver.create_topic(&read_only_config, &read_only_request));
    assert!(matches!(
        read_only_result,
        Err(DomainError::Forbidden(message)) if message == READ_ONLY_MESSAGE
    ));

    let mut config = KafkaClusterConfig::new("ramag-docker-kafka-admin", vec![bootstrap]);
    config.read_only = KafkaReadOnlyState::ReadWrite;
    let topic = format!("ramag.integration.admin.{}", uuid::Uuid::new_v4().simple());
    let create_request = KafkaTopicCreateRequest::new(&topic, 2, 1);
    let create_result = smol::block_on(driver.create_topic(&config, &create_request));
    assert!(
        create_result.is_ok(),
        "Docker Kafka Admin API should create a topic: {create_result:?}"
    );
    if create_result.is_err() {
        return;
    }

    let created_topics = smol::block_on(driver.list_topics(&config));
    assert!(
        created_topics.is_ok(),
        "created topic metadata should be readable: {created_topics:?}"
    );
    let Some(created_topic) = created_topics
        .ok()
        .and_then(|topics| topics.into_iter().find(|candidate| candidate.name == topic))
    else {
        let _ = smol::block_on(driver.delete_topic(&config, &topic));
        panic!("created topic should appear in refreshed metadata");
    };
    assert_eq!(created_topic.partitions.len(), 2);

    let expansion = KafkaTopicPartitionExpansion::new(&topic, 3);
    let expand_result = smol::block_on(driver.increase_topic_partitions(&config, &expansion));
    assert!(
        expand_result.is_ok(),
        "Docker Kafka Admin API should increase partitions: {expand_result:?}"
    );
    if expand_result.is_err() {
        let _ = smol::block_on(driver.delete_topic(&config, &topic));
        return;
    }

    let expanded_topics = smol::block_on(driver.list_topics(&config));
    assert!(
        expanded_topics.is_ok(),
        "expanded topic metadata should be readable: {expanded_topics:?}"
    );
    let expanded_partition_count = expanded_topics.ok().and_then(|topics| {
        topics
            .into_iter()
            .find(|candidate| candidate.name == topic)
            .map(|candidate| candidate.partitions.len())
    });
    assert_eq!(expanded_partition_count, Some(3));

    let delete_result = smol::block_on(driver.delete_topic(&config, &topic));
    assert!(
        delete_result.is_ok(),
        "Docker Kafka Admin API should delete the topic: {delete_result:?}"
    );
    if delete_result.is_err() {
        return;
    }

    let remaining_topics = smol::block_on(driver.list_topics(&config));
    assert!(
        remaining_topics.is_ok(),
        "metadata should refresh after delete: {remaining_topics:?}"
    );
    assert!(
        !remaining_topics
            .unwrap_or_default()
            .iter()
            .any(|candidate| candidate.name == topic)
    );
}
