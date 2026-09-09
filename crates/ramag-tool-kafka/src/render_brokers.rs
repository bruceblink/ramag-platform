use super::*;

impl KafkaView {
    pub(super) fn render_broker_table(
        &self,
        metadata: &KafkaClusterMetadata,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut rows = v_flex()
            .w_full()
            .border_1()
            .border_color(theme.border)
            .rounded(px(6.0));
        for broker in metadata.brokers.iter().take(100) {
            rows = rows.child(broker_row(broker, &theme));
        }
        if metadata.brokers.len() > 100 {
            rows = rows.child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .text_xs()
                    .text_color(theme.warning)
                    .child("Broker 数量过多，仅展示前 100 个"),
            );
        }
        rows
    }

    pub(super) fn render_broker_health(
        &self,
        metadata: &KafkaClusterMetadata,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (status, status_color) = if self.loading_runtime {
            ("正在刷新 Metadata", theme.warning)
        } else if self.runtime_error.is_some() {
            ("刷新失败，展示上次元数据", theme.danger)
        } else {
            ("协议可达", theme.success)
        };
        let snapshot_broker_status = match self
            .metrics_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.cluster.broker_count)
        {
            Some(count) if count == metadata.brokers.len() => {
                format!("{} 个 · 与元数据一致", count)
            }
            Some(count) => format!(
                "{} 个 · 与元数据不一致（元数据 {} 个）",
                count,
                metadata.brokers.len()
            ),
            None => "未知 · 快照未提供".into(),
        };

        v_flex()
            .id("kafka-overview-broker-health")
            .debug_selector(|| "kafka-overview-broker-health".into())
            .w_full()
            .min_w_0()
            .gap(px(8.0))
            .child(
                h_flex()
                    .id("kafka-overview-broker-health-status")
                    .debug_selector(|| "kafka-overview-broker-health-status".into())
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().size(px(8.0)).rounded_full().bg(status_color))
                    .child(
                        div()
                            .min_w_0()
                            .text_xs()
                            .text_color(status_color)
                            .child(status),
                    ),
            )
            .child(
                h_flex()
                    .id("kafka-overview-broker-health-details")
                    .debug_selector(|| "kafka-overview-broker-health-details".into())
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .gap(px(8.0))
                    .child(broker_health_value(
                        "元数据 Broker",
                        format!("{} 个", metadata.brokers.len()),
                        &theme,
                    ))
                    .child(broker_health_value(
                        "指标快照 Broker",
                        snapshot_broker_status,
                        &theme,
                    ))
                    .child(broker_health_value(
                        "Broker 运行指标",
                        "未接入；需配置 JMX、Prometheus 或 exporter".into(),
                        &theme,
                    )),
            )
    }

    pub(super) fn render_cluster_summary(
        &self,
        metadata: &KafkaClusterMetadata,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        v_flex()
            .w_full()
            .gap(px(10.0))
            .p(px(14.0))
            .border_1()
            .border_color(theme.border)
            .rounded(px(6.0))
            .child(summary_row(
                "Cluster ID",
                metadata.cluster_id.as_deref().unwrap_or("未知"),
                &theme,
            ))
            .child(summary_row(
                "Controller",
                &display_option_i32(metadata.controller_id),
                &theme,
            ))
            .child(summary_row(
                "Kafka 版本",
                metadata.kafka_version.as_deref().unwrap_or("未知"),
                &theme,
            ))
            .child(summary_row(
                "读取模式",
                "只读，不提交 Consumer Offset",
                &theme,
            ))
    }

    pub(super) fn render_topic_preview(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        if self.topics.is_empty() {
            return v_flex()
                .w_full()
                .items_center()
                .p(px(18.0))
                .border_1()
                .border_color(theme.border)
                .rounded(px(6.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Broker 没有返回 Topic"),
                );
        }
        let mut rows = v_flex()
            .w_full()
            .border_1()
            .border_color(theme.border)
            .rounded(px(6.0));
        for (index, topic) in self.topics.iter().take(8).enumerate() {
            let name_selector = format!("kafka-overview-topic-preview-name-{index}");
            let partition_selector = format!("kafka-overview-topic-preview-partitions-{index}");
            rows = rows.child(
                h_flex()
                    .debug_selector(move || format!("kafka-overview-topic-preview-row-{index}"))
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(9.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .debug_selector(move || name_selector.clone())
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_sm()
                            .child(topic.name.clone()),
                    )
                    .child(
                        div()
                            .debug_selector(move || partition_selector.clone())
                            .flex_none()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{} partitions", topic.partitions.len())),
                    ),
            );
        }
        rows
    }
}

fn broker_health_value(
    label: &'static str,
    value: String,
    theme: &gpui_component::Theme,
) -> impl IntoElement {
    v_flex()
        .flex_1()
        .min_w(px(132.0))
        .gap(px(2.0))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(div().min_w_0().text_xs().whitespace_normal().child(value))
}
