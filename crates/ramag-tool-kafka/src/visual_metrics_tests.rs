use std::sync::Arc;

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use gpui::{Context, IntoElement, Render, TestAppContext, Window, px, size};
use ramag_app::KafkaService;
use ramag_domain::entities::{
    KafkaBroker, KafkaBrokerMetricsSnapshot, KafkaBrokerRuntimeMetrics, KafkaClusterConfig,
    KafkaClusterMetadata, KafkaClusterMetrics, KafkaConsumerGroupMetrics, KafkaMetricsSnapshot,
    KafkaMetricsSnapshotState, KafkaMetricsSource, KafkaPartitionMetrics, KafkaTopicMetrics,
    KafkaTransportBackend, KafkaTransportCapabilities,
};
use ramag_domain::error::Result;
use ramag_domain::traits::{KafkaBrokerMetricsDriver, KafkaDriver, KafkaMonitoringDriver};

use super::*;

struct MetricsKafkaDriver;

#[async_trait]
impl KafkaDriver for MetricsKafkaDriver {
    fn transport_capabilities(&self) -> KafkaTransportCapabilities {
        KafkaTransportCapabilities {
            backend: KafkaTransportBackend::TestDouble,
            build_available: true,
            metadata: true,
            fetch: true,
            list_offsets: true,
            consumer_groups: true,
            topic_admin: false,
            config_admin: false,
            acl_admin: false,
            metrics_snapshot: true,
            tls: false,
            sasl: false,
        }
    }

    async fn test_connection(&self, _config: &KafkaClusterConfig) -> Result<()> {
        Ok(())
    }
}

struct MetricsMonitoringDriver {
    snapshot: KafkaMetricsSnapshot,
}

#[async_trait]
impl KafkaMonitoringDriver for MetricsMonitoringDriver {
    async fn metrics_snapshot(&self, _config: &KafkaClusterConfig) -> Result<KafkaMetricsSnapshot> {
        Ok(self.snapshot.clone())
    }
}

struct MetricsBrokerDriver {
    snapshot: KafkaBrokerMetricsSnapshot,
}

#[async_trait]
impl KafkaBrokerMetricsDriver for MetricsBrokerDriver {
    async fn broker_metrics_snapshot(
        &self,
        _config: &KafkaClusterConfig,
    ) -> Result<KafkaBrokerMetricsSnapshot> {
        Ok(self.snapshot.clone())
    }
}

struct KafkaMetricsTestHost {
    view: gpui::Entity<KafkaView>,
}

impl Render for KafkaMetricsTestHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = gpui_component::Root::render_dialog_layer(window, cx);
        gpui::div()
            .relative()
            .size_full()
            .child(self.view.clone())
            .children(dialog_layer)
    }
}

fn snapshot() -> KafkaMetricsSnapshot {
    KafkaMetricsSnapshot {
        cluster_id: Some("metrics-cluster".into()),
        sampled_at: Utc.with_ymd_and_hms(2026, 9, 7, 12, 0, 0).unwrap(),
        source: KafkaMetricsSource::KafkaProtocol,
        state: KafkaMetricsSnapshotState::Ready,
        error: None,
        cluster: KafkaClusterMetrics {
            broker_count: Some(3),
            topic_count: Some(2),
            partition_count: Some(4),
            total_lag: Some(12),
            max_lag: Some(8),
            message_rate_per_second: Some(2.5),
            under_replicated_partitions: Some(1),
            offline_partitions: Some(0),
        },
        topics: vec![KafkaTopicMetrics {
            name: "metrics.events".into(),
            partition_count: Some(2),
            low_watermark: Some(10),
            high_watermark: Some(20),
            message_rate_per_second: Some(2.5),
            under_replicated_partitions: Some(1),
            offline_partitions: Some(0),
            partitions: vec![KafkaPartitionMetrics {
                topic: "metrics.events".into(),
                partition: 0,
                leader: Some(1),
                replica_count: Some(2),
                isr_count: Some(1),
                low_watermark: Some(10),
                high_watermark: Some(20),
                under_replicated: Some(true),
                offline: Some(false),
                message_rate_per_second: Some(2.5),
            }],
        }],
        consumer_groups: vec![KafkaConsumerGroupMetrics {
            group_id: "metrics-workers".into(),
            state: Some("Stable".into()),
            member_count: Some(2),
            assigned_partition_count: Some(1),
            offset_count: Some(1),
            total_lag: Some(12),
            max_lag: Some(8),
        }],
    }
}

fn broker_metrics_snapshot() -> KafkaBrokerMetricsSnapshot {
    KafkaBrokerMetricsSnapshot {
        cluster_id: None,
        sampled_at: Utc.with_ymd_and_hms(2026, 9, 7, 12, 0, 0).unwrap(),
        source: KafkaMetricsSource::ExternalBrokerMetrics,
        state: KafkaMetricsSnapshotState::Ready,
        error: None,
        brokers: vec![KafkaBrokerRuntimeMetrics {
            broker_id: 1,
            cpu_usage_percent: Some(32.5),
            memory_used_bytes: Some(1024.0),
            disk_used_bytes: Some(4096.0),
            request_latency_ms: Some(2.5),
        }],
    }
}

#[gpui::test]
fn kafka_metrics_snapshot_reflows_without_horizontal_overflow(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let cluster = KafkaClusterConfig::new("Metrics Kafka", vec!["127.0.0.1:19092".into()]);
    let service = Arc::new(
        KafkaService::new(
            Arc::new(MetricsKafkaDriver),
            Arc::new(FakeStorage {
                cluster: cluster.clone(),
            }),
        )
        .with_monitoring_driver(Arc::new(MetricsMonitoringDriver {
            snapshot: snapshot(),
        }))
        .with_broker_metrics_driver(Arc::new(MetricsBrokerDriver {
            snapshot: broker_metrics_snapshot(),
        })),
    );
    let mut kafka_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let kafka = cx.new(|cx| KafkaView::new(service, window, cx));
        kafka_entity = Some(kafka.clone());
        let host = cx.new(|_| KafkaMetricsTestHost { view: kafka });
        gpui_component::Root::new(host, window, cx)
    });
    let Some(kafka_entity) = kafka_entity else {
        return;
    };

    kafka_entity.update(visual_cx, |view, cx| {
        view.clusters = vec![cluster.clone()];
        view.selected_cluster_id = Some(cluster.id.clone());
        view.metadata = Some(KafkaClusterMetadata {
            cluster_id: Some("metrics-cluster".into()),
            controller_id: Some(1),
            brokers: vec![KafkaBroker {
                id: 1,
                host: "127.0.0.1".into(),
                port: 19092,
                rack: None,
                version: Some("4.0.0".into()),
                is_controller: true,
            }],
            kafka_version: Some("4.0.0".into()),
        });
        view.metrics_snapshot = Some(snapshot());
        view.broker_metrics_snapshot = Some(broker_metrics_snapshot());
        view.loading_clusters = false;
        view.loading_runtime = false;
        view.section = KafkaSection::Overview;
        cx.notify();
    });
    visual_cx.run_until_parked();

    for (width, height) in [(360.0, 900.0), (900.0, 900.0), (1440.0, 900.0)] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();
        let snapshot_bounds = visual_cx.debug_bounds("kafka-overview-metrics-snapshot");
        let controls = visual_cx.debug_bounds("kafka-metrics-controls");
        let status = visual_cx.debug_bounds("kafka-metrics-status");
        let cluster_summary = visual_cx.debug_bounds("kafka-metrics-cluster");
        let topics = visual_cx.debug_bounds("kafka-metrics-topics");
        let partitions = visual_cx.debug_bounds("kafka-metrics-partition-health");
        let groups = visual_cx.debug_bounds("kafka-metrics-consumer-groups");
        let broker_health = visual_cx.debug_bounds("kafka-overview-broker-health");
        let broker_runtime = visual_cx.debug_bounds("kafka-overview-broker-runtime-metrics");
        let broker_runtime_status = visual_cx.debug_bounds("kafka-broker-metrics-status");
        let broker_runtime_rows = visual_cx.debug_bounds("kafka-broker-metrics-brokers");
        assert!(
            snapshot_bounds.is_some()
                && controls.is_some()
                && status.is_some()
                && cluster_summary.is_some()
                && topics.is_some()
                && partitions.is_some()
                && groups.is_some()
                && broker_health.is_some()
                && broker_runtime.is_some()
                && broker_runtime_status.is_some()
                && broker_runtime_rows.is_some(),
            "指标快照各区域都应参与布局: width={width}"
        );
        let (
            Some(snapshot_bounds),
            Some(controls),
            Some(status),
            Some(cluster_summary),
            Some(topics),
            Some(partitions),
            Some(groups),
            Some(broker_health),
            Some(broker_runtime),
            Some(broker_runtime_status),
            Some(broker_runtime_rows),
        ) = (
            snapshot_bounds,
            controls,
            status,
            cluster_summary,
            topics,
            partitions,
            groups,
            broker_health,
            broker_runtime,
            broker_runtime_status,
            broker_runtime_rows,
        )
        else {
            return;
        };

        for selector in [
            "kafka-overview-metrics-snapshot",
            "kafka-metrics-controls",
            "kafka-metrics-status",
            "kafka-metrics-cluster",
            "kafka-metrics-topics",
            "kafka-metrics-partition-health",
            "kafka-metrics-consumer-groups",
            "kafka-overview-broker-health",
            "kafka-overview-broker-runtime-metrics",
            "kafka-broker-metrics-status",
            "kafka-broker-metrics-brokers",
        ] {
            super::assert_within_width(visual_cx, selector, width);
        }
        assert!(
            controls.origin.x >= snapshot_bounds.origin.x
                && controls.right() <= snapshot_bounds.right()
                && status.origin.x >= snapshot_bounds.origin.x
                && status.right() <= snapshot_bounds.right()
                && cluster_summary.right() <= snapshot_bounds.right()
                && topics.right() <= snapshot_bounds.right()
                && partitions.right() <= snapshot_bounds.right()
                && groups.right() <= snapshot_bounds.right()
                && broker_health.right() <= px(width)
                && broker_runtime.right() <= px(width)
                && broker_runtime_status.right() <= broker_runtime.right()
                && broker_runtime_rows.right() <= broker_runtime.right(),
            "指标快照和 Broker 运行内容不能横向越出容器: width={width}, snapshot={snapshot_bounds:?}, controls={controls:?}, status={status:?}, summary={cluster_summary:?}, topics={topics:?}, partitions={partitions:?}, groups={groups:?}, broker_health={broker_health:?}, broker_runtime={broker_runtime:?}"
        );
    }
}
