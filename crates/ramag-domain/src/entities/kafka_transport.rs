use serde::{Deserialize, Serialize};

use super::kafka_validation::validate_protocol_text;
use super::{
    KafkaMessageRecord, MAX_KAFKA_QUERY_PARTITIONS, MAX_KAFKA_SCAN_BYTES, MAX_KAFKA_SCAN_RECORDS,
    MAX_KAFKA_TAIL_MESSAGE_BYTES, MAX_KAFKA_TAIL_POLL_TIMEOUT_MILLIS, MAX_KAFKA_TAIL_WINDOW_BYTES,
    MAX_KAFKA_TAIL_WINDOW_MESSAGES,
};

/// Kafka 传输实现的来源；产品层不依赖具体客户端类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KafkaTransportBackend {
    NativeRdkafka,
    PureRust,
    TestDouble,
}

/// 传输层在当前构建和配置下能提供的协议能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaTransportCapabilities {
    pub backend: KafkaTransportBackend,
    pub build_available: bool,
    pub metadata: bool,
    pub fetch: bool,
    pub list_offsets: bool,
    pub consumer_groups: bool,
    pub topic_admin: bool,
    pub config_admin: bool,
    pub acl_admin: bool,
    pub tls: bool,
    pub sasl: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KafkaTransportCapability {
    Metadata,
    Fetch,
    ListOffsets,
    ConsumerGroups,
    TopicAdmin,
    ConfigAdmin,
    AclAdmin,
    Tls,
    Sasl,
}

impl KafkaTransportCapabilities {
    pub const fn native(build_available: bool, tls: bool, sasl: bool) -> Self {
        Self {
            backend: KafkaTransportBackend::NativeRdkafka,
            build_available,
            metadata: build_available,
            fetch: build_available,
            list_offsets: build_available,
            consumer_groups: build_available,
            topic_admin: build_available,
            config_admin: build_available,
            acl_admin: build_available,
            tls: build_available && tls,
            sasl: build_available && sasl,
        }
    }

    pub const fn unknown() -> Self {
        Self {
            backend: KafkaTransportBackend::TestDouble,
            build_available: false,
            metadata: false,
            fetch: false,
            list_offsets: false,
            consumer_groups: false,
            topic_admin: false,
            config_admin: false,
            acl_admin: false,
            tls: false,
            sasl: false,
        }
    }

    pub const fn supports(self, capability: KafkaTransportCapability) -> bool {
        match capability {
            KafkaTransportCapability::Metadata => self.metadata,
            KafkaTransportCapability::Fetch => self.fetch,
            KafkaTransportCapability::ListOffsets => self.list_offsets,
            KafkaTransportCapability::ConsumerGroups => self.consumer_groups,
            KafkaTransportCapability::TopicAdmin => self.topic_admin,
            KafkaTransportCapability::ConfigAdmin => self.config_admin,
            KafkaTransportCapability::AclAdmin => self.acl_admin,
            KafkaTransportCapability::Tls => self.tls,
            KafkaTransportCapability::Sasl => self.sasl,
        }
    }
}

/// 实时消息流的起始位置；`Latest` 只读取启动后到达的消息。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KafkaMessageTailStart {
    Latest,
    Earliest,
    Offset(i64),
}

impl KafkaMessageTailStart {
    fn validate(self) -> Result<(), String> {
        if let Self::Offset(offset) = self
            && offset < 0
        {
            return Err("实时消息流起始 Offset 不能为负数".into());
        }
        Ok(())
    }
}

/// 实时消息流的读取范围和内存预算；窗口上限不会改变 Kafka 的消费提交状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaMessageTailRequest {
    pub topic: String,
    pub partitions: Vec<i32>,
    pub start: KafkaMessageTailStart,
    pub max_window_messages: usize,
    pub max_window_bytes: u64,
    pub max_message_bytes: usize,
    pub poll_timeout_millis: u32,
}

impl KafkaMessageTailRequest {
    pub fn latest(topic: impl Into<String>, partitions: Vec<i32>) -> Self {
        Self::new(topic, partitions, KafkaMessageTailStart::Latest)
    }

    pub fn new(
        topic: impl Into<String>,
        partitions: Vec<i32>,
        start: KafkaMessageTailStart,
    ) -> Self {
        Self {
            topic: topic.into(),
            partitions,
            start,
            max_window_messages: super::DEFAULT_KAFKA_TAIL_WINDOW_MESSAGES,
            max_window_bytes: super::DEFAULT_KAFKA_TAIL_WINDOW_BYTES,
            max_message_bytes: super::DEFAULT_KAFKA_TAIL_MAX_MESSAGE_BYTES,
            poll_timeout_millis: super::DEFAULT_KAFKA_TAIL_POLL_TIMEOUT_MILLIS,
        }
    }

    pub fn with_limits(
        mut self,
        max_window_messages: usize,
        max_window_bytes: u64,
        max_message_bytes: usize,
        poll_timeout_millis: u32,
    ) -> Self {
        self.max_window_messages = max_window_messages;
        self.max_window_bytes = max_window_bytes;
        self.max_message_bytes = max_message_bytes;
        self.poll_timeout_millis = poll_timeout_millis;
        self
    }

    /// 校验实时流范围、单条消息大小和窗口预算，防止后台任务无界运行或持有正文。
    pub fn validate(&self) -> Result<(), String> {
        validate_protocol_text(
            "实时消息流 Topic",
            &self.topic,
            super::MAX_KAFKA_TOPIC_NAME_BYTES,
        )?;
        if self.topic.trim().is_empty() {
            return Err("实时消息流 Topic 不能为空".into());
        }
        if self.partitions.is_empty() {
            return Err("实时消息流至少需要一个 Partition".into());
        }
        if self.partitions.len() > MAX_KAFKA_QUERY_PARTITIONS {
            return Err(format!(
                "实时消息流 Partition 数量超过 {MAX_KAFKA_QUERY_PARTITIONS} 个"
            ));
        }
        let mut seen = std::collections::HashSet::with_capacity(self.partitions.len());
        for partition in &self.partitions {
            if *partition < 0 {
                return Err("实时消息流 Partition 不能为负数".into());
            }
            if !seen.insert(*partition) {
                return Err(format!("实时消息流 Partition 不能重复：{partition}"));
            }
        }
        self.start.validate()?;
        if !(1..=MAX_KAFKA_TAIL_WINDOW_MESSAGES).contains(&self.max_window_messages) {
            return Err(format!(
                "实时消息窗口条数必须在 1 - {MAX_KAFKA_TAIL_WINDOW_MESSAGES} 之间"
            ));
        }
        if !(1..=MAX_KAFKA_TAIL_WINDOW_BYTES).contains(&self.max_window_bytes) {
            return Err(format!(
                "实时消息窗口字节数必须在 1 - {MAX_KAFKA_TAIL_WINDOW_BYTES} 之间"
            ));
        }
        if !(1..=MAX_KAFKA_TAIL_MESSAGE_BYTES).contains(&self.max_message_bytes) {
            return Err(format!(
                "实时消息单条大小必须在 1 - {MAX_KAFKA_TAIL_MESSAGE_BYTES} bytes 之间"
            ));
        }
        if self.max_message_bytes as u64 > self.max_window_bytes {
            return Err("实时消息单条大小不能超过窗口字节上限".into());
        }
        if !(1..=MAX_KAFKA_TAIL_POLL_TIMEOUT_MILLIS).contains(&self.poll_timeout_millis) {
            return Err(format!(
                "实时消息轮询等待必须在 1 - {MAX_KAFKA_TAIL_POLL_TIMEOUT_MILLIS} 毫秒之间"
            ));
        }
        if self.max_window_messages > MAX_KAFKA_SCAN_RECORDS
            || self.max_window_bytes > MAX_KAFKA_SCAN_BYTES
        {
            return Err("实时消息窗口超过通用消息扫描上限".into());
        }
        Ok(())
    }
}

/// 实时流向 UI 发送的有界事件；消息正文只在内存窗口中短暂保留。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KafkaMessageTailEvent {
    Message(KafkaMessageRecord),
    Dropped { records: u64, bytes: u64 },
    Reconnecting { attempt: u32 },
    Connected { attempt: u32 },
}

impl KafkaMessageTailEvent {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Message(record) => record.validate(),
            Self::Dropped { records, .. } if *records == 0 => {
                Err("实时消息丢弃统计必须包含记录数".into())
            }
            Self::Dropped { .. } => Ok(()),
            Self::Reconnecting { attempt } if *attempt == 0 => {
                Err("实时消息重连次数必须从 1 开始".into())
            }
            Self::Reconnecting { .. } | Self::Connected { .. } => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_capabilities_follow_build_and_security_features() {
        let unavailable = KafkaTransportCapabilities::native(false, true, true);
        assert_eq!(unavailable.backend, KafkaTransportBackend::NativeRdkafka);
        assert!(!unavailable.build_available);
        assert!(!unavailable.supports(KafkaTransportCapability::Metadata));
        assert!(!unavailable.supports(KafkaTransportCapability::Tls));

        let available = KafkaTransportCapabilities::native(true, true, false);
        assert!(available.supports(KafkaTransportCapability::Metadata));
        assert!(available.supports(KafkaTransportCapability::ConfigAdmin));
        assert!(available.supports(KafkaTransportCapability::Tls));
        assert!(!available.supports(KafkaTransportCapability::Sasl));
    }

    #[test]
    fn unknown_capabilities_are_explicitly_unavailable() {
        let capabilities = KafkaTransportCapabilities::unknown();
        assert_eq!(capabilities.backend, KafkaTransportBackend::TestDouble);
        assert!(!capabilities.build_available);
        assert!(!capabilities.supports(KafkaTransportCapability::Fetch));
    }

    #[test]
    fn tail_request_bounds_range_and_window() {
        let request = KafkaMessageTailRequest::latest("events", vec![0, 1]);
        assert!(request.validate().is_ok());

        let invalid = request.clone().with_limits(0, 1, 1, 250);
        assert!(invalid.validate().is_err());

        let too_large = request.with_limits(10, 1024, super::MAX_KAFKA_TAIL_MESSAGE_BYTES, 250);
        assert!(too_large.validate().is_err());

        let negative =
            KafkaMessageTailRequest::new("events", vec![0], KafkaMessageTailStart::Offset(-1));
        assert!(negative.validate().is_err());
    }

    #[test]
    fn tail_event_validation_rejects_empty_drop_and_zero_reconnect() {
        assert!(
            KafkaMessageTailEvent::Dropped {
                records: 0,
                bytes: 0
            }
            .validate()
            .is_err()
        );
        assert!(
            KafkaMessageTailEvent::Reconnecting { attempt: 0 }
                .validate()
                .is_err()
        );
        assert!(
            KafkaMessageTailEvent::Connected { attempt: 0 }
                .validate()
                .is_ok()
        );
    }
}
