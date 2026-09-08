use super::render_metrics::{format_count, format_rate, metric_cell};
use super::*;

const MAX_OVERVIEW_PARTITION_HEALTH_ROWS: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PartitionHealthStatus {
    Healthy,
    UnderReplicated,
    Offline,
    Unknown,
}

impl PartitionHealthStatus {
    const fn label(self) -> &'static str {
        match self {
            Self::Healthy => "正常",
            Self::UnderReplicated => "副本不足",
            Self::Offline => "离线",
            Self::Unknown => "未知",
        }
    }
}

/// 将 Partition 快照映射为用户可读的健康状态；缺失字段保持未知，不猜测 Broker 状态。
pub(super) fn partition_health_status(partition: &KafkaPartitionMetrics) -> PartitionHealthStatus {
    if partition.offline == Some(true) {
        return PartitionHealthStatus::Offline;
    }
    if partition.under_replicated == Some(true)
        || matches!((partition.replica_count, partition.isr_count), (Some(replicas), Some(isr)) if isr < replicas)
    {
        return PartitionHealthStatus::UnderReplicated;
    }
    if partition.offline == Some(false) && partition.under_replicated == Some(false) {
        return PartitionHealthStatus::Healthy;
    }
    if matches!((partition.replica_count, partition.isr_count), (Some(replicas), Some(isr)) if isr >= replicas)
    {
        return PartitionHealthStatus::Healthy;
    }
    PartitionHealthStatus::Unknown
}

/// 展示有界的 Partition 健康明细，避免大集群快照一次性膨胀 UI 布局。
pub(super) fn render_partition_health(
    snapshot: &KafkaMetricsSnapshot,
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
    let partitions = snapshot
        .topics
        .iter()
        .flat_map(|topic| topic.partitions.iter())
        .take(MAX_OVERVIEW_PARTITION_HEALTH_ROWS);
    let total_partitions = snapshot
        .topics
        .iter()
        .map(|topic| topic.partitions.len())
        .sum::<usize>();
    let mut rows = v_flex()
        .id("kafka-metrics-partition-health")
        .debug_selector(|| "kafka-metrics-partition-health".into())
        .w_full()
        .border_1()
        .border_color(theme.border)
        .rounded(px(6.0));
    let mut visible = 0;
    for partition in partitions {
        visible += 1;
        let status = partition_health_status(partition);
        let status_color = match status {
            PartitionHealthStatus::Healthy => theme.success,
            PartitionHealthStatus::UnderReplicated => theme.warning,
            PartitionHealthStatus::Offline => theme.danger,
            PartitionHealthStatus::Unknown => theme.muted_foreground,
        };
        rows = rows.child(
            h_flex()
                .w_full()
                .min_w_0()
                .items_center()
                .flex_wrap()
                .gap(px(8.0))
                .px(px(10.0))
                .py(px(7.0))
                .border_b_1()
                .border_color(theme.border)
                .child(div().flex_1().min_w_0().text_xs().truncate().child(format!(
                    "{} · Partition {}",
                    partition.topic, partition.partition
                )))
                .child(metric_cell(
                    format!("Leader {}", display_option_i32(partition.leader)),
                    theme,
                ))
                .child(metric_cell(
                    format!(
                        "ISR {}/{}",
                        format_count(partition.isr_count),
                        format_count(partition.replica_count)
                    ),
                    theme,
                ))
                .child(metric_cell(
                    format!("速率 {}", format_rate(partition.message_rate_per_second)),
                    theme,
                ))
                .child(
                    div()
                        .max_w(px(84.0))
                        .min_w(px(64.0))
                        .text_xs()
                        .text_color(status_color)
                        .truncate()
                        .child(status.label()),
                ),
        );
    }
    if visible == 0 {
        rows = rows.child(
            div()
                .w_full()
                .items_center()
                .py(px(14.0))
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("当前快照没有 Partition 数据"),
        );
    } else if total_partitions > visible {
        rows = rows.child(
            div()
                .px(px(10.0))
                .py(px(7.0))
                .text_xs()
                .text_color(theme.warning)
                .child(format!(
                    "Partition 数量过多，仅展示前 {visible} 个；完整快照仍保留 {total_partitions} 个"
                )),
        );
    }
    rows.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn partition() -> KafkaPartitionMetrics {
        KafkaPartitionMetrics {
            topic: "events".into(),
            partition: 0,
            leader: Some(1),
            replica_count: Some(3),
            isr_count: Some(3),
            low_watermark: Some(0),
            high_watermark: Some(10),
            under_replicated: Some(false),
            offline: Some(false),
            message_rate_per_second: None,
        }
    }

    #[test]
    fn partition_health_preserves_explicit_failure_states() {
        let mut partition = partition();
        assert_eq!(
            partition_health_status(&partition),
            PartitionHealthStatus::Healthy
        );

        partition.under_replicated = Some(true);
        assert_eq!(
            partition_health_status(&partition),
            PartitionHealthStatus::UnderReplicated
        );

        partition.offline = Some(true);
        assert_eq!(
            partition_health_status(&partition),
            PartitionHealthStatus::Offline
        );
    }

    #[test]
    fn partition_health_does_not_turn_missing_fields_into_healthy() {
        let mut partition = partition();
        partition.offline = None;
        partition.under_replicated = None;
        partition.replica_count = None;
        partition.isr_count = None;
        assert_eq!(
            partition_health_status(&partition),
            PartitionHealthStatus::Unknown
        );
    }
}
