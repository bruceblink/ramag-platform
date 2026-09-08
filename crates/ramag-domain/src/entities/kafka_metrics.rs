use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::kafka_validation::{validate_optional_offset, validate_optional_single_line};
use super::{
    KafkaClusterMetadata, KafkaConsumerGroup, KafkaConsumerGroupOffset, KafkaPartition, KafkaTopic,
    MAX_KAFKA_ACL_RESOURCE_NAME_BYTES, MAX_KAFKA_CONFIG_VALUE_BYTES, MAX_KAFKA_GROUP_MEMBERS,
    MAX_KAFKA_GROUP_OFFSETS, MAX_KAFKA_PARTITIONS, MAX_KAFKA_TOPICS, MAX_KAFKA_VERSION_BYTES,
};

/// 指标快照的采集来源；Broker 运行指标由独立外部适配器提供。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KafkaMetricsSource {
    KafkaProtocol,
    ExternalBrokerMetrics,
}

impl KafkaMetricsSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::KafkaProtocol => "Kafka Protocol API",
            Self::ExternalBrokerMetrics => "外部 Broker 指标",
        }
    }
}

/// 快照本身可以表示部分数据或数据不可用；传输失败仍通过 `DomainError` 返回。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KafkaMetricsSnapshotState {
    Ready,
    Partial,
    NoData,
    PermissionDenied,
    SourceNotConfigured,
    CollectionFailed,
}

impl KafkaMetricsSnapshotState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ready => "已更新",
            Self::Partial => "部分数据",
            Self::NoData => "暂无数据",
            Self::PermissionDenied => "权限不足",
            Self::SourceNotConfigured => "未配置数据源",
            Self::CollectionFailed => "采集失败",
        }
    }
}

/// 集群级协议指标；`None` 表示这次采集没有得到该项数据，不把缺失转换为 0。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KafkaClusterMetrics {
    pub broker_count: Option<usize>,
    pub topic_count: Option<usize>,
    pub partition_count: Option<usize>,
    pub total_lag: Option<i64>,
    pub max_lag: Option<i64>,
    pub message_rate_per_second: Option<f64>,
    pub under_replicated_partitions: Option<usize>,
    pub offline_partitions: Option<usize>,
}

/// Partition 的一次可比较快照；消息速率由连续两次 high watermark 样本计算。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KafkaPartitionMetrics {
    pub topic: String,
    pub partition: i32,
    pub leader: Option<i32>,
    pub replica_count: Option<usize>,
    pub isr_count: Option<usize>,
    pub low_watermark: Option<i64>,
    pub high_watermark: Option<i64>,
    pub under_replicated: Option<bool>,
    pub offline: Option<bool>,
    pub message_rate_per_second: Option<f64>,
}

/// Topic 级聚合，同时保留受限的 Partition 明细供 UI 展示和速率采样。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KafkaTopicMetrics {
    pub name: String,
    pub partition_count: Option<usize>,
    pub low_watermark: Option<i64>,
    pub high_watermark: Option<i64>,
    pub message_rate_per_second: Option<f64>,
    pub under_replicated_partitions: Option<usize>,
    pub offline_partitions: Option<usize>,
    pub partitions: Vec<KafkaPartitionMetrics>,
}

/// Consumer Group 的 Lag 聚合；未知 Offset 不会被当作零 Lag。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KafkaConsumerGroupMetrics {
    pub group_id: String,
    pub state: Option<String>,
    pub member_count: Option<usize>,
    pub assigned_partition_count: Option<usize>,
    pub offset_count: Option<usize>,
    pub total_lag: Option<i64>,
    pub max_lag: Option<i64>,
}

/// 在一个时间点采集的 Kafka 协议指标；正文只保留结构化元数据，不包含消息内容。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KafkaMetricsSnapshot {
    pub cluster_id: Option<String>,
    pub sampled_at: DateTime<Utc>,
    pub source: KafkaMetricsSource,
    pub state: KafkaMetricsSnapshotState,
    #[serde(default)]
    pub error: Option<String>,
    pub cluster: KafkaClusterMetrics,
    pub topics: Vec<KafkaTopicMetrics>,
    pub consumer_groups: Vec<KafkaConsumerGroupMetrics>,
}

impl KafkaMetricsSnapshot {
    /// 用已读取的元数据、Topic 和 Consumer Group 构造一次协议指标快照。
    pub fn from_runtime(
        sampled_at: DateTime<Utc>,
        metadata: &KafkaClusterMetadata,
        topics: &[KafkaTopic],
        groups: &[KafkaConsumerGroup],
    ) -> Self {
        let topic_metrics = topics
            .iter()
            .map(topic_metrics_from_topic)
            .collect::<Vec<_>>();
        let group_metrics = groups
            .iter()
            .map(group_metrics_from_group)
            .collect::<Vec<_>>();
        let partition_count = topic_metrics
            .iter()
            .map(|topic| topic.partitions.len())
            .sum();

        Self {
            cluster_id: metadata.cluster_id.clone(),
            sampled_at,
            source: KafkaMetricsSource::KafkaProtocol,
            state: snapshot_state(metadata, &topic_metrics, &group_metrics),
            error: None,
            cluster: KafkaClusterMetrics {
                broker_count: Some(metadata.brokers.len()),
                topic_count: Some(topic_metrics.len()),
                partition_count: Some(partition_count),
                total_lag: aggregate_lag(group_metrics.iter().map(|group| group.total_lag)),
                max_lag: aggregate_max_lag(group_metrics.iter().map(|group| group.max_lag)),
                message_rate_per_second: None,
                under_replicated_partitions: sum_optional_usize(
                    topic_metrics
                        .iter()
                        .map(|topic| topic.under_replicated_partitions),
                ),
                offline_partitions: sum_optional_usize(
                    topic_metrics.iter().map(|topic| topic.offline_partitions),
                ),
            },
            topics: topic_metrics,
            consumer_groups: group_metrics,
        }
    }

    /// 根据上一次同一集群的快照计算 Partition、Topic 和集群消息速率。
    /// Offset 回退、时间不递增、Partition 新出现或数据缺失时保留未知值。
    pub fn with_high_watermark_rates(mut self, previous: Option<&Self>) -> Self {
        let previous = previous.filter(|previous| {
            self.cluster_id.is_some()
                && self.cluster_id == previous.cluster_id
                && self.source == previous.source
                && matches!(
                    previous.state,
                    KafkaMetricsSnapshotState::Ready | KafkaMetricsSnapshotState::Partial
                )
                && matches!(
                    self.state,
                    KafkaMetricsSnapshotState::Ready | KafkaMetricsSnapshotState::Partial
                )
        });

        let previous_high_watermarks = previous.map(|snapshot| {
            let mut lookup = HashMap::new();
            for topic in &snapshot.topics {
                for partition in &topic.partitions {
                    lookup
                        .entry((topic.name.as_str(), partition.partition))
                        .or_insert(partition.high_watermark);
                }
            }
            lookup
        });

        for topic in &mut self.topics {
            let mut all_rates_known = !topic.partitions.is_empty();
            let mut total_rate = 0.0;
            for partition in &mut topic.partitions {
                let previous_high_watermark = previous_high_watermarks
                    .as_ref()
                    .and_then(|lookup| lookup.get(&(topic.name.as_str(), partition.partition)))
                    .copied();
                partition.message_rate_per_second = previous_high_watermark.and_then(|old| {
                    watermark_rate(
                        old,
                        partition.high_watermark,
                        previous.map(|snapshot| snapshot.sampled_at),
                        self.sampled_at,
                    )
                });
                if let Some(rate) = partition.message_rate_per_second {
                    total_rate += rate;
                } else {
                    all_rates_known = false;
                }
            }
            topic.message_rate_per_second = all_rates_known.then_some(total_rate);
        }

        let mut all_topic_rates_known = !self.topics.is_empty();
        let mut total_rate = 0.0;
        for topic in &self.topics {
            if let Some(rate) = topic.message_rate_per_second {
                total_rate += rate;
            } else {
                all_topic_rates_known = false;
            }
        }
        self.cluster.message_rate_per_second = all_topic_rates_known.then_some(total_rate);
        self
    }

    /// 校验快照的范围、唯一性、Offset 关系和速率数值，保护 UI 与替换驱动。
    pub fn validate(&self) -> Result<(), String> {
        validate_optional_single_line(
            "指标快照 Cluster ID",
            self.cluster_id.as_deref(),
            super::MAX_KAFKA_CLUSTER_ID_BYTES,
        )?;
        validate_optional_single_line(
            "指标快照错误",
            self.error.as_deref(),
            MAX_KAFKA_CONFIG_VALUE_BYTES,
        )?;
        validate_cluster_metrics(&self.cluster)?;
        if self.topics.len() > MAX_KAFKA_TOPICS {
            return Err(format!("指标 Topic 数量超过 {MAX_KAFKA_TOPICS} 个上限"));
        }
        let mut topic_names = HashSet::with_capacity(self.topics.len());
        let mut partition_count = 0usize;
        for topic in &self.topics {
            if !topic_names.insert(topic.name.as_str()) {
                return Err(format!("指标 Topic 名称重复：{}", topic.name));
            }
            validate_topic_metrics(topic)?;
            partition_count = partition_count.saturating_add(topic.partitions.len());
        }
        if partition_count > MAX_KAFKA_PARTITIONS {
            return Err(format!(
                "指标 Partition 数量超过 {MAX_KAFKA_PARTITIONS} 个上限"
            ));
        }
        if self.consumer_groups.len() > super::MAX_KAFKA_CONSUMER_GROUPS {
            return Err(format!(
                "指标消费者组数量超过 {} 个上限",
                super::MAX_KAFKA_CONSUMER_GROUPS
            ));
        }
        let mut group_ids = HashSet::with_capacity(self.consumer_groups.len());
        for group in &self.consumer_groups {
            validate_group_metrics(group)?;
            if !group_ids.insert(group.group_id.as_str()) {
                return Err(format!("指标消费者组 ID 重复：{}", group.group_id));
            }
        }
        Ok(())
    }
}

fn topic_metrics_from_topic(topic: &KafkaTopic) -> KafkaTopicMetrics {
    let partitions = topic
        .partitions
        .iter()
        .map(|partition| partition_metrics_from_partition(&topic.name, partition))
        .collect::<Vec<_>>();
    KafkaTopicMetrics {
        name: topic.name.clone(),
        partition_count: Some(partitions.len()),
        low_watermark: sum_optional_i64(partitions.iter().map(|partition| partition.low_watermark)),
        high_watermark: sum_optional_i64(
            partitions.iter().map(|partition| partition.high_watermark),
        ),
        message_rate_per_second: None,
        under_replicated_partitions: Some(
            partitions
                .iter()
                .filter(|partition| partition.under_replicated == Some(true))
                .count(),
        ),
        offline_partitions: Some(
            partitions
                .iter()
                .filter(|partition| partition.offline == Some(true))
                .count(),
        ),
        partitions,
    }
}

fn snapshot_state(
    metadata: &KafkaClusterMetadata,
    topics: &[KafkaTopicMetrics],
    groups: &[KafkaConsumerGroupMetrics],
) -> KafkaMetricsSnapshotState {
    if metadata.brokers.is_empty() && topics.is_empty() {
        return KafkaMetricsSnapshotState::NoData;
    }

    let topic_data_is_partial = topics.iter().any(|topic| {
        topic.partitions.is_empty()
            || topic.partitions.iter().any(|partition| {
                partition.low_watermark.is_none() || partition.high_watermark.is_none()
            })
    });
    let group_data_is_partial = groups.iter().any(|group| {
        group.state.is_none()
            || group.offset_count == Some(0)
            || (group.offset_count.is_some() && group.total_lag.is_none())
    });
    if metadata.brokers.is_empty()
        || metadata.cluster_id.is_none()
        || topic_data_is_partial
        || group_data_is_partial
    {
        KafkaMetricsSnapshotState::Partial
    } else {
        KafkaMetricsSnapshotState::Ready
    }
}

fn partition_metrics_from_partition(
    topic: &str,
    partition: &KafkaPartition,
) -> KafkaPartitionMetrics {
    KafkaPartitionMetrics {
        topic: topic.to_owned(),
        partition: partition.id,
        leader: partition.leader,
        replica_count: Some(partition.replicas.len()),
        isr_count: Some(partition.isr.len()),
        low_watermark: partition.low_watermark,
        high_watermark: partition.high_watermark,
        under_replicated: (!partition.replicas.is_empty())
            .then_some(partition.isr.len() < partition.replicas.len()),
        offline: Some(partition.leader.is_none()),
        message_rate_per_second: None,
    }
}

fn group_metrics_from_group(group: &KafkaConsumerGroup) -> KafkaConsumerGroupMetrics {
    let assigned_partition_count = group
        .members
        .iter()
        .map(|member| member.assigned_partitions.len())
        .sum();
    let (total_lag, max_lag) = lag_summary(&group.offsets);
    KafkaConsumerGroupMetrics {
        group_id: group.group_id.clone(),
        state: group.state.clone(),
        member_count: Some(group.members.len()),
        assigned_partition_count: Some(assigned_partition_count),
        offset_count: Some(group.offsets.len()),
        total_lag,
        max_lag,
    }
}

fn lag_summary(offsets: &[KafkaConsumerGroupOffset]) -> (Option<i64>, Option<i64>) {
    if offsets.is_empty() {
        return (None, None);
    }
    let mut total = 0i64;
    let mut maximum = 0i64;
    for offset in offsets {
        let Some(lag) = offset.lag.filter(|lag| *lag >= 0) else {
            return (None, None);
        };
        let Some(next_total) = total.checked_add(lag) else {
            return (None, None);
        };
        total = next_total;
        maximum = maximum.max(lag);
    }
    (Some(total), Some(maximum))
}

fn aggregate_lag(values: impl Iterator<Item = Option<i64>>) -> Option<i64> {
    let mut total = 0i64;
    let mut count = 0usize;
    for value in values {
        total = total.checked_add(value?)?;
        count = count.saturating_add(1);
    }
    (count > 0).then_some(total)
}

fn aggregate_max_lag(values: impl Iterator<Item = Option<i64>>) -> Option<i64> {
    let mut maximum = 0i64;
    let mut count = 0usize;
    for value in values {
        maximum = maximum.max(value?);
        count = count.saturating_add(1);
    }
    (count > 0).then_some(maximum)
}

fn sum_optional_i64(values: impl Iterator<Item = Option<i64>>) -> Option<i64> {
    let mut total = 0i64;
    let mut count = 0usize;
    for value in values {
        total = total.checked_add(value?)?;
        count = count.saturating_add(1);
    }
    (count > 0).then_some(total)
}

fn sum_optional_usize(values: impl Iterator<Item = Option<usize>>) -> Option<usize> {
    let mut total = 0usize;
    for value in values {
        total = total.checked_add(value?)?;
    }
    Some(total)
}

fn watermark_rate(
    previous: Option<i64>,
    current: Option<i64>,
    previous_at: Option<DateTime<Utc>>,
    current_at: DateTime<Utc>,
) -> Option<f64> {
    let (Some(previous), Some(current), Some(previous_at)) = (previous, current, previous_at)
    else {
        return None;
    };
    let delta = current.checked_sub(previous)?;
    if delta < 0 {
        return None;
    }
    let seconds = current_at
        .signed_duration_since(previous_at)
        .to_std()
        .ok()?
        .as_secs_f64();
    if seconds <= 0.0 {
        return None;
    }
    let rate = delta as f64 / seconds;
    rate.is_finite().then_some(rate)
}

fn validate_cluster_metrics(metrics: &KafkaClusterMetrics) -> Result<(), String> {
    validate_optional_count(metrics.broker_count, super::MAX_KAFKA_BROKERS, "Broker")?;
    validate_optional_count(metrics.topic_count, MAX_KAFKA_TOPICS, "Topic")?;
    validate_optional_count(metrics.partition_count, MAX_KAFKA_PARTITIONS, "Partition")?;
    validate_optional_count(
        metrics.under_replicated_partitions,
        MAX_KAFKA_PARTITIONS,
        "异常副本 Partition",
    )?;
    validate_optional_count(
        metrics.offline_partitions,
        MAX_KAFKA_PARTITIONS,
        "离线 Partition",
    )?;
    validate_lag(metrics.total_lag, "集群总 Lag")?;
    validate_lag(metrics.max_lag, "集群最大 Lag")?;
    validate_rate(metrics.message_rate_per_second, "集群消息速率")
}

fn validate_topic_metrics(topic: &KafkaTopicMetrics) -> Result<(), String> {
    super::validate_kafka_topic_name(&topic.name)?;
    validate_optional_count(
        topic.partition_count,
        MAX_KAFKA_PARTITIONS,
        "Topic Partition",
    )?;
    validate_optional_offset(topic.low_watermark)?;
    validate_optional_offset(topic.high_watermark)?;
    if let (Some(low), Some(high)) = (topic.low_watermark, topic.high_watermark)
        && low > high
    {
        return Err(format!("Topic 指标首尾 Offset 无效：{}", topic.name));
    }
    validate_rate(topic.message_rate_per_second, "Topic 消息速率")?;
    validate_optional_count(
        topic.under_replicated_partitions,
        MAX_KAFKA_PARTITIONS,
        "异常副本 Partition",
    )?;
    validate_optional_count(
        topic.offline_partitions,
        MAX_KAFKA_PARTITIONS,
        "离线 Partition",
    )?;
    let mut partitions = HashSet::with_capacity(topic.partitions.len());
    for partition in &topic.partitions {
        if partition.topic != topic.name {
            return Err("Partition 指标 Topic 与父级 Topic 不一致".into());
        }
        if !partitions.insert(partition.partition) {
            return Err(format!(
                "Topic 指标 Partition 重复：{}",
                partition.partition
            ));
        }
        validate_partition_metrics(partition)?;
    }
    Ok(())
}

fn validate_partition_metrics(partition: &KafkaPartitionMetrics) -> Result<(), String> {
    if partition.partition < 0 {
        return Err("Partition 指标 ID 不能为负数".into());
    }
    if partition.leader.is_some_and(|leader| leader < 0) {
        return Err("Partition 指标 Leader 不能为负数".into());
    }
    validate_optional_count(partition.replica_count, super::MAX_KAFKA_REPLICAS, "副本")?;
    validate_optional_count(partition.isr_count, super::MAX_KAFKA_REPLICAS, "ISR")?;
    validate_optional_offset(partition.low_watermark)?;
    validate_optional_offset(partition.high_watermark)?;
    if let (Some(low), Some(high)) = (partition.low_watermark, partition.high_watermark)
        && low > high
    {
        return Err(format!(
            "Partition 指标首尾 Offset 无效：{}/{}",
            partition.topic, partition.partition
        ));
    }
    validate_rate(partition.message_rate_per_second, "Partition 消息速率")
}

fn validate_group_metrics(group: &KafkaConsumerGroupMetrics) -> Result<(), String> {
    if group.group_id.trim().is_empty() || group.group_id.len() > MAX_KAFKA_ACL_RESOURCE_NAME_BYTES
    {
        return Err("指标消费者组 ID 不能为空或超长".into());
    }
    validate_optional_single_line(
        "指标消费者组状态",
        group.state.as_deref(),
        MAX_KAFKA_VERSION_BYTES,
    )?;
    validate_optional_count(group.member_count, MAX_KAFKA_GROUP_MEMBERS, "消费者成员")?;
    validate_optional_count(
        group.assigned_partition_count,
        MAX_KAFKA_PARTITIONS,
        "分配 Partition",
    )?;
    validate_optional_count(
        group.offset_count,
        MAX_KAFKA_GROUP_OFFSETS,
        "Consumer Offset",
    )?;
    validate_lag(group.total_lag, "消费者组总 Lag")?;
    validate_lag(group.max_lag, "消费者组最大 Lag")
}

fn validate_optional_count(
    value: Option<usize>,
    maximum: usize,
    label: &str,
) -> Result<(), String> {
    if value.is_some_and(|value| value > maximum) {
        return Err(format!("{label}数量超过 {maximum} 个上限"));
    }
    Ok(())
}

fn validate_lag(value: Option<i64>, label: &str) -> Result<(), String> {
    if value.is_some_and(|value| value < 0) {
        return Err(format!("{label}不能为负数"));
    }
    Ok(())
}

fn validate_rate(value: Option<f64>, label: &str) -> Result<(), String> {
    if value.is_some_and(|value| !value.is_finite() || value < 0.0) {
        return Err(format!("{label}必须是有限的非负数"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "kafka_metrics_tests.rs"]
mod tests;
