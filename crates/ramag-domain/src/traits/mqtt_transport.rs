use crate::entities::MqttTransportCapabilities;

/// MQTT 具体客户端的适配边界；应用层只依赖能力快照。
pub trait MqttTransport: Send + Sync {
    fn capabilities(&self) -> MqttTransportCapabilities;
}
