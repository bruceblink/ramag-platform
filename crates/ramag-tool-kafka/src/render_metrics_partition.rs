use super::render_metrics::{format_count, format_rate, metric_cell};
use super::*;

const MAX_OVERVIEW_PARTITION_HEALTH_ROWS: usize = 100;
const PARTITION_HEALTH_LIST_HEIGHT: f32 = 320.0;
const PARTITION_HEALTH_ROW_HEIGHT: f32 = 44.0;
const PARTITION_HEALTH_COMPACT_ROW_HEIGHT: f32 = 68.0;

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
    compact: bool,
    scroll: &UniformListScrollHandle,
    cx: &mut Context<KafkaView>,
) -> gpui::AnyElement {
    let partitions = snapshot
        .topics
        .iter()
        .flat_map(|topic| topic.partitions.iter())
        .take(MAX_OVERVIEW_PARTITION_HEALTH_ROWS)
        .cloned()
        .collect::<Vec<_>>();
    let total_partitions = snapshot
        .topics
        .iter()
        .map(|topic| topic.partitions.len())
        .sum::<usize>();
    let visible = partitions.len();
    let list_body = if partitions.is_empty() {
        v_flex()
            .id("kafka-metrics-partition-health-empty")
            .w_full()
            .h(px(PARTITION_HEALTH_LIST_HEIGHT))
            .items_center()
            .justify_center()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child("当前快照没有 Partition 数据")
            .into_any_element()
    } else {
        let partitions = Arc::new(partitions);
        let row_theme = theme.clone();
        let rows = uniform_list(
            "kafka-metrics-partition-health-list",
            visible,
            cx.processor(move |this, range: Range<usize>, _window, cx| {
                range
                    .filter_map(|row_index| {
                        let partition = partitions.get(row_index)?.clone();
                        Some(partition_health_row(
                            this, partition, row_index, compact, &row_theme, cx,
                        ))
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(scroll)
        .w_full()
        .h(px(PARTITION_HEALTH_LIST_HEIGHT))
        .min_h_0();

        div()
            .id("kafka-metrics-partition-health-list-viewport")
            .relative()
            .w_full()
            .h(px(PARTITION_HEALTH_LIST_HEIGHT))
            .min_h_0()
            .child(rows)
            .child(
                div()
                    .id("kafka-metrics-partition-health-v-scrollbar")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(px(16.0))
                    .bg(theme.scrollbar)
                    .child(
                        Scrollbar::vertical(scroll)
                            .id("kafka-metrics-partition-health-v-scrollbar-control")
                            .scrollbar_show(ScrollbarShow::Always),
                    ),
            )
            .into_any_element()
    };
    let mut table = v_flex()
        .id("kafka-metrics-partition-health")
        .debug_selector(|| "kafka-metrics-partition-health".into())
        .w_full()
        .min_w_0()
        .border_1()
        .border_color(theme.border)
        .rounded(px(6.0))
        .child(list_body);
    if total_partitions > visible {
        table = table.child(
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
    table.into_any_element()
}

/// 构造固定高度的 Partition 行，窄窗口改为两行布局以保持虚拟列表尺寸一致。
fn partition_health_row(
    _view: &mut KafkaView,
    partition: KafkaPartitionMetrics,
    row_index: usize,
    compact: bool,
    theme: &gpui_component::Theme,
    cx: &mut Context<KafkaView>,
) -> gpui::AnyElement {
    let status = partition_health_status(&partition);
    let status_color = match status {
        PartitionHealthStatus::Healthy => theme.success,
        PartitionHealthStatus::UnderReplicated => theme.warning,
        PartitionHealthStatus::Offline => theme.danger,
        PartitionHealthStatus::Unknown => theme.muted_foreground,
    };
    let selector = format!("kafka-metrics-partition-browse-{row_index}");
    let topic_name = partition.topic.clone();
    let partition_id = partition.partition;
    let browse = ramag_ui::clickable_button(SharedString::from(selector.clone()))
        .debug_selector(move || selector.clone())
        .ghost()
        .xsmall()
        .icon(IconName::Search)
        .when(!compact, |button| button.label("浏览消息"))
        .tooltip("浏览消息")
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.open_partition_messages(topic_name.clone(), partition_id, window, cx);
        }));

    if compact {
        v_flex()
            .id(SharedString::from(format!(
                "kafka-metrics-partition-row-{row_index}"
            )))
            .w_full()
            .h(px(PARTITION_HEALTH_COMPACT_ROW_HEIGHT))
            .flex_none()
            .min_w_0()
            .gap(px(4.0))
            .px(px(10.0))
            .py(px(7.0))
            .border_b_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().flex_1().min_w_0().text_xs().truncate().child(format!(
                        "{} · Partition {}",
                        partition.topic, partition.partition
                    )))
                    .child(
                        div()
                            .flex_none()
                            .max_w(px(72.0))
                            .text_xs()
                            .text_color(status_color)
                            .truncate()
                            .child(status.label()),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.0))
                    .child(metric_cell(
                        format!("L {}", display_option_i32(partition.leader)),
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
                    .child(browse),
            )
            .into_any_element()
    } else {
        h_flex()
            .id(SharedString::from(format!(
                "kafka-metrics-partition-row-{row_index}"
            )))
            .w_full()
            .h(px(PARTITION_HEALTH_ROW_HEIGHT))
            .flex_none()
            .min_w_0()
            .items_center()
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
            )
            .child(browse)
            .into_any_element()
    }
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
