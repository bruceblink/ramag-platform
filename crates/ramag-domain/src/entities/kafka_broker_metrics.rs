use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::kafka_validation::validate_optional_single_line;
use super::{
    KafkaMetricsSnapshotState, KafkaMetricsSource, MAX_KAFKA_BROKERS, MAX_KAFKA_CONFIG_VALUE_BYTES,
};

/// 一个 Broker 的外部运行指标；这些数值不来自 Kafka Protocol API。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KafkaBrokerRuntimeMetrics {
    pub broker_id: i32,
    pub cpu_usage_percent: Option<f64>,
    pub memory_used_bytes: Option<f64>,
    pub disk_used_bytes: Option<f64>,
    pub request_latency_ms: Option<f64>,
}

/// 外部 exporter 的独立快照，不与 Kafka 协议指标快照合并。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KafkaBrokerMetricsSnapshot {
    pub cluster_id: Option<String>,
    pub sampled_at: DateTime<Utc>,
    pub source: KafkaMetricsSource,
    pub state: KafkaMetricsSnapshotState,
    #[serde(default)]
    pub error: Option<String>,
    pub brokers: Vec<KafkaBrokerRuntimeMetrics>,
}

impl KafkaBrokerMetricsSnapshot {
    pub fn unavailable(
        cluster_id: Option<String>,
        sampled_at: DateTime<Utc>,
        state: KafkaMetricsSnapshotState,
        error: Option<String>,
    ) -> Self {
        Self {
            cluster_id,
            sampled_at,
            source: KafkaMetricsSource::ExternalBrokerMetrics,
            state,
            error,
            brokers: Vec::new(),
        }
    }

    /// 校验外部快照的来源、数量、Broker ID 和指标数值，防止 exporter 响应污染 UI。
    pub fn validate(&self) -> Result<(), String> {
        if self.source != KafkaMetricsSource::ExternalBrokerMetrics {
            return Err("Broker 运行指标快照来源必须是外部 Broker 指标".into());
        }
        validate_optional_single_line(
            "Broker 运行指标快照 Cluster ID",
            self.cluster_id.as_deref(),
            super::MAX_KAFKA_CLUSTER_ID_BYTES,
        )?;
        validate_optional_single_line(
            "Broker 运行指标快照错误",
            self.error.as_deref(),
            MAX_KAFKA_CONFIG_VALUE_BYTES,
        )?;
        if self.brokers.len() > MAX_KAFKA_BROKERS {
            return Err(format!(
                "Broker 运行指标数量超过 {MAX_KAFKA_BROKERS} 个上限"
            ));
        }
        let mut broker_ids = std::collections::HashSet::with_capacity(self.brokers.len());
        for broker in &self.brokers {
            if broker.broker_id < 0 {
                return Err("Broker 运行指标 ID 不能为负数".into());
            }
            if !broker_ids.insert(broker.broker_id) {
                return Err(format!("Broker 运行指标 ID 重复：{}", broker.broker_id));
            }
            validate_metric(broker.cpu_usage_percent, "Broker CPU 使用率", Some(100.0))?;
            validate_metric(broker.memory_used_bytes, "Broker 已用内存", None)?;
            validate_metric(broker.disk_used_bytes, "Broker 已用磁盘", None)?;
            validate_metric(broker.request_latency_ms, "Broker 请求延迟", None)?;
        }
        Ok(())
    }
}

fn validate_metric(value: Option<f64>, label: &str, maximum: Option<f64>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    if !value.is_finite() || value < 0.0 || maximum.is_some_and(|maximum| value > maximum) {
        return Err(format!("{label}数值无效"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_snapshot_rejects_protocol_source_and_invalid_values() {
        let mut snapshot = KafkaBrokerMetricsSnapshot {
            cluster_id: Some("cluster-a".into()),
            sampled_at: Utc::now(),
            source: KafkaMetricsSource::ExternalBrokerMetrics,
            state: KafkaMetricsSnapshotState::Ready,
            error: None,
            brokers: vec![KafkaBrokerRuntimeMetrics {
                broker_id: 0,
                cpu_usage_percent: Some(50.0),
                memory_used_bytes: Some(10.0),
                disk_used_bytes: None,
                request_latency_ms: Some(2.0),
            }],
        };
        assert!(snapshot.validate().is_ok());

        snapshot.brokers[0].cpu_usage_percent = Some(101.0);
        assert!(snapshot.validate().is_err());
        snapshot.source = KafkaMetricsSource::KafkaProtocol;
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn unavailable_snapshot_preserves_state_and_source() {
        let snapshot = KafkaBrokerMetricsSnapshot::unavailable(
            Some("cluster-a".into()),
            Utc::now(),
            KafkaMetricsSnapshotState::PermissionDenied,
            Some("HTTP 403".into()),
        );
        assert_eq!(snapshot.source, KafkaMetricsSource::ExternalBrokerMetrics);
        assert_eq!(snapshot.state, KafkaMetricsSnapshotState::PermissionDenied);
        assert!(snapshot.validate().is_ok());
    }
}
