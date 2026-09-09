use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::kafka_validation::{validate_kafka_topic_name, validate_required_text};
use super::{MAX_KAFKA_ACL_RESOURCE_NAME_BYTES, MAX_KAFKA_GROUP_OFFSETS, MAX_KAFKA_PARTITIONS};

/// 修改一个消费者组已提交位点时提交的单个 Topic/Partition 目标。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaConsumerGroupOffsetReset {
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
}

impl KafkaConsumerGroupOffsetReset {
    pub fn new(topic: impl Into<String>, partition: i32, offset: i64) -> Self {
        Self {
            topic: topic.into(),
            partition,
            offset,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_kafka_topic_name(&self.topic)?;
        if self.partition < 0 {
            return Err("消费者组 Offset 的 Partition 不能为负数".into());
        }
        if self.partition as usize >= MAX_KAFKA_PARTITIONS {
            return Err(format!(
                "消费者组 Offset 的 Partition 不能超过 {}",
                MAX_KAFKA_PARTITIONS - 1
            ));
        }
        if self.offset < 0 {
            return Err("消费者组重置 Offset 不能为负数".into());
        }
        Ok(())
    }
}

/// 消费者组 Offset 重置请求；必须显式列出每个要修改的 Topic/Partition。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaConsumerGroupOffsetResetRequest {
    pub group_id: String,
    pub offsets: Vec<KafkaConsumerGroupOffsetReset>,
}

impl KafkaConsumerGroupOffsetResetRequest {
    pub fn new(group_id: impl Into<String>, offsets: Vec<KafkaConsumerGroupOffsetReset>) -> Self {
        Self {
            group_id: group_id.into(),
            offsets,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_required_text(
            "消费者组 ID",
            &self.group_id,
            MAX_KAFKA_ACL_RESOURCE_NAME_BYTES,
        )?;
        if self.offsets.is_empty() {
            return Err("消费者组 Offset 重置至少需要一个 Topic/Partition".into());
        }
        if self.offsets.len() > MAX_KAFKA_GROUP_OFFSETS {
            return Err(format!(
                "消费者组 Offset 重置数量超过 {MAX_KAFKA_GROUP_OFFSETS} 个上限"
            ));
        }
        let mut targets = HashSet::with_capacity(self.offsets.len());
        for target in &self.offsets {
            target.validate()?;
            if !targets.insert((&target.topic, target.partition)) {
                return Err(format!(
                    "消费者组 Offset 重置目标重复：{}/{}",
                    target.topic, target.partition
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_request_requires_unique_nonnegative_targets() {
        let request = KafkaConsumerGroupOffsetResetRequest::new(
            "workers",
            vec![KafkaConsumerGroupOffsetReset::new("events", 0, 12)],
        );
        assert!(request.validate().is_ok());

        let duplicate = KafkaConsumerGroupOffsetResetRequest::new(
            "workers",
            vec![
                KafkaConsumerGroupOffsetReset::new("events", 0, 12),
                KafkaConsumerGroupOffsetReset::new("events", 0, 13),
            ],
        );
        assert!(duplicate.validate().is_err());
        assert!(
            KafkaConsumerGroupOffsetReset::new("events", 0, -1)
                .validate()
                .is_err()
        );
    }

    #[test]
    fn reset_request_rejects_empty_group_and_targets() {
        assert!(
            KafkaConsumerGroupOffsetResetRequest::new("", Vec::new())
                .validate()
                .is_err()
        );
        assert!(
            KafkaConsumerGroupOffsetResetRequest::new("workers", Vec::new())
                .validate()
                .is_err()
        );
    }
}
