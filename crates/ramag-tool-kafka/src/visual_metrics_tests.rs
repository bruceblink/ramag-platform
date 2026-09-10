use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

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

struct MetricsBrowseKafkaDriver {
    read_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl KafkaDriver for MetricsBrowseKafkaDriver {
    async fn test_connection(&self, _config: &KafkaClusterConfig) -> Result<()> {
        Ok(())
    }

    async fn read_messages(
        &self,
        _config: &KafkaClusterConfig,
        _query: &KafkaMessageQuery,
    ) -> Result<KafkaMessagePage> {
        self.read_calls.fetch_add(1, Ordering::Relaxed);
        Ok(KafkaMessagePage::empty())
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

    for (width, height) in [
        (360.0, 900.0),
        (900.0, 900.0),
        (1200.0, 900.0),
        (1440.0, 900.0),
    ] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();
        let snapshot_bounds = visual_cx.debug_bounds("kafka-overview-metrics-snapshot");
        let controls = visual_cx.debug_bounds("kafka-metrics-controls");
        let heading = visual_cx.debug_bounds("kafka-metrics-heading");
        let status = visual_cx.debug_bounds("kafka-metrics-status");
        let status_indicator = visual_cx.debug_bounds("kafka-metrics-status-indicator");
        let sample_info = visual_cx.debug_bounds("kafka-metrics-sample-info");
        let cluster_summary = visual_cx.debug_bounds("kafka-metrics-cluster");
        let topics = visual_cx.debug_bounds("kafka-metrics-topics");
        let partitions = visual_cx.debug_bounds("kafka-metrics-partition-health");
        let groups = visual_cx.debug_bounds("kafka-metrics-consumer-groups");
        let broker_health = visual_cx.debug_bounds("kafka-overview-broker-health");
        let broker_runtime = visual_cx.debug_bounds("kafka-overview-broker-runtime-metrics");
        let broker_runtime_status = visual_cx.debug_bounds("kafka-broker-metrics-status");
        let broker_runtime_sample_info = visual_cx.debug_bounds("kafka-broker-metrics-sample-info");
        let broker_runtime_rows = visual_cx.debug_bounds("kafka-broker-metrics-brokers");
        assert!(
            snapshot_bounds.is_some()
                && controls.is_some()
                && heading.is_some()
                && status.is_some()
                && status_indicator.is_some()
                && sample_info.is_some()
                && cluster_summary.is_some()
                && topics.is_some()
                && partitions.is_some()
                && groups.is_some()
                && broker_health.is_some()
                && broker_runtime.is_some()
                && broker_runtime_status.is_some()
                && broker_runtime_sample_info.is_some()
                && broker_runtime_rows.is_some(),
            "指标快照各区域都应参与布局: width={width}"
        );
        let (
            Some(snapshot_bounds),
            Some(controls),
            Some(heading),
            Some(status),
            Some(status_indicator),
            Some(sample_info),
            Some(cluster_summary),
            Some(topics),
            Some(partitions),
            Some(groups),
            Some(broker_health),
            Some(broker_runtime),
            Some(broker_runtime_status),
            Some(broker_runtime_sample_info),
            Some(broker_runtime_rows),
        ) = (
            snapshot_bounds,
            controls,
            heading,
            status,
            status_indicator,
            sample_info,
            cluster_summary,
            topics,
            partitions,
            groups,
            broker_health,
            broker_runtime,
            broker_runtime_status,
            broker_runtime_sample_info,
            broker_runtime_rows,
        )
        else {
            return;
        };

        for selector in [
            "kafka-overview-metrics-snapshot",
            "kafka-metrics-controls",
            "kafka-metrics-heading",
            "kafka-metrics-status",
            "kafka-metrics-status-indicator",
            "kafka-metrics-sample-info",
            "kafka-metrics-cluster",
            "kafka-metrics-topics",
            "kafka-metrics-partition-health",
            "kafka-metrics-consumer-groups",
            "kafka-overview-broker-health",
            "kafka-overview-broker-runtime-metrics",
            "kafka-broker-metrics-status",
            "kafka-broker-metrics-sample-info",
            "kafka-broker-metrics-brokers",
        ] {
            super::assert_within_width(visual_cx, selector, width);
        }
        assert!(
            controls.origin.x >= snapshot_bounds.origin.x
                && controls.right() <= snapshot_bounds.right()
                && heading.origin.x >= snapshot_bounds.origin.x
                && heading.right() <= snapshot_bounds.right()
                && status.origin.x >= snapshot_bounds.origin.x
                && status.right() <= snapshot_bounds.right()
                && status_indicator.right() <= status.right()
                && sample_info.right() <= status.right()
                && cluster_summary.right() <= snapshot_bounds.right()
                && topics.right() <= snapshot_bounds.right()
                && partitions.right() <= snapshot_bounds.right()
                && groups.right() <= snapshot_bounds.right()
                && broker_health.right() <= px(width)
                && broker_runtime.right() <= px(width)
                && broker_runtime_status.right() <= broker_runtime.right()
                && broker_runtime_sample_info.right() <= broker_runtime_status.right()
                && broker_runtime_rows.right() <= broker_runtime.right(),
            "指标快照和 Broker 运行内容不能横向越出容器: width={width}, snapshot={snapshot_bounds:?}, controls={controls:?}, status={status:?}, summary={cluster_summary:?}, topics={topics:?}, partitions={partitions:?}, groups={groups:?}, broker_health={broker_health:?}, broker_runtime={broker_runtime:?}"
        );
        let content_width = if width < 900.0 { width } else { width - 260.0 };
        if content_width >= 700.0 {
            assert!(
                heading.right() <= controls.origin.x,
                "桌面布局中指标标题不能被控件行压缩或覆盖: width={width}, heading={heading:?}, controls={controls:?}"
            );
            assert!(
                status_indicator.size.width >= px(80.0) && sample_info.size.width >= px(180.0),
                "桌面布局中指标状态和采样时间不能被压成窄列: width={width}, status_indicator={status_indicator:?}, sample_info={sample_info:?}"
            );
            assert!(
                broker_runtime_sample_info.size.width >= px(180.0),
                "桌面布局中 Broker 指标采样时间不能被压成逐字竖排: width={width}, sample_info={broker_runtime_sample_info:?}"
            );
        }
    }
}

#[gpui::test]
fn kafka_metrics_partition_browse_preserves_message_context(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let cluster = KafkaClusterConfig::new("Metrics 定位 Kafka", vec!["127.0.0.1:19092".into()]);
    let read_calls = Arc::new(AtomicUsize::new(0));
    let service = Arc::new(
        KafkaService::new(
            Arc::new(MetricsBrowseKafkaDriver {
                read_calls: read_calls.clone(),
            }),
            Arc::new(FakeStorage {
                cluster: cluster.clone(),
            }),
        )
        .with_monitoring_driver(Arc::new(MetricsMonitoringDriver {
            snapshot: snapshot(),
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
        view.loading_clusters = false;
        view.loading_runtime = false;
        view.section = KafkaSection::Overview;
        view.metrics_snapshot = Some(snapshot());
        view.message_page = Some(KafkaMessagePage::empty());
        view.selected_message = Some(0);
        view.loading_messages = true;
        cx.notify();
    });
    for width in [360.0, 900.0, 1200.0] {
        visual_cx.simulate_resize(size(px(width), px(900.0)));
        visual_cx.run_until_parked();
        assert!(
            visual_cx
                .debug_bounds("kafka-metrics-partition-browse-0")
                .is_some(),
            "Partition 健康行应提供消息定位入口: width={width}"
        );
        super::assert_within_width(visual_cx, "kafka-metrics-partition-browse-0", width);
    }
    click(visual_cx, "kafka-metrics-partition-browse-0");
    visual_cx.run_until_parked();

    let state = kafka_entity.read_with(visual_cx, |view, cx| {
        (
            view.section,
            view.topic_input.read(cx).value().to_string(),
            view.produce_topic_input.read(cx).value().to_string(),
            view.partition_input.read(cx).value().to_string(),
            view.message_page.is_none(),
            view.selected_message.is_none(),
            !view.loading_messages,
        )
    });
    assert_eq!(state.0, KafkaSection::Messages);
    assert_eq!(state.1, "metrics.events");
    assert_eq!(state.2, "metrics.events");
    assert_eq!(state.3, "0");
    assert!(state.4, "指标定位应清理旧消息页");
    assert!(state.5, "指标定位应清理旧消息选择");
    assert!(state.6, "指标定位不应自动启动消息读取");
    assert_eq!(read_calls.load(Ordering::Relaxed), 0);
}
