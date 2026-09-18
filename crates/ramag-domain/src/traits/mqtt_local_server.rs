//! 内置 MQTT Broker 的生命周期接口。

use async_trait::async_trait;

use crate::entities::{
    MqttLocalServerConfig, MqttLocalServerStatus, MqttPublishRequest, MqttPublishResult,
};
use crate::error::{DomainError, Result};

#[async_trait]
pub trait MqttLocalServerDriver: Send + Sync {
    async fn start(&self, _config: &MqttLocalServerConfig) -> Result<MqttLocalServerStatus> {
        Err(DomainError::NotImplemented(
            "mqtt_local_server_start".into(),
        ))
    }

    async fn stop(&self) -> Result<MqttLocalServerStatus> {
        Err(DomainError::NotImplemented("mqtt_local_server_stop".into()))
    }

    async fn status(&self) -> Result<MqttLocalServerStatus> {
        Err(DomainError::NotImplemented(
            "mqtt_local_server_status".into(),
        ))
    }

    async fn publish(&self, _request: &MqttPublishRequest) -> Result<MqttPublishResult> {
        Err(DomainError::NotImplemented(
            "mqtt_local_server_publish".into(),
        ))
    }
}
