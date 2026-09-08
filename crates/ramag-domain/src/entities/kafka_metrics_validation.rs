use std::collections::HashSet;

use super::kafka_metrics::{
    KafkaClusterMetrics, KafkaConsumerGroupMetrics, KafkaPartitionMetrics, KafkaTopicMetrics,
};
use super::kafka_validation::{validate_optional_offset, validate_optional_single_line};

pub(super) fn validate_cluster_metrics(metrics: &KafkaClusterMetrics) -> Result<(), String> {
    validate_optional_count(metrics.broker_count, super::MAX_KAFKA_BROKERS, "Broker")?;
    validate_optional_count(metrics.topic_count, super::MAX_KAFKA_TOPICS, "Topic")?;
    validate_optional_count(
        metrics.partition_count,
        super::MAX_KAFKA_PARTITIONS,
        "Partition",
    )?;
    validate_optional_count(
        metrics.under_replicated_partitions,
        super::MAX_KAFKA_PARTITIONS,
        "异常副本 Partition",
    )?;
    validate_optional_count(
        metrics.offline_partitions,
        super::MAX_KAFKA_PARTITIONS,
        "离线 Partition",
    )?;
    validate_lag(metrics.total_lag, "集群总 Lag")?;
    validate_lag(metrics.max_lag, "集群最大 Lag")?;
    validate_rate(metrics.message_rate_per_second, "集群消息速率")
}

pub(super) fn validate_topic_metrics(topic: &KafkaTopicMetrics) -> Result<(), String> {
    super::validate_kafka_topic_name(&topic.name)?;
    validate_optional_count(
        topic.partition_count,
        super::MAX_KAFKA_PARTITIONS,
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
        super::MAX_KAFKA_PARTITIONS,
        "异常副本 Partition",
    )?;
    validate_optional_count(
        topic.offline_partitions,
        super::MAX_KAFKA_PARTITIONS,
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

pub(super) fn validate_group_metrics(group: &KafkaConsumerGroupMetrics) -> Result<(), String> {
    if group.group_id.trim().is_empty()
        || group.group_id.len() > super::MAX_KAFKA_ACL_RESOURCE_NAME_BYTES
    {
        return Err("指标消费者组 ID 不能为空或超长".into());
    }
    validate_optional_single_line(
        "指标消费者组状态",
        group.state.as_deref(),
        super::MAX_KAFKA_VERSION_BYTES,
    )?;
    validate_optional_count(
        group.member_count,
        super::MAX_KAFKA_GROUP_MEMBERS,
        "消费者成员",
    )?;
    validate_optional_count(
        group.assigned_partition_count,
        super::MAX_KAFKA_PARTITIONS,
        "分配 Partition",
    )?;
    validate_optional_count(
        group.offset_count,
        super::MAX_KAFKA_GROUP_OFFSETS,
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
