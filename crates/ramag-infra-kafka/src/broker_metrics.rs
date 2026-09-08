use std::{collections::BTreeMap, io::Read, time::Duration};

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use ramag_domain::{
    entities::{
        KafkaBrokerMetricsSnapshot, KafkaBrokerRuntimeMetrics, KafkaClusterConfig,
        KafkaMetricsSnapshotState, KafkaMetricsSource,
    },
    error::{DomainError, Result},
    traits::KafkaBrokerMetricsDriver,
};
use reqwest::blocking::{Client, Response};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const CPU_METRIC: &str = "ramag_kafka_broker_cpu_usage_percent";
const MEMORY_METRIC: &str = "ramag_kafka_broker_memory_used_bytes";
const DISK_METRIC: &str = "ramag_kafka_broker_disk_used_bytes";
const LATENCY_METRIC: &str = "ramag_kafka_broker_request_latency_ms";

/// 读取 Prometheus/OpenMetrics exposition 文本中的 Ramag Broker 指标。
#[derive(Clone)]
pub struct PrometheusBrokerMetricsDriver {
    client: Client,
}

impl PrometheusBrokerMetricsDriver {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .user_agent(concat!("Ramag/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| DomainError::Other("创建 Broker 指标 HTTP 客户端失败".into()))?;
        Ok(Self { client })
    }

    fn snapshot_blocking(&self, config: &KafkaClusterConfig) -> KafkaBrokerMetricsSnapshot {
        // KafkaClusterId identifies the saved Ramag profile, not the Kafka cluster returned by
        // Metadata. Do not copy it into the external snapshot as an unverified cluster ID.
        let cluster_id = None;
        let Some(endpoint) = config.broker_metrics.endpoint.as_deref() else {
            return KafkaBrokerMetricsSnapshot::unavailable(
                cluster_id,
                Utc::now(),
                KafkaMetricsSnapshotState::SourceNotConfigured,
                None,
            );
        };

        let response = match self.client.get(endpoint).send() {
            Ok(response) => response,
            Err(error) => {
                let message = request_error_message(&error);
                tracing::warn!(operation = "kafka_broker_metrics_request", error = %message, "外部 Broker 指标请求失败");
                return KafkaBrokerMetricsSnapshot::unavailable(
                    cluster_id,
                    Utc::now(),
                    KafkaMetricsSnapshotState::CollectionFailed,
                    Some(message),
                );
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let state = if status == reqwest::StatusCode::UNAUTHORIZED
                || status == reqwest::StatusCode::FORBIDDEN
            {
                KafkaMetricsSnapshotState::PermissionDenied
            } else {
                KafkaMetricsSnapshotState::CollectionFailed
            };
            return KafkaBrokerMetricsSnapshot::unavailable(
                cluster_id,
                Utc::now(),
                state,
                Some(format!("外部 Broker 指标源返回 HTTP {}", status.as_u16())),
            );
        }

        let body = match read_response_body(response) {
            Ok(body) => body,
            Err(message) => {
                return KafkaBrokerMetricsSnapshot::unavailable(
                    cluster_id,
                    Utc::now(),
                    KafkaMetricsSnapshotState::CollectionFailed,
                    Some(message),
                );
            }
        };
        match parse_exposition(&body) {
            Ok(parsed) => KafkaBrokerMetricsSnapshot {
                cluster_id,
                sampled_at: parsed.sampled_at,
                source: KafkaMetricsSource::ExternalBrokerMetrics,
                state: parsed.state,
                error: parsed.error,
                brokers: parsed.brokers,
            },
            Err(error) => KafkaBrokerMetricsSnapshot::unavailable(
                cluster_id,
                Utc::now(),
                KafkaMetricsSnapshotState::CollectionFailed,
                Some(error),
            ),
        }
    }
}

#[async_trait]
impl KafkaBrokerMetricsDriver for PrometheusBrokerMetricsDriver {
    async fn broker_metrics_snapshot(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<KafkaBrokerMetricsSnapshot> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        let driver = self.clone();
        let config = config.clone();
        Ok(smol::unblock(move || driver.snapshot_blocking(&config)).await)
    }
}

fn read_response_body(mut response: Response) -> std::result::Result<String, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("外部 Broker 指标响应超过 4 MiB 限制".into());
    }
    let mut body = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|_| "读取外部 Broker 指标响应失败".to_string())?;
        if read == 0 {
            break;
        }
        if body.len().saturating_add(read) > MAX_RESPONSE_BYTES {
            return Err("外部 Broker 指标响应超过 4 MiB 限制".into());
        }
        body.extend_from_slice(&buffer[..read]);
    }
    String::from_utf8(body).map_err(|_| "外部 Broker 指标响应不是 UTF-8 文本".into())
}

fn request_error_message(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "外部 Broker 指标请求超时".into()
    } else if error.is_connect() {
        "无法连接外部 Broker 指标源".into()
    } else {
        "外部 Broker 指标请求失败".into()
    }
}

#[derive(Debug)]
struct ParsedExposition {
    sampled_at: DateTime<Utc>,
    state: KafkaMetricsSnapshotState,
    error: Option<String>,
    brokers: Vec<KafkaBrokerRuntimeMetrics>,
}

#[derive(Default)]
struct BrokerValues {
    cpu_usage_percent: Option<f64>,
    memory_used_bytes: Option<f64>,
    disk_used_bytes: Option<f64>,
    request_latency_ms: Option<f64>,
}

fn parse_exposition(body: &str) -> std::result::Result<ParsedExposition, String> {
    let mut values = BTreeMap::<i32, BrokerValues>::new();
    let mut sampled_at: Option<DateTime<Utc>> = None;
    for line in body.lines() {
        let Some(metric) = sample_metric_name(line) else {
            continue;
        };
        let Some(field) = metric_field(metric) else {
            continue;
        };
        let Some((metric, labels, value, timestamp)) = parse_sample_line(line)? else {
            continue;
        };
        let Some(broker_id_text) = label_value(labels, "broker_id") else {
            return Err(format!("指标 {metric} 缺少 broker_id 标签"));
        };
        let broker_id = broker_id_text
            .parse::<i32>()
            .map_err(|_| format!("指标 {metric} 的 broker_id 无效"))?;
        if broker_id < 0 {
            return Err(format!("指标 {metric} 的 broker_id 不能为负数"));
        }
        let broker = values.entry(broker_id).or_default();
        match field {
            MetricField::Cpu => broker.cpu_usage_percent = Some(value),
            MetricField::Memory => broker.memory_used_bytes = Some(value),
            MetricField::Disk => broker.disk_used_bytes = Some(value),
            MetricField::Latency => broker.request_latency_ms = Some(value),
        }
        if let Some(timestamp) = timestamp.and_then(parse_timestamp) {
            sampled_at = Some(sampled_at.map_or(timestamp, |current| current.max(timestamp)));
        }
    }

    let brokers = values
        .into_iter()
        .map(|(broker_id, values)| KafkaBrokerRuntimeMetrics {
            broker_id,
            cpu_usage_percent: values.cpu_usage_percent,
            memory_used_bytes: values.memory_used_bytes,
            disk_used_bytes: values.disk_used_bytes,
            request_latency_ms: values.request_latency_ms,
        })
        .collect::<Vec<_>>();
    let state = if brokers.is_empty() {
        KafkaMetricsSnapshotState::NoData
    } else if brokers.iter().any(|broker| {
        broker.cpu_usage_percent.is_none()
            || broker.memory_used_bytes.is_none()
            || broker.disk_used_bytes.is_none()
            || broker.request_latency_ms.is_none()
    }) {
        KafkaMetricsSnapshotState::Partial
    } else {
        KafkaMetricsSnapshotState::Ready
    };
    let error = (state == KafkaMetricsSnapshotState::NoData)
        .then_some("没有找到可识别的 Broker 运行指标".into());
    Ok(ParsedExposition {
        sampled_at: sampled_at.unwrap_or_else(Utc::now),
        state,
        error,
        brokers,
    })
}

#[derive(Clone, Copy)]
enum MetricField {
    Cpu,
    Memory,
    Disk,
    Latency,
}

fn metric_field(metric: &str) -> Option<MetricField> {
    match metric {
        CPU_METRIC => Some(MetricField::Cpu),
        MEMORY_METRIC => Some(MetricField::Memory),
        DISK_METRIC => Some(MetricField::Disk),
        LATENCY_METRIC => Some(MetricField::Latency),
        _ => None,
    }
}

fn sample_metric_name(line: &str) -> Option<&str> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    Some(
        line.split_once(|character: char| character == '{' || character.is_ascii_whitespace())
            .map_or(line, |(metric, _)| metric),
    )
}

type ParsedSample<'a> = (&'a str, &'a str, f64, Option<f64>);

fn parse_sample_line(line: &str) -> std::result::Result<Option<ParsedSample<'_>>, String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let (metric, labels, values) = if let Some(open) = line.find('{') {
        let metric = &line[..open];
        let labeled_values = &line[open + 1..];
        let end = labeled_values
            .find('}')
            .ok_or_else(|| format!("指标 {metric} 标签未闭合"))?;
        (
            metric,
            &labeled_values[..end],
            labeled_values[end + 1..].trim_start(),
        )
    } else {
        let (metric, values) = line
            .split_once(|character: char| character.is_ascii_whitespace())
            .ok_or_else(|| "外部 Broker 指标行缺少值".to_string())?;
        (metric, "", values.trim_start())
    };
    let mut tokens = values.split_whitespace();
    let value = tokens
        .next()
        .ok_or_else(|| format!("指标 {metric} 缺少值"))?
        .parse::<f64>()
        .map_err(|_| format!("指标 {metric} 数值无效"))?;
    if !value.is_finite() || value < 0.0 {
        return Err(format!("指标 {metric} 数值无效"));
    }
    let timestamp = tokens
        .next()
        .map(|token| {
            token
                .parse::<f64>()
                .map_err(|_| format!("指标 {metric} 时间戳无效"))
        })
        .transpose()?;
    Ok(Some((metric, labels, value, timestamp)))
}

fn label_value<'a>(labels: &'a str, name: &str) -> Option<&'a str> {
    labels.split(',').find_map(|label| {
        let (key, value) = label.split_once('=')?;
        if key.trim() != name {
            return None;
        }
        value.trim().strip_prefix('"')?.strip_suffix('"')
    })
}

fn parse_timestamp(value: f64) -> Option<DateTime<Utc>> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    if value > 100_000_000_000.0 {
        Utc.timestamp_millis_opt(value as i64).single()
    } else {
        Utc.timestamp_opt(value as i64, (value.fract() * 1_000_000_000.0) as u32)
            .single()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bounded_prometheus_exposition_and_preserves_sample_time() {
        let result = parse_exposition(
            "# HELP ignored ignored\nramag_kafka_broker_cpu_usage_percent{broker_id=\"0\"} 12.5 1725600000000\nramag_kafka_broker_memory_used_bytes{broker_id=\"0\"} 20\nramag_kafka_broker_disk_used_bytes{broker_id=\"0\"} 30\nramag_kafka_broker_request_latency_ms{broker_id=\"0\"} 4.5\n",
        );
        let error = result.as_ref().err();
        assert!(result.is_ok(), "valid exposition: {error:?}");
        let Some(parsed) = result.ok() else {
            return;
        };
        assert_eq!(parsed.state, KafkaMetricsSnapshotState::Ready);
        assert_eq!(parsed.brokers.len(), 1);
        assert_eq!(parsed.brokers[0].cpu_usage_percent, Some(12.5));
        assert_eq!(parsed.brokers[0].request_latency_ms, Some(4.5));
        assert_eq!(parsed.sampled_at.timestamp_millis(), 1_725_600_000_000);
    }

    #[test]
    fn reports_partial_and_ignores_unrelated_metrics() {
        let result = parse_exposition(
            "node_cpu_seconds_total{cpu=\"0\"} NaN\nramag_kafka_broker_cpu_usage_percent{broker_id=\"2\"} 10\n",
        );
        let error = result.as_ref().err();
        assert!(result.is_ok(), "valid exposition: {error:?}");
        let Some(parsed) = result.ok() else {
            return;
        };
        assert_eq!(parsed.state, KafkaMetricsSnapshotState::Partial);
        assert_eq!(parsed.brokers[0].broker_id, 2);
    }

    #[test]
    fn rejects_known_metric_without_broker_label() {
        let result =
            parse_exposition("ramag_kafka_broker_cpu_usage_percent{instance=\"broker\"} 10\n");
        assert!(result.is_err(), "broker label is required");
        let Some(error) = result.err() else {
            return;
        };
        assert!(error.contains("broker_id"));
    }
}
