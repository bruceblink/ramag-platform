use super::*;
#[cfg(feature = "cmake-build")]
use chrono::Utc;
use ramag_domain::entities::KafkaMetricsSnapshot;

#[cfg(not(feature = "cmake-build"))]
impl RdkafkaTransport {
    pub(super) fn metrics_snapshot_blocking_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: &AtomicBool,
    ) -> Result<KafkaMetricsSnapshot> {
        let _ = self.request_timeout;
        ensure_not_cancelled(cancelled, "读取 Kafka 指标快照")?;
        Self::ensure_build_features(config)?;
        Err(native_client_unavailable("读取 Kafka 指标快照"))
    }
}

#[cfg(feature = "cmake-build")]
impl RdkafkaTransport {
    /// 复用同一轮读取的 Topic 快照和消费者组查询，构造不包含消息正文的快照。
    pub(super) fn metrics_snapshot_blocking_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: &AtomicBool,
    ) -> Result<KafkaMetricsSnapshot> {
        ensure_not_cancelled(cancelled, "读取 Kafka 指标快照")?;
        Self::ensure_build_features(config)?;
        let (metadata, topics) = {
            let (consumer, metadata) = self.fetch_metadata_blocking_with_cancel(
                config,
                "读取 Kafka 集群元数据",
                cancelled,
            )?;
            let cluster_metadata = self.cluster_metadata_from_metadata(&consumer, &metadata)?;
            let topics =
                self.list_topics_from_metadata_with_cancel(&consumer, &metadata, cancelled)?;
            (cluster_metadata, topics)
        };
        let groups =
            self.list_consumer_groups_with_topics_blocking_with_cancel(config, &topics, cancelled)?;
        ensure_not_cancelled(cancelled, "读取 Kafka 指标快照")?;
        let snapshot =
            KafkaMetricsSnapshot::from_runtime_owned(Utc::now(), &metadata, topics, groups);
        snapshot
            .validate()
            .map(|()| snapshot)
            .map_err(DomainError::InvalidConfig)
    }
}
