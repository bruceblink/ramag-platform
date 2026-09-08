use super::*;
use chrono::TimeZone;

fn runtime_topic(high: i64) -> KafkaTopic {
    runtime_topic_named("events", &[high])
}

fn runtime_topic_named(name: &str, high_watermarks: &[i64]) -> KafkaTopic {
    KafkaTopic {
        name: name.into(),
        internal: false,
        partitions: high_watermarks
            .iter()
            .enumerate()
            .map(|(id, high)| KafkaPartition {
                id: id as i32,
                leader: Some(1),
                replicas: vec![1, 2],
                isr: vec![1, 2],
                low_watermark: Some(0),
                high_watermark: Some(*high),
            })
            .collect(),
    }
}

fn runtime_metadata() -> KafkaClusterMetadata {
    KafkaClusterMetadata {
        cluster_id: Some("cluster-a".into()),
        controller_id: Some(1),
        brokers: vec![super::super::KafkaBroker {
            id: 1,
            host: "broker-a".into(),
            port: 9092,
            rack: None,
            version: None,
            is_controller: true,
        }],
        kafka_version: None,
    }
}

fn group(lag: Option<i64>) -> KafkaConsumerGroup {
    KafkaConsumerGroup {
        group_id: "workers".into(),
        state: Some("Stable".into()),
        protocol: Some("range".into()),
        members: Vec::new(),
        offsets: vec![KafkaConsumerGroupOffset {
            topic: "events".into(),
            partition: 0,
            committed_offset: Some(1),
            end_offset: Some(1 + lag.unwrap_or(0)),
            lag,
        }],
    }
}

#[test]
fn snapshot_aggregates_fixed_offsets_and_preserves_unknown_lag() {
    let metadata = runtime_metadata();
    let ready = KafkaMetricsSnapshot::from_runtime(
        Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 0).unwrap(),
        &metadata,
        &[runtime_topic(12)],
        &[group(Some(5))],
    );
    assert_eq!(ready.cluster.total_lag, Some(5));
    assert_eq!(ready.cluster.max_lag, Some(5));
    assert_eq!(ready.topics[0].high_watermark, Some(12));
    assert!(ready.validate().is_ok());

    let unknown = KafkaMetricsSnapshot::from_runtime(
        ready.sampled_at,
        &metadata,
        &[runtime_topic(12)],
        &[group(None)],
    );
    assert_eq!(unknown.state, KafkaMetricsSnapshotState::Partial);
    assert_eq!(unknown.cluster.total_lag, None);
    assert_eq!(unknown.consumer_groups[0].total_lag, None);
}

#[test]
fn owned_snapshot_constructor_matches_borrowed_constructor() {
    let metadata = runtime_metadata();
    let topics = vec![runtime_topic(12)];
    let groups = vec![group(Some(5))];
    let sampled_at = Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 0).unwrap();
    let borrowed = KafkaMetricsSnapshot::from_runtime(sampled_at, &metadata, &topics, &groups);
    let owned = KafkaMetricsSnapshot::from_runtime_owned(sampled_at, &metadata, topics, groups);
    assert_eq!(owned, borrowed);
}

#[test]
fn rate_sampling_uses_high_watermark_delta_and_rejects_reset() {
    let metadata = runtime_metadata();
    let previous = KafkaMetricsSnapshot::from_runtime(
        Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 0).unwrap(),
        &metadata,
        &[runtime_topic(10)],
        &[],
    );
    let current = KafkaMetricsSnapshot::from_runtime(
        Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 5).unwrap(),
        &metadata,
        &[runtime_topic(20)],
        &[],
    )
    .with_high_watermark_rates(Some(&previous));
    assert_eq!(
        current.topics[0].partitions[0].message_rate_per_second,
        Some(2.0)
    );
    assert_eq!(current.cluster.message_rate_per_second, Some(2.0));

    let reset = KafkaMetricsSnapshot::from_runtime(
        Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 6).unwrap(),
        &metadata,
        &[runtime_topic(3)],
        &[],
    )
    .with_high_watermark_rates(Some(&current));
    assert_eq!(reset.topics[0].message_rate_per_second, None);
    assert_eq!(reset.cluster.message_rate_per_second, None);
}

#[test]
fn rate_sampling_matches_topic_and_partition_identity() {
    let metadata = runtime_metadata();
    let previous = KafkaMetricsSnapshot::from_runtime(
        Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 0).unwrap(),
        &metadata,
        &[
            runtime_topic_named("events", &[10, 110]),
            runtime_topic_named("payments", &[200]),
        ],
        &[],
    );
    let current = KafkaMetricsSnapshot::from_runtime(
        Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 5).unwrap(),
        &metadata,
        &[
            runtime_topic_named("events", &[20, 130]),
            runtime_topic_named("payments", &[230]),
        ],
        &[],
    )
    .with_high_watermark_rates(Some(&previous));

    assert_eq!(
        current.topics[0].partitions[0].message_rate_per_second,
        Some(2.0)
    );
    assert_eq!(
        current.topics[0].partitions[1].message_rate_per_second,
        Some(4.0)
    );
    assert_eq!(
        current.topics[1].partitions[0].message_rate_per_second,
        Some(6.0)
    );
    assert_eq!(current.cluster.message_rate_per_second, Some(12.0));
}

#[test]
fn rate_sampling_does_not_cross_cluster_boundaries() {
    let mut first_metadata = runtime_metadata();
    first_metadata.cluster_id = Some("cluster-a".into());
    let mut second_metadata = runtime_metadata();
    second_metadata.cluster_id = Some("cluster-b".into());
    let previous = KafkaMetricsSnapshot::from_runtime(
        Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 0).unwrap(),
        &first_metadata,
        &[runtime_topic(10)],
        &[],
    );
    let current = KafkaMetricsSnapshot::from_runtime(
        Utc.with_ymd_and_hms(2026, 9, 7, 10, 0, 5).unwrap(),
        &second_metadata,
        &[runtime_topic(20)],
        &[],
    )
    .with_high_watermark_rates(Some(&previous));
    assert_eq!(
        current.topics[0].partitions[0].message_rate_per_second,
        None
    );
}
