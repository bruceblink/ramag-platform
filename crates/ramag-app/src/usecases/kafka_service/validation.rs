use super::*;

pub(super) fn validate_config_resource(
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

pub(super) fn validate_message_page(page: KafkaMessagePage) -> Result<KafkaMessagePage> {
    page.validate()
        .map(|()| page)
        .map_err(DomainError::InvalidConfig)
}

pub(super) fn validate_message_produce_result(
    request: &KafkaMessageProduceRequest,
    result: KafkaMessageProduceResult,
) -> Result<KafkaMessageProduceResult> {
    result.validate().map_err(DomainError::InvalidConfig)?;
    if result.topic != request.topic {
        return Err(DomainError::InvalidConfig(
            "Kafka 生产结果 Topic 与请求目标不一致".into(),
        ));
    }
    Ok(result)
}

pub(super) fn validate_metrics_snapshot(
    snapshot: KafkaMetricsSnapshot,
) -> Result<KafkaMetricsSnapshot> {
    snapshot
        .validate()
        .map(|()| snapshot)
        .map_err(DomainError::InvalidConfig)
}

pub(super) fn validate_broker_metrics_snapshot(
    snapshot: KafkaBrokerMetricsSnapshot,
) -> Result<KafkaBrokerMetricsSnapshot> {
    snapshot
        .validate()
        .map(|()| snapshot)
        .map_err(DomainError::InvalidConfig)
}

pub(super) fn validate_cluster_metadata(
    metadata: KafkaClusterMetadata,
) -> Result<KafkaClusterMetadata> {
    metadata
        .validate()
        .map(|()| metadata)
        .map_err(DomainError::InvalidConfig)
}

/// 在应用层校验 Topic 列表及其 Partition 快照，保持替换驱动后的边界不变。
pub(super) fn validate_topics(topics: Vec<KafkaTopic>) -> Result<Vec<KafkaTopic>> {
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

pub(super) fn validate_replica_id_budget(total: &mut usize, count: usize) -> Result<()> {
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

pub(super) fn validate_topic_partition_budget(total: &mut usize, count: usize) -> Result<()> {
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
pub(super) fn validate_consumer_groups(
    groups: Vec<KafkaConsumerGroup>,
) -> Result<Vec<KafkaConsumerGroup>> {
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

pub(super) fn validate_consumer_group_member_budget(total: &mut usize, count: usize) -> Result<()> {
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

pub(super) fn validate_consumer_group_assignment_budget(
    total: &mut usize,
    count: usize,
) -> Result<()> {
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

pub(super) fn validate_consumer_group_offset_budget(total: &mut usize, count: usize) -> Result<()> {
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

pub(super) fn validate_acls(acls: Vec<KafkaAcl>) -> Result<Vec<KafkaAcl>> {
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

pub(super) fn validate_config(config: &KafkaClusterConfig) -> Result<()> {
    config.validate().map_err(DomainError::InvalidConfig)
}

pub(super) fn validate_admin_request(
    config: &KafkaClusterConfig,
    request: std::result::Result<(), String>,
) -> Result<()> {
    validate_config(config)?;
    request.map_err(DomainError::InvalidConfig)?;
    ensure_admin_enabled(config)
}

pub(super) fn ensure_admin_enabled(config: &KafkaClusterConfig) -> Result<()> {
    if config.read_only.allows_admin() {
        Ok(())
    } else {
        Err(DomainError::Forbidden(READ_ONLY_MESSAGE.into()))
    }
}
