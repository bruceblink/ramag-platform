use super::*;
#[cfg(feature = "cmake-build")]
use chrono::Utc;
use ramag_domain::entities::KafkaMetricsSnapshot;

#[cfg(not(feature = "cmake-build"))]
impl RdkafkaTransport {
    pub(super) fn metrics_snapshot_blocking(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<KafkaMetricsSnapshot> {
        let _ = self.request_timeout;
        Self::ensure_build_features(config)?;
        Err(native_client_unavailable("读取 Kafka 指标快照"))
    }
}

#[cfg(feature = "cmake-build")]
impl RdkafkaTransport {
    /// 复用同一轮读取的 Topic 快照和消费者组查询，构造不包含消息正文的快照。
    pub(super) fn metrics_snapshot_blocking(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<KafkaMetricsSnapshot> {
        Self::ensure_build_features(config)?;
        let metadata = self.cluster_metadata_blocking(config)?;
        let topics = self.list_topics_blocking(config)?;
        let groups = self.list_consumer_groups_with_topics_blocking(config, &topics)?;
        let snapshot = KafkaMetricsSnapshot::from_runtime(Utc::now(), &metadata, &topics, &groups);
        snapshot
            .validate()
            .map(|()| snapshot)
            .map_err(DomainError::InvalidConfig)
    }
}
