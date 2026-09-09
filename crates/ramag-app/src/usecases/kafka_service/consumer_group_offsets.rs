use ramag_domain::entities::{KafkaClusterConfig, KafkaConsumerGroupOffsetResetRequest};
use ramag_domain::error::Result;

use super::{KafkaService, log_admin_result, validate_admin_request};

impl KafkaService {
    /// 修改消费者组已提交 Offset；应用层先校验管理模式和显式目标列表。
    pub async fn reset_consumer_group_offsets(
        &self,
        config: &KafkaClusterConfig,
        request: &KafkaConsumerGroupOffsetResetRequest,
    ) -> Result<()> {
        validate_admin_request(config, request.validate())?;
        let started = std::time::Instant::now();
        let result = self
            .admin_driver
            .reset_consumer_group_offsets(config, request)
            .await;
        log_admin_result(
            "kafka_consumer_group_offset_reset",
            config,
            &request.group_id,
            started,
            &result,
        );
        result
    }
}
