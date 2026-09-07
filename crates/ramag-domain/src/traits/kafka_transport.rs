use crate::entities::KafkaTransportCapabilities;

/// Kafka 具体客户端的适配边界；上层只读取能力快照，不依赖客户端类型。
pub trait KafkaTransport: Send + Sync {
    fn capabilities(&self) -> KafkaTransportCapabilities;
}
