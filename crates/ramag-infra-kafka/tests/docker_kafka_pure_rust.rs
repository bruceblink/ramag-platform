#![cfg(feature = "pure-rust")]

use chrono::{Duration, Utc};
use ramag_domain::entities::{
    KafkaClusterConfig, KafkaMessageQuery, KafkaMessageSearchField, KafkaMessageSearchQuery,
    KafkaTransportBackend,
};
use ramag_domain::error::{DomainError, KafkaErrorCategory};
use ramag_domain::traits::KafkaDriver;
use ramag_infra_kafka::PureRustDriver;
use std::env;

const FIXTURE_TOPIC: &str = "ramag.integration.messages";

fn docker_bootstrap() -> Option<String> {
    match env::var("RAMAG_TEST_KAFKA_BOOTSTRAP") {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!(
                "Skipping Docker Kafka pure Rust integration test; set RAMAG_TEST_KAFKA_BOOTSTRAP or run scripts/kafka-test/kafka-test.ps1 test."
            );
            None
        }
    }
}

#[test]
fn docker_kafka_pure_rust_reads_and_searches_seeded_fixture() -> Result<(), String> {
    let Some(bootstrap) = docker_bootstrap() else {
        return Ok(());
    };
    let config = KafkaClusterConfig::new("ramag-docker-kafka-pure-rust", vec![bootstrap]);
    let driver = PureRustDriver::new();
    let capabilities = driver.transport_capabilities();
    assert_eq!(capabilities.backend, KafkaTransportBackend::PureRust);
    assert!(capabilities.build_available);
    assert!(capabilities.fetch);
    assert!(capabilities.list_offsets);
    assert!(!capabilities.metadata);
    assert!(!capabilities.consumer_groups);
    assert!(!capabilities.topic_admin);
    assert!(!capabilities.config_admin);
    assert!(!capabilities.acl_admin);
    assert!(!capabilities.metrics_snapshot);
    assert!(!capabilities.tls);

    let connection_result = smol::block_on(driver.test_connection(&config));
    assert!(
        connection_result.is_ok(),
        "Docker Kafka should accept the pure Rust connection: {connection_result:?}"
    );

    let topics = smol::block_on(driver.list_topics(&config)).map_err(|error| {
        format!("Docker Kafka should list topics through the pure Rust client: {error:?}")
    })?;
    let fixture = topics
        .iter()
        .find(|topic| topic.name == FIXTURE_TOPIC)
        .ok_or_else(|| "the Docker fixture topic should exist".to_string())?;
    assert_eq!(fixture.partitions.len(), 3);

    let scan = KafkaMessageQuery::by_offset(FIXTURE_TOPIC, vec![0, 1, 2], 0, None).with_limits(
        10,
        1024 * 1024,
        30,
        3,
    );
    let page = smol::block_on(driver.read_messages(&config, &scan)).map_err(|error| {
        format!(
            "the Docker fixture should be readable by offset through the pure Rust client: {error:?}"
        )
    })?;
    assert_eq!(page.records.len(), 10);
    assert_eq!(page.scanned_records, 10);
    assert!(
        page.records
            .iter()
            .all(|record| record.topic == FIXTURE_TOPIC)
    );

    let time_scan = KafkaMessageQuery::by_time(
        FIXTURE_TOPIC,
        vec![0, 1, 2],
        Utc::now() - Duration::days(7),
        Some(Utc::now() + Duration::days(7)),
    )
    .with_limits(10, 1024 * 1024, 30, 3);
    let time_page = smol::block_on(driver.read_messages(&config, &time_scan)).map_err(|error| {
        format!(
            "the Docker fixture should be readable by time through the pure Rust client: {error:?}"
        )
    })?;
    assert!(!time_page.records.is_empty());
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
    let search_page =
        smol::block_on(driver.search_messages(&config, &search)).map_err(|error| {
            format!("the Docker fixture should support pure Rust value search: {error:?}")
        })?;
    assert_eq!(search_page.records.len(), 1);
    assert_eq!(
        search_page.records[0].key.as_deref(),
        Some(&b"event-002"[..])
    );

    let metadata_result = smol::block_on(driver.cluster_metadata(&config));
    assert!(matches!(
        metadata_result,
        Err(DomainError::Kafka(error)) if error.category == KafkaErrorCategory::Unsupported
    ));
    Ok(())
}
