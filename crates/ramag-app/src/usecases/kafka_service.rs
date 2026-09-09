//! Kafka 配置、元数据和有界消息读取的应用服务。

use std::sync::{Arc, atomic::AtomicBool};

use ramag_domain::entities::{
    KafkaAcl, KafkaBrokerMetricsSnapshot, KafkaClusterConfig, KafkaClusterId, KafkaClusterMetadata,
    KafkaConfigResource, KafkaConfigResourceType, KafkaConfigUpdateRequest, KafkaConnectConnector,
    KafkaConsumerGroup, KafkaMessagePage, KafkaMetricsSnapshot, KafkaSchemaRegistrySubject,
    KafkaTopic, KafkaTopicCreateRequest, KafkaTopicPartitionExpansion, KafkaTransportCapabilities,
    MAX_KAFKA_GROUP_OFFSETS, MAX_KAFKA_GROUP_TOTAL_ASSIGNMENTS, MAX_KAFKA_GROUP_TOTAL_MEMBERS,
    MAX_KAFKA_PARTITION_REPLICA_IDS, MAX_KAFKA_PARTITIONS,
};
use ramag_domain::error::{DomainError, READ_ONLY_MESSAGE, Result};
use ramag_domain::traits::{
    KafkaAdminDriver, KafkaBrokerMetricsDriver, KafkaConnectDriver, KafkaDriver,
    KafkaMonitoringDriver, KafkaSchemaRegistryDriver, Storage,
};

pub struct KafkaService {
    driver: Arc<dyn KafkaDriver>,
    admin_driver: Arc<dyn KafkaAdminDriver>,
    monitoring_driver: Arc<dyn KafkaMonitoringDriver>,
    broker_metrics_driver: Arc<dyn KafkaBrokerMetricsDriver>,
    schema_registry_driver: Arc<dyn KafkaSchemaRegistryDriver>,
    connect_driver: Arc<dyn KafkaConnectDriver>,
    storage: Arc<dyn Storage>,
}

mod acls;
mod connect;
mod connection;
mod logging;
mod messages;
mod schema_registry;
mod tail;
use logging::*;

impl KafkaService {
    pub fn new(driver: Arc<dyn KafkaDriver>, storage: Arc<dyn Storage>) -> Self {
        Self {
            driver,
            admin_driver: Arc::new(UnsupportedKafkaAdminDriver),
            monitoring_driver: Arc::new(UnsupportedKafkaMonitoringDriver),
            broker_metrics_driver: Arc::new(UnsupportedKafkaBrokerMetricsDriver),
            schema_registry_driver: Arc::new(UnsupportedKafkaSchemaRegistryDriver),
            connect_driver: Arc::new(UnsupportedKafkaConnectDriver),
            storage,
        }
    }

    pub fn with_admin_driver(mut self, admin_driver: Arc<dyn KafkaAdminDriver>) -> Self {
        self.admin_driver = admin_driver;
        self
    }

    pub fn with_monitoring_driver(
        mut self,
        monitoring_driver: Arc<dyn KafkaMonitoringDriver>,
    ) -> Self {
        self.monitoring_driver = monitoring_driver;
        self
    }

    pub fn with_broker_metrics_driver(
        mut self,
        broker_metrics_driver: Arc<dyn KafkaBrokerMetricsDriver>,
    ) -> Self {
        self.broker_metrics_driver = broker_metrics_driver;
        self
    }

    pub fn with_schema_registry_driver(
        mut self,
        schema_registry_driver: Arc<dyn KafkaSchemaRegistryDriver>,
    ) -> Self {
        self.schema_registry_driver = schema_registry_driver;
        self
    }

    pub fn with_connect_driver(mut self, connect_driver: Arc<dyn KafkaConnectDriver>) -> Self {
        self.connect_driver = connect_driver;
        self
    }

    /// 向 UI 暴露当前适配器的能力，不泄露具体 Kafka 客户端类型。
    pub fn transport_capabilities(&self) -> KafkaTransportCapabilities {
        self.driver.transport_capabilities()
    }

    /// 读取本地保存的 Kafka 集群配置，不包含消息正文或运行时快照。
    pub async fn list_clusters(&self) -> Result<Vec<KafkaClusterConfig>> {
        let result = self.storage.list_kafka_clusters().await;
        log_storage_result("kafka_cluster_list", &result);
        result
    }

    pub async fn get_cluster(&self, id: &KafkaClusterId) -> Result<Option<KafkaClusterConfig>> {
        let result = self.storage.get_kafka_cluster(id).await;
        log_storage_result("kafka_cluster_get", &result);
        result
    }

    /// 保存前执行领域校验；密码仍由 Storage 的加密实现负责保护。
    pub async fn save_cluster(&self, config: &KafkaClusterConfig) -> Result<()> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        let result = self.storage.save_kafka_cluster(config).await;
        log_storage_result("kafka_cluster_save", &result);
        result
    }

    pub async fn delete_cluster(&self, id: &KafkaClusterId) -> Result<()> {
        let result = self.storage.delete_kafka_cluster(id).await;
        log_storage_result("kafka_cluster_delete", &result);
        result
    }

    pub async fn cluster_metadata(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<KafkaClusterMetadata> {
        self.cluster_metadata_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    /// 读取集群元数据并把取消信号传到读取适配器。
    pub async fn cluster_metadata_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaClusterMetadata> {
        validate_config(config)?;
        let started = std::time::Instant::now();
        let result = self
            .driver
            .cluster_metadata_with_cancel(config, cancelled)
            .await;
        log_runtime_result(
            "kafka_cluster_metadata",
            config,
            started,
            result.as_ref().ok().map(|metadata| metadata.brokers.len()),
            result.as_ref().err(),
        );
        result.and_then(validate_cluster_metadata)
    }

    pub async fn list_topics(&self, config: &KafkaClusterConfig) -> Result<Vec<KafkaTopic>> {
        self.list_topics_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    /// 读取 Topic 快照并把取消信号传到读取适配器。
    pub async fn list_topics_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<KafkaTopic>> {
        validate_config(config)?;
        let started = std::time::Instant::now();
        let result = self.driver.list_topics_with_cancel(config, cancelled).await;
        log_runtime_result(
            "kafka_topic_list",
            config,
            started,
            result.as_ref().ok().map(Vec::len),
            result.as_ref().err(),
        );
        result.and_then(validate_topics)
    }

    /// 读取消费者组、成员和已提交 Offset；该查询只读，不加入任何业务消费者组。
    pub async fn list_consumer_groups(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<Vec<KafkaConsumerGroup>> {
        self.list_consumer_groups_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    /// 读取消费者组、成员和已提交 Offset，并把取消信号传到读取适配器。
    pub async fn list_consumer_groups_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<KafkaConsumerGroup>> {
        validate_config(config)?;
        let started = std::time::Instant::now();
        let result = self
            .driver
            .list_consumer_groups_with_cancel(config, cancelled)
            .await;
        log_runtime_result(
            "kafka_consumer_group_list",
            config,
            started,
            result.as_ref().ok().map(Vec::len),
            result.as_ref().err(),
        );
        result.and_then(validate_consumer_groups)
    }

    /// 读取协议指标快照；应用层再次校验范围和唯一性，缺失字段保持为未知。
    pub async fn metrics_snapshot(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<KafkaMetricsSnapshot> {
        self.metrics_snapshot_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    /// 读取协议指标快照并把取消信号传到观测适配器。
    pub async fn metrics_snapshot_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaMetricsSnapshot> {
        validate_config(config)?;
        let started = std::time::Instant::now();
        let result = self
            .monitoring_driver
            .metrics_snapshot_with_cancel(config, cancelled)
            .await;
        tracing::info!(
            operation = "kafka_metrics_snapshot",
            cluster_id = %config.id,
            elapsed_ms = started.elapsed().as_millis(),
            success = result.is_ok(),
            result_topic_count = result.as_ref().map_or(0, |snapshot| snapshot.topics.len()),
            result_group_count = result
                .as_ref()
                .map_or(0, |snapshot| snapshot.consumer_groups.len()),
            "Kafka metrics snapshot completed"
        );
        result.and_then(validate_metrics_snapshot)
    }

    /// 读取外部 Broker 运行指标；它与 Kafka Protocol API 快照分别校验和记录。
    pub async fn broker_metrics_snapshot(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<KafkaBrokerMetricsSnapshot> {
        self.broker_metrics_snapshot_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    /// 读取外部 Broker 指标并把取消信号传到外部指标适配器。
    pub async fn broker_metrics_snapshot_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaBrokerMetricsSnapshot> {
        validate_config(config)?;
        let started = std::time::Instant::now();
        let result = self
            .broker_metrics_driver
            .broker_metrics_snapshot_with_cancel(config, cancelled)
            .await;
        let result = result.and_then(validate_broker_metrics_snapshot);
        tracing::info!(
            operation = "kafka_broker_metrics_snapshot",
            cluster_id = %config.id,
            elapsed_ms = started.elapsed().as_millis(),
            success = result.is_ok(),
            result_broker_count = result.as_ref().map_or(0, |snapshot| snapshot.brokers.len()),
            "Kafka external broker metrics snapshot completed"
        );
        result
    }

    pub async fn create_topic(
        &self,
        config: &KafkaClusterConfig,
        request: &KafkaTopicCreateRequest,
    ) -> Result<()> {
        validate_admin_request(config, request.validate())?;
        let started = std::time::Instant::now();
        let result = self.admin_driver.create_topic(config, request).await;
        log_admin_result(
            "kafka_topic_create",
            config,
            &request.name,
            started,
            &result,
        );
        result
    }

    pub async fn delete_topic(&self, config: &KafkaClusterConfig, topic: &str) -> Result<()> {
        validate_config(config)?;
        ramag_domain::entities::validate_kafka_managed_topic_name(topic)
            .map_err(DomainError::InvalidConfig)?;
        ensure_admin_enabled(config)?;
        let started = std::time::Instant::now();
        let result = self.admin_driver.delete_topic(config, topic).await;
        log_admin_result("kafka_topic_delete", config, topic, started, &result);
        result
    }

    pub async fn increase_topic_partitions(
        &self,
        config: &KafkaClusterConfig,
        request: &KafkaTopicPartitionExpansion,
    ) -> Result<()> {
        validate_admin_request(config, request.validate())?;
        let started = std::time::Instant::now();
        let result = self
            .admin_driver
            .increase_topic_partitions(config, request)
            .await;
        log_admin_result(
            "kafka_topic_partition_increase",
            config,
            &request.name,
            started,
            &result,
        );
        result
    }

    /// 读取指定 Topic 或 Broker 的配置快照；只读模式仍允许查看配置来源和可见性。
    pub async fn describe_configs(
        &self,
        config: &KafkaClusterConfig,
        resource_type: KafkaConfigResourceType,
        resource_name: &str,
    ) -> Result<KafkaConfigResource> {
        self.describe_configs_with_cancel(
            config,
            resource_type,
            resource_name,
            Arc::new(AtomicBool::new(false)),
        )
        .await
    }

    /// 读取 Kafka 配置快照并把取消信号传到管理适配器。
    pub async fn describe_configs_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        resource_type: KafkaConfigResourceType,
        resource_name: &str,
        cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaConfigResource> {
        validate_config(config)?;
        resource_type
            .validate_resource_name(resource_name)
            .map_err(DomainError::InvalidConfig)?;
        let started = std::time::Instant::now();
        let result = self
            .admin_driver
            .describe_configs_with_cancel(config, resource_type, resource_name, cancelled)
            .await
            .and_then(|resource| validate_config_resource(resource, resource_type, resource_name));
        log_config_read_result(
            "kafka_config_describe",
            config,
            resource_type,
            resource_name,
            started,
            &result,
        );
        result
    }

    /// 修改单个动态配置项；调用方必须明确开启管理模式，具体资源来源由驱动再次核验。
    pub async fn update_config(
        &self,
        config: &KafkaClusterConfig,
        request: &KafkaConfigUpdateRequest,
    ) -> Result<()> {
        validate_admin_request(config, request.validate())?;
        let started = std::time::Instant::now();
        let result = self.admin_driver.update_config(config, request).await;
        log_config_update_result("kafka_config_update", config, request, started, &result);
        result
    }
}

struct UnsupportedKafkaAdminDriver;

impl KafkaAdminDriver for UnsupportedKafkaAdminDriver {}

struct UnsupportedKafkaMonitoringDriver;

impl KafkaMonitoringDriver for UnsupportedKafkaMonitoringDriver {}

struct UnsupportedKafkaBrokerMetricsDriver;

impl KafkaBrokerMetricsDriver for UnsupportedKafkaBrokerMetricsDriver {}

struct UnsupportedKafkaSchemaRegistryDriver;

impl KafkaSchemaRegistryDriver for UnsupportedKafkaSchemaRegistryDriver {}

struct UnsupportedKafkaConnectDriver;

impl KafkaConnectDriver for UnsupportedKafkaConnectDriver {}

/// 在应用层再次校验驱动返回的页，避免替换基础设施实现时绕过领域资源上限。
fn validate_message_page(page: KafkaMessagePage) -> Result<KafkaMessagePage> {
    page.validate()
        .map(|()| page)
        .map_err(DomainError::InvalidConfig)
}

fn validate_metrics_snapshot(snapshot: KafkaMetricsSnapshot) -> Result<KafkaMetricsSnapshot> {
    snapshot
        .validate()
        .map(|()| snapshot)
        .map_err(DomainError::InvalidConfig)
}

fn validate_broker_metrics_snapshot(
    snapshot: KafkaBrokerMetricsSnapshot,
) -> Result<KafkaBrokerMetricsSnapshot> {
    snapshot
        .validate()
        .map(|()| snapshot)
        .map_err(DomainError::InvalidConfig)
}

/// 在应用层校验驱动返回的集群快照，避免不完整或重复的 Broker 污染 UI。
fn validate_cluster_metadata(metadata: KafkaClusterMetadata) -> Result<KafkaClusterMetadata> {
    metadata
        .validate()
        .map(|()| metadata)
        .map_err(DomainError::InvalidConfig)
}

/// 在应用层校验 Topic 列表及其 Partition 快照，保持替换驱动后的边界不变。
fn validate_topics(topics: Vec<KafkaTopic>) -> Result<Vec<KafkaTopic>> {
    if topics.len() > ramag_domain::entities::MAX_KAFKA_TOPICS {
        return Err(DomainError::InvalidConfig(format!(
            "Kafka Topic 数量超过 {} 个上限",
            ramag_domain::entities::MAX_KAFKA_TOPICS
        )));
    }
    let mut names = std::collections::HashSet::with_capacity(topics.len());
    let mut partition_count = 0usize;
    let mut replica_id_count = 0usize;
    for topic in &topics {
        topic.validate().map_err(DomainError::InvalidConfig)?;
        validate_topic_partition_budget(&mut partition_count, topic.partitions.len())?;
        let topic_replica_id_count = topic
            .partitions
            .iter()
            .try_fold(0usize, |total, partition| {
                total
                    .checked_add(partition.replicas.len())
                    .and_then(|total| total.checked_add(partition.isr.len()))
            })
            .ok_or_else(|| {
                DomainError::InvalidConfig("Kafka Partition 副本 ID 数量超出可计算范围".into())
            })?;
        validate_replica_id_budget(&mut replica_id_count, topic_replica_id_count)?;
        if !names.insert(topic.name.as_str()) {
            return Err(DomainError::InvalidConfig(format!(
                "Kafka Topic 名称重复：{}",
                topic.name
            )));
        }
    }
    Ok(topics)
}

fn validate_replica_id_budget(total: &mut usize, count: usize) -> Result<()> {
    let next_total = total.checked_add(count).ok_or_else(|| {
        DomainError::InvalidConfig("Kafka Partition 副本 ID 总数量超出可计算范围".into())
    })?;
    if next_total > MAX_KAFKA_PARTITION_REPLICA_IDS {
        return Err(DomainError::InvalidConfig(format!(
            "Kafka Partition 副本 ID 总数量超过 {MAX_KAFKA_PARTITION_REPLICA_IDS} 个上限"
        )));
    }
    *total = next_total;
    Ok(())
}

fn validate_topic_partition_budget(total: &mut usize, count: usize) -> Result<()> {
    let next_total = total
        .checked_add(count)
        .ok_or_else(|| DomainError::InvalidConfig("Kafka Partition 总数量超出可计算范围".into()))?;
    if next_total > MAX_KAFKA_PARTITIONS {
        return Err(DomainError::InvalidConfig(format!(
            "Kafka Partition 总数量超过 {MAX_KAFKA_PARTITIONS} 个上限"
        )));
    }
    *total = next_total;
    Ok(())
}

/// 在应用边界校验消费者组快照，防止驱动实现绕过数量、成员和 Offset 约束。
fn validate_consumer_groups(groups: Vec<KafkaConsumerGroup>) -> Result<Vec<KafkaConsumerGroup>> {
    if groups.len() > ramag_domain::entities::MAX_KAFKA_CONSUMER_GROUPS {
        return Err(DomainError::InvalidConfig(format!(
            "消费者组数量超过 {} 个上限",
            ramag_domain::entities::MAX_KAFKA_CONSUMER_GROUPS
        )));
    }
    let mut ids = std::collections::HashSet::with_capacity(groups.len());
    let mut member_count = 0usize;
    let mut assignment_count = 0usize;
    let mut offset_count = 0usize;
    for group in &groups {
        group.validate().map_err(DomainError::InvalidConfig)?;
        validate_consumer_group_member_budget(&mut member_count, group.members.len())?;
        for member in &group.members {
            validate_consumer_group_assignment_budget(
                &mut assignment_count,
                member.assigned_partitions.len(),
            )?;
        }
        validate_consumer_group_offset_budget(&mut offset_count, group.offsets.len())?;
        if !ids.insert(group.group_id.as_str()) {
            return Err(DomainError::InvalidConfig(format!(
                "消费者组 ID 重复：{}",
                group.group_id
            )));
        }
    }
    Ok(groups)
}

fn validate_consumer_group_member_budget(total: &mut usize, count: usize) -> Result<()> {
    let next_total = total
        .checked_add(count)
        .ok_or_else(|| DomainError::InvalidConfig("消费者组成员总数量超出可计算范围".into()))?;
    if next_total > MAX_KAFKA_GROUP_TOTAL_MEMBERS {
        return Err(DomainError::InvalidConfig(format!(
            "消费者组成员总数量超过 {MAX_KAFKA_GROUP_TOTAL_MEMBERS} 个上限"
        )));
    }
    *total = next_total;
    Ok(())
}

fn validate_consumer_group_assignment_budget(total: &mut usize, count: usize) -> Result<()> {
    let next_total = total
        .checked_add(count)
        .ok_or_else(|| DomainError::InvalidConfig("消费者组分配总数量超出可计算范围".into()))?;
    if next_total > MAX_KAFKA_GROUP_TOTAL_ASSIGNMENTS {
        return Err(DomainError::InvalidConfig(format!(
            "消费者组分配总数量超过 {MAX_KAFKA_GROUP_TOTAL_ASSIGNMENTS} 个上限"
        )));
    }
    *total = next_total;
    Ok(())
}

fn validate_consumer_group_offset_budget(total: &mut usize, count: usize) -> Result<()> {
    let next_total = total
        .checked_add(count)
        .ok_or_else(|| DomainError::InvalidConfig("消费者组 Offset 总数量超出可计算范围".into()))?;
    if next_total > MAX_KAFKA_GROUP_OFFSETS {
        return Err(DomainError::InvalidConfig(format!(
            "消费者组 Offset 总数量超过 {MAX_KAFKA_GROUP_OFFSETS} 个上限"
        )));
    }
    *total = next_total;
    Ok(())
}

fn validate_acls(acls: Vec<KafkaAcl>) -> Result<Vec<KafkaAcl>> {
    if acls.len() > ramag_domain::entities::MAX_KAFKA_ACLS {
        return Err(DomainError::InvalidConfig(format!(
            "Kafka ACL 数量超过 {} 个上限",
            ramag_domain::entities::MAX_KAFKA_ACLS
        )));
    }
    for acl in &acls {
        acl.validate().map_err(DomainError::InvalidConfig)?;
    }
    Ok(acls)
}

fn validate_config(config: &KafkaClusterConfig) -> Result<()> {
    config.validate().map_err(DomainError::InvalidConfig)
}

fn validate_admin_request(
    config: &KafkaClusterConfig,
    request: std::result::Result<(), String>,
) -> Result<()> {
    validate_config(config)?;
    request.map_err(DomainError::InvalidConfig)?;
    ensure_admin_enabled(config)
}

fn ensure_admin_enabled(config: &KafkaClusterConfig) -> Result<()> {
    if config.read_only.allows_admin() {
        Ok(())
    } else {
        Err(DomainError::Forbidden(READ_ONLY_MESSAGE.into()))
    }
}

fn validate_config_resource(
    resource: KafkaConfigResource,
    expected_type: KafkaConfigResourceType,
    expected_name: &str,
) -> Result<KafkaConfigResource> {
    resource.validate().map_err(DomainError::InvalidConfig)?;
    if resource.resource_type != expected_type || resource.resource_name != expected_name {
        return Err(DomainError::InvalidConfig(
            "Kafka 配置资源与请求不一致".into(),
        ));
    }
    Ok(resource)
}

#[cfg(test)]
#[path = "kafka_service_tests.rs"]
mod tests;
