//! Kafka 基础设施层：使用 `rdkafka` 创建隔离的客户端，并将错误映射为安全领域错误。
mod admin;
mod broker_metrics;
#[cfg(feature = "cmake-build")]
mod config;
mod connect;
#[cfg(feature = "cmake-build")]
mod consumer_groups;
#[cfg(feature = "cmake-build")]
pub mod errors;
#[cfg(feature = "cmake-build")]
mod messages;
#[cfg(feature = "cmake-build")]
mod metadata;
mod metrics;
mod producer_driver;
mod schema_registry;
pub use broker_metrics::PrometheusBrokerMetricsDriver;
pub use connect::KafkaConnectHttpDriver;
use ramag_domain::entities::KafkaMessageTailRequest;
#[cfg(feature = "cmake-build")]
use ramag_domain::entities::{
    KafkaBroker, KafkaClusterMetadata, KafkaPartition, KafkaTopic, MAX_KAFKA_BROKERS,
    MAX_KAFKA_PARTITION_REPLICA_IDS, MAX_KAFKA_PARTITIONS, MAX_KAFKA_REPLICAS, MAX_KAFKA_TOPICS,
};
use ramag_domain::entities::{KafkaClusterConfig, KafkaTransportCapabilities};
use ramag_domain::error::{DomainError, KafkaError, KafkaErrorCategory, Result};
use ramag_domain::traits::{
    KafkaDriver, KafkaMessageTailSink, KafkaMonitoringDriver, KafkaTransport,
};
#[cfg(feature = "cmake-build")]
use rdkafka::admin::AdminClient;
#[cfg(feature = "cmake-build")]
use rdkafka::client::DefaultClientContext;
#[cfg(feature = "cmake-build")]
use rdkafka::consumer::{BaseConsumer, Consumer};
#[cfg(feature = "cmake-build")]
use rdkafka::metadata::Metadata;
pub use schema_registry::SchemaRegistryHttpDriver;
use std::sync::atomic::Ordering;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Duration;
use tracing::{debug, info};
pub const DEFAULT_KAFKA_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_KAFKA_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy)]
pub struct RdkafkaTransport {
    request_timeout: Duration,
}

/// Compatibility name kept for existing integration callers.
pub type RdkafkaDriver = RdkafkaTransport;

impl RdkafkaTransport {
    /// 创建使用固定默认请求预算的 Kafka 驱动。
    pub fn new() -> Self {
        Self {
            request_timeout: DEFAULT_KAFKA_REQUEST_TIMEOUT,
        }
    }

    /// 创建可指定请求预算的驱动，供集成测试和调用方控制连接等待时间。
    pub fn with_request_timeout(request_timeout: Duration) -> Result<Self> {
        validate_request_timeout(request_timeout)?;
        Ok(Self { request_timeout })
    }

    /// 在进入 native 客户端前检查构建能力，避免把不支持的安全配置交给底层库。
    fn ensure_build_features(config: &KafkaClusterConfig) -> Result<()> {
        if config.uses_sasl() && !cfg!(feature = "kafka-sasl") {
            return Err(DomainError::Kafka(KafkaError::new(
                KafkaErrorCategory::Unsupported,
                "创建连接客户端",
                "当前构建未启用 Kafka SASL 支持",
            )));
        }
        if config.uses_tls() && !cfg!(feature = "kafka-tls") {
            return Err(DomainError::Kafka(KafkaError::new(
                KafkaErrorCategory::Tls,
                "创建连接客户端",
                "当前构建未启用 Kafka TLS 支持",
            )));
        }
        Ok(())
    }

    #[cfg(all(not(feature = "cmake-build"), test))]
    /// 默认构建不拉入 librdkafka；启用 `cmake-build` 后才提供真实连接能力。
    fn test_connection_blocking(&self, config: &KafkaClusterConfig) -> Result<()> {
        let _ = self.request_timeout;
        Self::ensure_build_features(config)?;
        Err(DomainError::Kafka(KafkaError::new(
            KafkaErrorCategory::Unsupported,
            "创建连接客户端",
            "当前构建未启用 Kafka native 客户端；请启用 cmake-build feature",
        )))
    }

    #[cfg(not(feature = "cmake-build"))]
    fn test_connection_blocking_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        ensure_not_cancelled(cancelled, "测试 Kafka 连接")?;
        let _ = self.request_timeout;
        Self::ensure_build_features(config)?;
        Err(DomainError::Kafka(KafkaError::new(
            KafkaErrorCategory::Unsupported,
            "创建连接客户端",
            "当前构建未启用 Kafka native 客户端；请启用 cmake-build feature",
        )))
    }

    #[cfg(not(feature = "cmake-build"))]
    fn cluster_metadata_blocking(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<ramag_domain::entities::KafkaClusterMetadata> {
        let _ = self.request_timeout;
        Self::ensure_build_features(config)?;
        Err(native_client_unavailable("读取 Kafka 集群元数据"))
    }

    #[cfg(not(feature = "cmake-build"))]
    fn cluster_metadata_blocking_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: &AtomicBool,
    ) -> Result<ramag_domain::entities::KafkaClusterMetadata> {
        ensure_not_cancelled(cancelled, "读取 Kafka 集群元数据")?;
        self.cluster_metadata_blocking(config)
    }

    #[cfg(not(feature = "cmake-build"))]
    fn list_topics_blocking(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<Vec<ramag_domain::entities::KafkaTopic>> {
        let _ = self.request_timeout;
        Self::ensure_build_features(config)?;
        Err(native_client_unavailable("读取 Kafka Topic 元数据"))
    }

    #[cfg(not(feature = "cmake-build"))]
    fn list_topics_blocking_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: &AtomicBool,
    ) -> Result<Vec<ramag_domain::entities::KafkaTopic>> {
        ensure_not_cancelled(cancelled, "读取 Kafka Topic 元数据")?;
        self.list_topics_blocking(config)
    }

    #[cfg(not(feature = "cmake-build"))]
    fn list_consumer_groups_blocking(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<Vec<ramag_domain::entities::KafkaConsumerGroup>> {
        let _ = self.request_timeout;
        Self::ensure_build_features(config)?;
        Err(native_client_unavailable("读取 Kafka 消费者组"))
    }

    #[cfg(not(feature = "cmake-build"))]
    fn list_consumer_groups_blocking_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: &AtomicBool,
    ) -> Result<Vec<ramag_domain::entities::KafkaConsumerGroup>> {
        ensure_not_cancelled(cancelled, "读取 Kafka 消费者组")?;
        self.list_consumer_groups_blocking(config)
    }

    #[cfg(not(feature = "cmake-build"))]
    fn scan_messages_blocking(
        &self,
        config: &KafkaClusterConfig,
        _query: &ramag_domain::entities::KafkaMessageQuery,
        _search: Option<&ramag_domain::entities::KafkaMessageSearchQuery>,
    ) -> Result<ramag_domain::entities::KafkaMessagePage> {
        let _ = self.request_timeout;
        Self::ensure_build_features(config)?;
        Err(native_client_unavailable("读取 Kafka 消息"))
    }

    #[cfg(not(feature = "cmake-build"))]
    fn tail_messages_blocking(
        &self,
        config: &KafkaClusterConfig,
        _request: &KafkaMessageTailRequest,
        _sink: KafkaMessageTailSink,
        _cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        let _ = self.request_timeout;
        Self::ensure_build_features(config)?;
        Err(native_client_unavailable("读取 Kafka 实时消息流"))
    }

    #[cfg(not(feature = "cmake-build"))]
    fn scan_messages_blocking_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        query: &ramag_domain::entities::KafkaMessageQuery,
        search: Option<&ramag_domain::entities::KafkaMessageSearchQuery>,
        _cancelled: &AtomicBool,
    ) -> Result<ramag_domain::entities::KafkaMessagePage> {
        self.scan_messages_blocking(config, query, search)
    }

    #[cfg(all(feature = "cmake-build", test))]
    /// 在阻塞线程中创建 Admin Client 并拉取集群元数据，以验证连接配置和网络可达性。
    fn test_connection_blocking(&self, config: &KafkaClusterConfig) -> Result<()> {
        let cancelled = AtomicBool::new(false);
        self.test_connection_blocking_with_cancel(config, &cancelled)
    }

    #[cfg(feature = "cmake-build")]
    fn test_connection_blocking_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        ensure_not_cancelled(cancelled, "测试 Kafka 连接")?;
        Self::ensure_build_features(config)?;
        let client_config = config::build_client_config(config, self.request_timeout)?;
        ensure_not_cancelled(cancelled, "测试 Kafka 连接")?;
        let admin: AdminClient<DefaultClientContext> = client_config
            .create()
            .map_err(|error| errors::map_kafka_error(error, "创建连接客户端"))?;
        ensure_not_cancelled(cancelled, "测试 Kafka 连接")?;
        admin
            .inner()
            .fetch_metadata(None, self.request_timeout)
            .map_err(|error| errors::map_kafka_error(error, "测试 Kafka 连接"))?;
        ensure_not_cancelled(cancelled, "测试 Kafka 连接")?;
        Ok(())
    }

    #[cfg(feature = "cmake-build")]
    fn create_consumer(&self, config: &KafkaClusterConfig) -> Result<BaseConsumer> {
        Self::ensure_build_features(config)?;
        let mut client_config = config::build_client_config(config, self.request_timeout)?;
        // 手动分配 Partition 仍需 group.id；使用临时 UUID，且客户端配置关闭自动提交，
        // 避免浏览任务加入或推进用户已有的业务消费组。
        client_config.set(
            "group.id",
            format!("ramag-kafka-browser-{}", uuid::Uuid::new_v4()),
        );
        client_config
            .create()
            .map_err(|error| errors::map_kafka_error(error, "创建 Kafka 读取客户端"))
    }
}

fn ensure_not_cancelled(cancelled: &AtomicBool, operation: &'static str) -> Result<()> {
    if cancelled.load(Ordering::Acquire) {
        return Err(DomainError::Kafka(KafkaError::new(
            KafkaErrorCategory::Cancelled,
            operation,
            "Kafka 读取任务已取消",
        )));
    }
    Ok(())
}

#[cfg(feature = "cmake-build")]
fn validate_partition_budget(
    total_partitions: &mut usize,
    topic_name: &str,
    partition_count: usize,
) -> Result<()> {
    if partition_count > MAX_KAFKA_PARTITIONS {
        return Err(DomainError::InvalidConfig(format!(
            "Topic Partition 数量超过 {MAX_KAFKA_PARTITIONS} 个上限：{topic_name}"
        )));
    }
    let next_total = total_partitions
        .checked_add(partition_count)
        .ok_or_else(|| DomainError::InvalidConfig("Kafka Partition 总数量超出可计算范围".into()))?;
    if next_total > MAX_KAFKA_PARTITIONS {
        return Err(DomainError::InvalidConfig(format!(
            "Kafka Partition 总数量超过 {MAX_KAFKA_PARTITIONS} 个上限"
        )));
    }
    *total_partitions = next_total;
    Ok(())
}

#[cfg(feature = "cmake-build")]
fn validate_partition_replica_budget(
    total_replica_ids: &mut usize,
    topic_name: &str,
    partition_id: i32,
    replica_count: usize,
    isr_count: usize,
) -> Result<()> {
    if replica_count > MAX_KAFKA_REPLICAS {
        return Err(DomainError::InvalidConfig(format!(
            "Topic Partition 副本数量超过 {MAX_KAFKA_REPLICAS} 个上限：{topic_name}/{partition_id}"
        )));
    }
    if isr_count > MAX_KAFKA_REPLICAS {
        return Err(DomainError::InvalidConfig(format!(
            "Topic Partition ISR 数量超过 {MAX_KAFKA_REPLICAS} 个上限：{topic_name}/{partition_id}"
        )));
    }
    let count = replica_count.checked_add(isr_count).ok_or_else(|| {
        DomainError::InvalidConfig("Kafka Partition 副本 ID 数量超出可计算范围".into())
    })?;
    let next_total = total_replica_ids.checked_add(count).ok_or_else(|| {
        DomainError::InvalidConfig("Kafka Partition 副本 ID 总数量超出可计算范围".into())
    })?;
    if next_total > MAX_KAFKA_PARTITION_REPLICA_IDS {
        return Err(DomainError::InvalidConfig(format!(
            "Kafka Partition 副本 ID 总数量超过 {MAX_KAFKA_PARTITION_REPLICA_IDS} 个上限"
        )));
    }
    *total_replica_ids = next_total;
    Ok(())
}

/// 校验对外暴露的请求时间预算，防止亚毫秒值转换后变成无效的零超时。
fn validate_request_timeout(request_timeout: Duration) -> Result<()> {
    if request_timeout.as_millis() == 0 {
        return Err(DomainError::InvalidConfig(
            "Kafka 请求超时必须至少为 1 毫秒".into(),
        ));
    }
    if request_timeout > MAX_KAFKA_REQUEST_TIMEOUT {
        return Err(DomainError::InvalidConfig(
            "Kafka 请求超时不能超过 60 秒".into(),
        ));
    }
    Ok(())
}

#[cfg(not(feature = "cmake-build"))]
fn native_client_unavailable(operation: &'static str) -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::Unsupported,
        operation,
        "当前构建未启用 Kafka native 客户端；请启用 cmake-build feature",
    ))
}

impl Default for RdkafkaTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl KafkaTransport for RdkafkaTransport {
    fn capabilities(&self) -> KafkaTransportCapabilities {
        KafkaTransportCapabilities::native(
            cfg!(feature = "cmake-build"),
            cfg!(feature = "kafka-tls"),
            cfg!(feature = "kafka-sasl"),
        )
    }
}

#[async_trait::async_trait]
impl KafkaDriver for RdkafkaTransport {
    fn transport_capabilities(&self) -> KafkaTransportCapabilities {
        self.capabilities()
    }

    async fn test_connection(&self, config: &KafkaClusterConfig) -> Result<()> {
        self.test_connection_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn test_connection_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        let driver = *self;
        let config = config.clone();
        debug!(
            operation = "kafka_test_connection",
            "starting Kafka connection test"
        );
        smol::unblock(move || driver.test_connection_blocking_with_cancel(&config, &cancelled))
            .await?;
        info!(
            operation = "kafka_test_connection",
            "Kafka connection test passed"
        );
        Ok(())
    }

    async fn cluster_metadata(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<ramag_domain::entities::KafkaClusterMetadata> {
        self.cluster_metadata_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn cluster_metadata_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<ramag_domain::entities::KafkaClusterMetadata> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        let driver = *self;
        let config = config.clone();
        smol::unblock(move || driver.cluster_metadata_blocking_with_cancel(&config, &cancelled))
            .await
    }

    async fn list_topics(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<Vec<ramag_domain::entities::KafkaTopic>> {
        self.list_topics_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn list_topics_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<ramag_domain::entities::KafkaTopic>> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        let driver = *self;
        let config = config.clone();
        smol::unblock(move || driver.list_topics_blocking_with_cancel(&config, &cancelled)).await
    }

    async fn list_consumer_groups(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<Vec<ramag_domain::entities::KafkaConsumerGroup>> {
        self.list_consumer_groups_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn list_consumer_groups_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<ramag_domain::entities::KafkaConsumerGroup>> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        let driver = *self;
        let config = config.clone();
        smol::unblock(move || driver.list_consumer_groups_blocking_with_cancel(&config, &cancelled))
            .await
    }

    async fn read_messages(
        &self,
        config: &KafkaClusterConfig,
        query: &ramag_domain::entities::KafkaMessageQuery,
    ) -> Result<ramag_domain::entities::KafkaMessagePage> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        query.validate().map_err(DomainError::InvalidConfig)?;
        let driver = *self;
        let config = config.clone();
        let query = query.clone();
        smol::unblock(move || driver.scan_messages_blocking(&config, &query, None)).await
    }

    async fn read_messages_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        query: &ramag_domain::entities::KafkaMessageQuery,
        cancelled: Arc<AtomicBool>,
    ) -> Result<ramag_domain::entities::KafkaMessagePage> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        query.validate().map_err(DomainError::InvalidConfig)?;
        let driver = *self;
        let config = config.clone();
        let query = query.clone();
        smol::unblock(move || {
            driver.scan_messages_blocking_with_cancel(&config, &query, None, &cancelled)
        })
        .await
    }

    async fn search_messages(
        &self,
        config: &KafkaClusterConfig,
        query: &ramag_domain::entities::KafkaMessageSearchQuery,
    ) -> Result<ramag_domain::entities::KafkaMessagePage> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        query.validate().map_err(DomainError::InvalidConfig)?;
        let driver = *self;
        let config = config.clone();
        let query = query.clone();
        smol::unblock(move || driver.scan_messages_blocking(&config, &query.scan, Some(&query)))
            .await
    }

    async fn search_messages_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        query: &ramag_domain::entities::KafkaMessageSearchQuery,
        cancelled: Arc<AtomicBool>,
    ) -> Result<ramag_domain::entities::KafkaMessagePage> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        query.validate().map_err(DomainError::InvalidConfig)?;
        let driver = *self;
        let config = config.clone();
        let query = query.clone();
        smol::unblock(move || {
            driver.scan_messages_blocking_with_cancel(
                &config,
                &query.scan,
                Some(&query),
                &cancelled,
            )
        })
        .await
    }

    async fn tail_messages(
        &self,
        config: &KafkaClusterConfig,
        request: &KafkaMessageTailRequest,
        sink: KafkaMessageTailSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        request.validate().map_err(DomainError::InvalidConfig)?;
        let driver = *self;
        let config = config.clone();
        let request = request.clone();
        smol::unblock(move || driver.tail_messages_blocking(&config, &request, sink, cancelled))
            .await
    }
}

#[async_trait::async_trait]
impl KafkaMonitoringDriver for RdkafkaTransport {
    async fn metrics_snapshot(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<ramag_domain::entities::KafkaMetricsSnapshot> {
        self.metrics_snapshot_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn metrics_snapshot_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<ramag_domain::entities::KafkaMetricsSnapshot> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        let driver = *self;
        let config = config.clone();
        smol::unblock(move || driver.metrics_snapshot_blocking_with_cancel(&config, &cancelled))
            .await
    }
}
#[cfg(test)]
mod tests;
