use std::sync::{Arc, atomic::AtomicBool};

use ramag_domain::entities::KafkaClusterConfig;
use ramag_domain::error::Result;

use super::{KafkaService, validate_config};

impl KafkaService {
    pub async fn test_connection(&self, config: &KafkaClusterConfig) -> Result<()> {
        self.test_connection_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    /// 测试 Kafka 连接并把取消信号传到读取适配器。
    pub async fn test_connection_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        validate_config(config)?;
        let started = std::time::Instant::now();
        let result = self
            .driver
            .test_connection_with_cancel(config, cancelled)
            .await;
        tracing::info!(
            operation = "kafka_connection_test",
            cluster_id = %config.id,
            elapsed_ms = started.elapsed().as_millis(),
            success = result.is_ok(),
            "Kafka connection test completed"
        );
        result
    }
}
