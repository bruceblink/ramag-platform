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
impl KafkaView {
    /// 展示 Kafka 协议指标快照；外部 Broker 运行指标由独立数据源接入，不在此处猜测。
    pub(super) fn render_metrics_snapshot(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let capabilities = self.service.transport_capabilities();
        let compact = f32::from(window.viewport_size().width) < 700.0;
        let controls = h_flex()
            .id("kafka-metrics-controls")
            .debug_selector(|| "kafka-metrics-controls".into())
            .w_full()
            .min_w_0()
            .items_end()
            .gap(px(8.0))
            .when(compact, |row| row.flex_col().items_stretch())
            .child(
                field(
                    "刷新间隔（秒）",
                    Input::new(&self.metrics_refresh_seconds_input).small(),
                    if compact { 0.0 } else { 100.0 },
                )
                .debug_selector(|| "kafka-metrics-refresh-interval".into()),
            )
            .child(
                ramag_ui::clickable_button("kafka-metrics-refresh")
                    .debug_selector(|| "kafka-metrics-refresh".into())
                    .outline()
                    .small()
                    .icon(IconName::Search)
                    .label("刷新指标")
                    .loading(self.metrics_loading)
                    .disabled(
                        !capabilities.metrics_snapshot
                            || self.metrics_loading
                            || self.selected_cluster_id.is_none()
                            || self.loading_runtime
                            || self.testing
                            || self.saving
                            || self.deleting,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.refresh_metrics(window, cx);
                    })),
            );

        let status = if !capabilities.metrics_snapshot {
            ("当前构建未提供 Kafka 指标快照", theme.warning)
        } else if self.metrics_loading {
            ("正在采集 Kafka 指标快照", theme.warning)
        } else if self.metrics_error.is_some() {
            ("指标采集失败", theme.danger)
        } else if let Some(snapshot) = &self.metrics_snapshot {
            let color = match snapshot.state {
                KafkaMetricsSnapshotState::Ready => theme.success,
                KafkaMetricsSnapshotState::Partial
                | KafkaMetricsSnapshotState::NoData
                | KafkaMetricsSnapshotState::SourceNotConfigured => theme.warning,
                KafkaMetricsSnapshotState::PermissionDenied
                | KafkaMetricsSnapshotState::CollectionFailed => theme.danger,
            };
            (snapshot.state.label(), color)
        } else {
            ("尚未采集", theme.muted_foreground)
        };
        let status_text = self
            .metrics_error
            .clone()
            .or_else(|| {
                self.metrics_snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.error.clone())
            })
            .unwrap_or_else(|| status.0.to_string());
        let status_row = h_flex()
            .id("kafka-metrics-status")
            .debug_selector(|| "kafka-metrics-status".into())
            .w_full()
            .min_w_0()
            .items_center()
            .gap(px(8.0))
            .child(div().size(px(8.0)).rounded_full().bg(status.1))
            .child(
                div()
                    .text_xs()
                    .text_color(status.1)
                    .truncate()
                    .child(status_text),
            )
            .child(div().flex_1().min_w_0())
            .child(div().text_xs().text_color(theme.muted_foreground).child(
                self.metrics_snapshot.as_ref().map_or_else(
                    || "采集时间：未知 · 来源：未采集".into(),
                    |snapshot| {
                        format!(
                            "采集时间：{} · 来源：{}",
                            snapshot.sampled_at.to_rfc3339(),
                            snapshot.source.label()
                        )
                    },
                ),
            ));

        let body = match self.metrics_snapshot.as_ref() {
            Some(snapshot) => self.render_metrics_snapshot_body(snapshot, &theme),
            None => v_flex()
                .id("kafka-metrics-empty")
                .debug_selector(|| "kafka-metrics-empty".into())
                .w_full()
                .items_center()
                .py(px(18.0))
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(if capabilities.metrics_snapshot {
                    "选择集群后将显示 Broker、Partition、Lag 和速率快照"
                } else {
                    "当前 Transport 未实现协议指标快照"
                })
                .into_any_element(),
        };

        v_flex()
            .id("kafka-overview-metrics-snapshot")
            .debug_selector(|| "kafka-overview-metrics-snapshot".into())
            .w_full()
            .min_w_0()
            .flex_none()
            .gap(px(10.0))
            .p(px(14.0))
            .border_1()
            .border_color(theme.border)
            .rounded(px(6.0))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .gap(px(12.0))
                    .when(compact, |row| row.flex_col().items_stretch())
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Kafka 指标快照"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("协议 API 读取的可比较状态；缺失字段保留为未知"),
                            ),
                    )
                    .child(controls),
            )
            .child(status_row)
            .child(body)
    }
    fn render_metrics_snapshot_body(
        &self,
        snapshot: &KafkaMetricsSnapshot,
        theme: &gpui_component::Theme,
    ) -> gpui::AnyElement {
        let cluster = &snapshot.cluster;
        let summary = h_flex()
            .id("kafka-metrics-cluster")
            .debug_selector(|| "kafka-metrics-cluster".into())
            .w_full()
            .min_w_0()
            .flex_wrap()
            .gap(px(8.0))
            .child(metric_value(
                "Broker",
                format_count(cluster.broker_count),
                theme,
            ))
            .child(metric_value(
                "Topic",
                format_count(cluster.topic_count),
                theme,
            ))
            .child(metric_value(
                "Partition",
                format_count(cluster.partition_count),
                theme,
            ))
            .child(metric_value("总 Lag", format_i64(cluster.total_lag), theme))
            .child(metric_value("最大 Lag", format_i64(cluster.max_lag), theme))
            .child(metric_value(
                "估算消息速率",
                format_rate(cluster.message_rate_per_second),
                theme,
            ))
            .child(metric_value(
                "异常副本",
                format_count(cluster.under_replicated_partitions),
                theme,
            ))
            .child(metric_value(
                "离线分区",
                format_count(cluster.offline_partitions),
                theme,
            ));

        let topic_rows = if snapshot.topics.is_empty() {
            empty_metrics_row("当前快照没有 Topic 数据", theme)
        } else {
            let mut rows = v_flex()
                .id("kafka-metrics-topics")
                .debug_selector(|| "kafka-metrics-topics".into())
                .w_full()
                .border_1()
                .border_color(theme.border)
                .rounded(px(6.0));
            for topic in snapshot.topics.iter().take(50) {
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
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_xs()
                                .truncate()
                                .child(topic.name.clone()),
                        )
                        .child(metric_cell(format_count(topic.partition_count), theme))
                        .child(metric_cell(format_i64(topic.high_watermark), theme))
                        .child(metric_cell(
                            format_rate(topic.message_rate_per_second),
                            theme,
                        ))
                        .child(metric_cell(
                            format!(
                                "异常 {} / 离线 {}",
                                format_count(topic.under_replicated_partitions),
                                format_count(topic.offline_partitions)
                            ),
                            theme,
                        )),
                );
            }
            if snapshot.topics.len() > 50 {
                rows = rows.child(
                    div()
                        .px(px(10.0))
                        .py(px(7.0))
                        .text_xs()
                        .text_color(theme.warning)
                        .child(format!(
                            "Topic 数量过多，仅展示前 50 个；完整快照仍保留 {} 个",
                            snapshot.topics.len()
                        )),
                );
            }
            rows.into_any_element()
        };

        let group_rows = if snapshot.consumer_groups.is_empty() {
            empty_metrics_row("当前快照没有消费者组数据", theme)
        } else {
            let mut rows = v_flex()
                .id("kafka-metrics-consumer-groups")
                .debug_selector(|| "kafka-metrics-consumer-groups".into())
                .w_full()
                .border_1()
                .border_color(theme.border)
                .rounded(px(6.0));
            for group in snapshot.consumer_groups.iter().take(50) {
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
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_xs()
                                .truncate()
                                .child(group.group_id.clone()),
                        )
                        .child(metric_cell(group.state.as_deref().unwrap_or("未知"), theme))
                        .child(metric_cell(
                            format!("成员 {}", format_count(group.member_count)),
                            theme,
                        ))
                        .child(metric_cell(
                            format!("Lag {}", format_i64(group.total_lag)),
                            theme,
                        ))
                        .child(metric_cell(
                            format!("最大 {}", format_i64(group.max_lag)),
                            theme,
                        )),
                );
            }
            if snapshot.consumer_groups.len() > 50 {
                rows = rows.child(
                    div()
                        .px(px(10.0))
                        .py(px(7.0))
                        .text_xs()
                        .text_color(theme.warning)
                        .child(format!(
                            "消费者组数量过多，仅展示前 50 个；完整快照仍保留 {} 个",
                            snapshot.consumer_groups.len()
                        )),
                );
            }
            rows.into_any_element()
        };

        let partition_rows = Self::render_partition_health(snapshot, theme);

        v_flex()
            .w_full()
            .min_w_0()
            .gap(px(12.0))
            .child(summary)
            .child(section_heading(
                "Topic 状态",
                "Partition 数、末尾 Offset、估算速率和副本健康",
                theme,
            ))
            .child(topic_rows)
            .child(section_heading(
                "Partition 健康",
                "仅展示有限明细；缺失字段保持未知",
                theme,
            ))
            .child(partition_rows)
            .child(section_heading(
                "消费者组 Lag",
                "已提交 Offset 与末尾 Offset 的差值汇总",
                theme,
            ))
            .child(group_rows)
            .into_any_element()
    }
    /// 展示有界的 Partition 健康明细，避免大集群快照一次性膨胀 UI 布局。
    fn render_partition_health(
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
}
fn metric_value(
    label: &'static str,
    value: String,
    theme: &gpui_component::Theme,
) -> impl IntoElement {
    v_flex()
        .min_w(px(92.0))
        .flex_1()
        .gap(px(2.0))
        .p(px(9.0))
        .border_1()
        .border_color(theme.border)
        .rounded(px(4.0))
        .bg(theme.secondary.opacity(0.3))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(div().text_sm().truncate().child(value))
}
fn metric_cell(value: impl Into<SharedString>, theme: &gpui_component::Theme) -> impl IntoElement {
    div()
        .max_w(px(150.0))
        .min_w(px(64.0))
        .text_xs()
        .text_color(theme.muted_foreground)
        .truncate()
        .child(value.into())
}
fn empty_metrics_row(message: &'static str, theme: &gpui_component::Theme) -> gpui::AnyElement {
    v_flex()
        .id(SharedString::from(format!("kafka-metrics-empty-{message}")))
        .w_full()
        .items_center()
        .py(px(14.0))
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(message)
        .into_any_element()
}
fn format_count(value: Option<usize>) -> String {
    value.map_or_else(|| "未知".into(), |value| value.to_string())
}
fn format_i64(value: Option<i64>) -> String {
    value.map_or_else(|| "未知".into(), |value| value.to_string())
}
fn format_rate(value: Option<f64>) -> String {
    value.map_or_else(|| "未知".into(), |value| format!("{value:.2}/s"))
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
