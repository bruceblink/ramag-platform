use std::sync::{Arc, atomic::AtomicBool};

use ramag_domain::entities::{KafkaMessagePage, KafkaMessageQuery, KafkaMessageSearchQuery};
use ramag_domain::error::{DomainError, Result};

use super::{KafkaService, log_message_result, validate_config, validate_message_page};

impl KafkaService {
    pub async fn read_messages(
        &self,
        config: &ramag_domain::entities::KafkaClusterConfig,
        query: &KafkaMessageQuery,
    ) -> Result<KafkaMessagePage> {
        self.read_messages_with_cancel(config, query, Arc::new(AtomicBool::new(false)))
            .await
    }

    /// 读取有限消息范围并把取消信号传给驱动，停止后不保留后台扫描 Consumer。
    pub async fn read_messages_with_cancel(
        &self,
        config: &ramag_domain::entities::KafkaClusterConfig,
        query: &KafkaMessageQuery,
        cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaMessagePage> {
        validate_config(config)?;
        query.validate().map_err(DomainError::InvalidConfig)?;
        let started = std::time::Instant::now();
        let result = self
            .driver
            .read_messages_with_cancel(config, query, cancelled)
            .await;
        log_message_result(
            "kafka_message_read",
            config,
            query.topic.as_str(),
            started,
            &result,
        );
        result.and_then(validate_message_page)
    }

    pub async fn search_messages(
        &self,
        config: &ramag_domain::entities::KafkaClusterConfig,
        query: &KafkaMessageSearchQuery,
    ) -> Result<KafkaMessagePage> {
        self.search_messages_with_cancel(config, query, Arc::new(AtomicBool::new(false)))
            .await
    }

    /// 扫描有限消息范围并把取消信号传给驱动，搜索和普通读取使用同一生命周期边界。
    pub async fn search_messages_with_cancel(
        &self,
        config: &ramag_domain::entities::KafkaClusterConfig,
        query: &KafkaMessageSearchQuery,
        cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaMessagePage> {
        validate_config(config)?;
        query.validate().map_err(DomainError::InvalidConfig)?;
        let started = std::time::Instant::now();
        let result = self
            .driver
            .search_messages_with_cancel(config, query, cancelled)
            .await;
        log_message_result(
            "kafka_message_search",
            config,
            query.scan.topic.as_str(),
            started,
            &result,
        );
        result.and_then(validate_message_page)
    }
}
