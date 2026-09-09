use super::*;

/// 单独展示 exporter 提供的 Broker 运行指标，避免与 Kafka Protocol API 数值混在一组。
pub(super) fn render_broker_runtime_metrics(
    view: &KafkaView,
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
    let (status_text, status_color, sample_info, brokers) =
        match view.broker_metrics_snapshot.as_ref() {
            None => (
                view.broker_metrics_error
                    .clone()
                    .unwrap_or_else(|| "尚未采集".into()),
                if view.broker_metrics_error.is_some() {
                    theme.danger
                } else {
                    theme.muted_foreground
                },
                "采集时间：未知 · 来源：未采集".into(),
                None,
            ),
            Some(snapshot) => {
                let color = match snapshot.state {
                    KafkaMetricsSnapshotState::Ready => theme.success,
                    KafkaMetricsSnapshotState::Partial
                    | KafkaMetricsSnapshotState::NoData
                    | KafkaMetricsSnapshotState::SourceNotConfigured => theme.warning,
                    KafkaMetricsSnapshotState::PermissionDenied
                    | KafkaMetricsSnapshotState::CollectionFailed => theme.danger,
                };
                (
                    snapshot
                        .error
                        .clone()
                        .unwrap_or_else(|| snapshot.state.label().into()),
                    color,
                    format!(
                        "采集时间：{} · 来源：{}",
                        snapshot.sampled_at.to_rfc3339(),
                        snapshot.source.label()
                    ),
                    Some(snapshot.brokers.as_slice()),
                )
            }
        };
    let body = match brokers {
        Some(brokers) if !brokers.is_empty() => {
            let mut rows = v_flex()
                .id("kafka-broker-metrics-brokers")
                .debug_selector(|| "kafka-broker-metrics-brokers".into())
                .w_full()
                .border_1()
                .border_color(theme.border)
                .rounded(px(6.0));
            for broker in brokers.iter().take(100) {
                rows = rows.child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .flex_wrap()
                        .items_center()
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
                                .child(format!("Broker {}", broker.broker_id)),
                        )
                        .child(runtime_metric_cell(
                            "CPU",
                            format_percent(broker.cpu_usage_percent),
                            theme,
                        ))
                        .child(runtime_metric_cell(
                            "内存",
                            format_bytes(broker.memory_used_bytes),
                            theme,
                        ))
                        .child(runtime_metric_cell(
                            "磁盘",
                            format_bytes(broker.disk_used_bytes),
                            theme,
                        ))
                        .child(runtime_metric_cell(
                            "请求延迟",
                            format_millis(broker.request_latency_ms),
                            theme,
                        )),
                );
            }
            if brokers.len() > 100 {
                rows = rows.child(
                    div()
                        .px(px(10.0))
                        .py(px(7.0))
                        .text_xs()
                        .text_color(theme.warning)
                        .child(format!(
                            "Broker 运行指标数量过多，仅展示前 100 个；完整快照仍保留 {} 个",
                            brokers.len()
                        )),
                );
            }
            rows.into_any_element()
        }
        _ => v_flex()
            .id("kafka-broker-metrics-empty")
            .debug_selector(|| "kafka-broker-metrics-empty".into())
            .w_full()
            .items_center()
            .py(px(14.0))
            .text_xs()
            .text_color(theme.muted_foreground)
            .child("当前没有可展示的外部 Broker 运行指标")
            .into_any_element(),
    };

    v_flex()
        .id("kafka-overview-broker-runtime-metrics")
        .debug_selector(|| "kafka-overview-broker-runtime-metrics".into())
        .w_full()
        .min_w_0()
        .gap(px(10.0))
        .child(section_heading(
            "Broker 运行指标",
            "来自 Prometheus/OpenMetrics exporter；不与 Kafka Protocol API 快照合并",
            theme,
        ))
        .child(
            h_flex()
                .id("kafka-broker-metrics-status")
                .debug_selector(|| "kafka-broker-metrics-status".into())
                .w_full()
                .min_w_0()
                .flex_wrap()
                .items_start()
                .gap(px(8.0))
                .child(div().size(px(8.0)).rounded_full().bg(status_color))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .whitespace_normal()
                        .text_xs()
                        .text_color(status_color)
                        .child(status_text),
                )
                .child(
                    div()
                        .debug_selector(|| "kafka-broker-metrics-sample-info".into())
                        .flex_1()
                        .max_w(px(560.0))
                        .min_w_0()
                        .whitespace_normal()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(sample_info),
                ),
        )
        .child(body)
        .into_any_element()
}

fn runtime_metric_cell(
    label: &'static str,
    value: String,
    theme: &gpui_component::Theme,
) -> impl IntoElement {
    div()
        .min_w(px(112.0))
        .max_w(px(150.0))
        .text_xs()
        .text_color(theme.muted_foreground)
        .truncate()
        .child(format!("{label} {value}"))
}

fn format_percent(value: Option<f64>) -> String {
    value.map_or_else(|| "未知".into(), |value| format!("{value:.2}%"))
}

fn format_bytes(value: Option<f64>) -> String {
    value.map_or_else(|| "未知".into(), |value| format!("{value:.0} B"))
}

fn format_millis(value: Option<f64>) -> String {
    value.map_or_else(|| "未知".into(), |value| format!("{value:.2} ms"))
}
