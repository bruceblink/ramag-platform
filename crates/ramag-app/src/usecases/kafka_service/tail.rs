use std::sync::{Arc, atomic::AtomicBool};

use ramag_domain::entities::{KafkaClusterConfig, KafkaMessageTailRequest};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::KafkaMessageTailSink;

impl super::KafkaService {
    /// 校验实时流请求后交给 Transport；消息事件由调用方通过有界 sink 接收。
    pub async fn tail_messages(
        &self,
        config: &KafkaClusterConfig,
        request: &KafkaMessageTailRequest,
        sink: KafkaMessageTailSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        request.validate().map_err(DomainError::InvalidConfig)?;
        let started = std::time::Instant::now();
        let result = self
            .driver
            .tail_messages(config, request, sink, cancelled)
            .await;
        tracing::info!(
            operation = "kafka_message_tail",
            cluster_id = %config.id,
            topic = %request.topic,
            partition_count = request.partitions.len(),
            elapsed_ms = started.elapsed().as_millis(),
            success = result.is_ok(),
            "Kafka 实时消息流结束"
        );
        result
    }
}
