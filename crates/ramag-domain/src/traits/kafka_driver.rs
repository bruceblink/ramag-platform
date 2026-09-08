//! Kafka 只读与管理能力的领域边界。

use async_trait::async_trait;
use std::sync::{Arc, atomic::AtomicBool};

use crate::entities::{
    KafkaAcl, KafkaAclFilter, KafkaBrokerMetricsSnapshot, KafkaClusterConfig, KafkaClusterMetadata,
    KafkaConfigResource, KafkaConfigResourceType, KafkaConfigUpdateRequest, KafkaConsumerGroup,
    KafkaMessagePage, KafkaMessageQuery, KafkaMessageSearchQuery, KafkaMessageTailEvent,
    KafkaMessageTailRequest, KafkaMetricsSnapshot, KafkaTopic, KafkaTopicCreateRequest,
    KafkaTopicPartitionExpansion, KafkaTransportCapabilities,
};
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KafkaMessageTailSinkResult {
    Accepted,
    Backpressured,
    Closed,
}

pub type KafkaMessageTailSink =
    Arc<dyn Fn(KafkaMessageTailEvent) -> KafkaMessageTailSinkResult + Send + Sync>;

/// Kafka 观测端口；只读取协议快照，不读取消息正文或修改 Consumer Offset。
#[async_trait]
pub trait KafkaMonitoringDriver: Send + Sync {
    async fn metrics_snapshot(&self, _config: &KafkaClusterConfig) -> Result<KafkaMetricsSnapshot> {
        Err(crate::error::DomainError::NotImplemented(
            "metrics_snapshot".into(),
        ))
    }
}

/// 外部 Broker 运行指标端口；与 Kafka Protocol API 快照分开，避免混淆数据含义。
#[async_trait]
pub trait KafkaBrokerMetricsDriver: Send + Sync {
    async fn broker_metrics_snapshot(
        &self,
        _config: &KafkaClusterConfig,
    ) -> Result<KafkaBrokerMetricsSnapshot> {
        Err(crate::error::DomainError::NotImplemented(
            "broker_metrics_snapshot".into(),
        ))
    }
}

/// Kafka 读取端口；不会提交 Offset，也不修改集群状态。
#[async_trait]
pub trait KafkaDriver: Send + Sync {
    fn name(&self) -> &'static str {
        "kafka"
    }

    /// 返回当前传输适配器的能力快照；应用层据此展示明确的不可用原因。
    fn transport_capabilities(&self) -> KafkaTransportCapabilities {
        KafkaTransportCapabilities::unknown()
    }

    async fn test_connection(&self, config: &KafkaClusterConfig) -> Result<()>;

    async fn cluster_metadata(&self, _config: &KafkaClusterConfig) -> Result<KafkaClusterMetadata> {
        Err(crate::error::DomainError::NotImplemented(
            "cluster_metadata".into(),
        ))
    }

    /// 读取集群元数据并支持后台取消；旧驱动默认沿用不可取消的读取实现。
    async fn cluster_metadata_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        _cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaClusterMetadata> {
        self.cluster_metadata(config).await
    }

    async fn list_topics(&self, _config: &KafkaClusterConfig) -> Result<Vec<KafkaTopic>> {
        Err(crate::error::DomainError::NotImplemented(
            "list_topics".into(),
        ))
    }

    /// 读取 Topic 快照并支持后台取消；旧驱动默认沿用不可取消的读取实现。
    async fn list_topics_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        _cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<KafkaTopic>> {
        self.list_topics(config).await
    }

    async fn list_consumer_groups(
        &self,
        _config: &KafkaClusterConfig,
    ) -> Result<Vec<KafkaConsumerGroup>> {
        Err(crate::error::DomainError::NotImplemented(
            "list_consumer_groups".into(),
        ))
    }

    /// 读取消费者组快照并支持后台取消；旧驱动默认沿用不可取消的读取实现。
    async fn list_consumer_groups_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        _cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<KafkaConsumerGroup>> {
        self.list_consumer_groups(config).await
    }

    async fn read_messages(
        &self,
        _config: &KafkaClusterConfig,
        _query: &KafkaMessageQuery,
    ) -> Result<KafkaMessagePage> {
        Err(crate::error::DomainError::NotImplemented(
            "read_messages".into(),
        ))
    }

    /// 读取有限消息范围并支持后台取消；旧驱动默认沿用不可取消的读取实现。
    async fn read_messages_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaMessageQuery,
        _cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaMessagePage> {
        self.read_messages(config, query).await
    }

    async fn search_messages(
        &self,
        _config: &KafkaClusterConfig,
        _query: &KafkaMessageSearchQuery,
    ) -> Result<KafkaMessagePage> {
        Err(crate::error::DomainError::NotImplemented(
            "search_messages".into(),
        ))
    }

    /// 扫描有限消息范围并支持后台取消；旧驱动默认沿用不可取消的搜索实现。
    async fn search_messages_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaMessageSearchQuery,
        _cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaMessagePage> {
        self.search_messages(config, query).await
    }

    /// 持续读取明确 Topic/Partition 范围；调用方通过有界 sink 和取消句柄控制生命周期。
    async fn tail_messages(
        &self,
        _config: &KafkaClusterConfig,
        _request: &KafkaMessageTailRequest,
        _sink: KafkaMessageTailSink,
        _cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        Err(crate::error::DomainError::NotImplemented(
            "tail_messages".into(),
        ))
    }
}

/// Kafka 管理端口；与只读消息浏览能力分开注入，后续承载确认过的变更操作。
#[async_trait]
pub trait KafkaAdminDriver: Send + Sync {
    async fn test_admin_connection(&self, _config: &KafkaClusterConfig) -> Result<()> {
        Err(crate::error::DomainError::NotImplemented(
            "test_admin_connection".into(),
        ))
    }

    async fn create_topic(
        &self,
        _config: &KafkaClusterConfig,
        _request: &KafkaTopicCreateRequest,
    ) -> Result<()> {
        Err(crate::error::DomainError::NotImplemented(
            "create_topic".into(),
        ))
    }

    async fn delete_topic(&self, _config: &KafkaClusterConfig, _topic: &str) -> Result<()> {
        Err(crate::error::DomainError::NotImplemented(
            "delete_topic".into(),
        ))
    }

    async fn increase_topic_partitions(
        &self,
        _config: &KafkaClusterConfig,
        _request: &KafkaTopicPartitionExpansion,
    ) -> Result<()> {
        Err(crate::error::DomainError::NotImplemented(
            "increase_topic_partitions".into(),
        ))
    }

    async fn describe_configs(
        &self,
        _config: &KafkaClusterConfig,
        _resource_type: KafkaConfigResourceType,
        _resource_name: &str,
    ) -> Result<KafkaConfigResource> {
        Err(crate::error::DomainError::NotImplemented(
            "describe_configs".into(),
        ))
    }

    async fn update_config(
        &self,
        _config: &KafkaClusterConfig,
        _request: &KafkaConfigUpdateRequest,
    ) -> Result<()> {
        Err(crate::error::DomainError::NotImplemented(
            "update_config".into(),
        ))
    }

    async fn list_acls(
        &self,
        _config: &KafkaClusterConfig,
        _filter: &KafkaAclFilter,
    ) -> Result<Vec<KafkaAcl>> {
        Err(crate::error::DomainError::NotImplemented(
            "list_acls".into(),
        ))
    }

    async fn create_acl(&self, _config: &KafkaClusterConfig, _acl: &KafkaAcl) -> Result<()> {
        Err(crate::error::DomainError::NotImplemented(
            "create_acl".into(),
        ))
    }

    async fn delete_acl(&self, _config: &KafkaClusterConfig, _acl: &KafkaAcl) -> Result<()> {
        Err(crate::error::DomainError::NotImplemented(
            "delete_acl".into(),
        ))
    }
}
