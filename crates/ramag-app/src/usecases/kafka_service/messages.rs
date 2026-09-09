use std::sync::{Arc, atomic::AtomicBool};

use ramag_domain::entities::{
    KafkaMessagePage, KafkaMessageProduceRequest, KafkaMessageProduceResult, KafkaMessageQuery,
    KafkaMessageSearchQuery,
};
use ramag_domain::error::{DomainError, Result};

use super::{
    KafkaService, ensure_admin_enabled, log_message_produce_result, log_message_result,
    validate_config, validate_message_page, validate_message_produce_result,
};

impl KafkaService {
    /// 校验并提交一条消息；只读模式在调用生产驱动前被应用层拒绝。
    pub async fn produce_message(
        &self,
        config: &ramag_domain::entities::KafkaClusterConfig,
        request: &KafkaMessageProduceRequest,
    ) -> Result<KafkaMessageProduceResult> {
        validate_config(config)?;
        request.validate().map_err(DomainError::InvalidConfig)?;
        ensure_admin_enabled(config)?;
        let started = std::time::Instant::now();
        let result = self.producer_driver.produce_message(config, request).await;
        log_message_produce_result("kafka_message_produce", config, request, started, &result);
        result.and_then(|result| validate_message_produce_result(request, result))
    }

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
