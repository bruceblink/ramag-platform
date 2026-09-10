//! MQTT 数据面和 Mosquitto 管理面的领域接口。

use std::sync::{Arc, atomic::AtomicBool};

use async_trait::async_trait;

use crate::entities::{
    MosquittoClient, MosquittoDynamicSecuritySnapshot, MosquittoGroup, MosquittoRole,
    MosquittoStaticFile, MosquittoStaticFileKind, MqttBrokerSnapshot, MqttMessageSink, MqttProfile,
    MqttPublishRequest, MqttPublishResult, MqttSubscribeRequest,
};
use crate::error::{DomainError, Result};

/// MQTT 客户端适配器；具体库和线程模型留在基础设施层。
#[async_trait]
pub trait MqttDriver: Send + Sync {
    fn name(&self) -> &'static str {
        "mqtt"
    }

    fn transport_capabilities(&self) -> crate::entities::MqttTransportCapabilities {
        crate::entities::MqttTransportCapabilities::unknown()
    }

    async fn test_connection(&self, _profile: &MqttProfile) -> Result<()> {
        Err(DomainError::NotImplemented("mqtt_test_connection".into()))
    }

    async fn publish(
        &self,
        _profile: &MqttProfile,
        _request: &MqttPublishRequest,
    ) -> Result<MqttPublishResult> {
        Err(DomainError::NotImplemented("mqtt_publish".into()))
    }

    /// 持续接收明确订阅的消息；调用方通过取消标志结束本次订阅。
    async fn subscribe(
        &self,
        _profile: &MqttProfile,
        _request: &MqttSubscribeRequest,
        _sink: MqttMessageSink,
        _cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        Err(DomainError::NotImplemented("mqtt_subscribe".into()))
    }

    /// 主题和在线客户端不是 MQTT 标准的完整目录；驱动必须明确返回数据是否完整。
    async fn broker_snapshot(&self, _profile: &MqttProfile) -> Result<MqttBrokerSnapshot> {
        Err(DomainError::NotImplemented("mqtt_broker_snapshot".into()))
    }
}

/// Mosquitto Dynamic Security 管理接口；不把管理命令拼接到通用 MQTT 数据面。
#[async_trait]
pub trait MosquittoDynamicSecurityDriver: Send + Sync {
    async fn snapshot(&self, _profile: &MqttProfile) -> Result<MosquittoDynamicSecuritySnapshot> {
        Err(DomainError::NotImplemented(
            "mosquitto_dynamic_security_snapshot".into(),
        ))
    }

    async fn save_client(&self, _profile: &MqttProfile, _client: &MosquittoClient) -> Result<()> {
        Err(DomainError::NotImplemented(
            "mosquitto_dynamic_security_save_client".into(),
        ))
    }

    async fn delete_client(&self, _profile: &MqttProfile, _username: &str) -> Result<()> {
        Err(DomainError::NotImplemented(
            "mosquitto_dynamic_security_delete_client".into(),
        ))
    }

    async fn save_group(&self, _profile: &MqttProfile, _group: &MosquittoGroup) -> Result<()> {
        Err(DomainError::NotImplemented(
            "mosquitto_dynamic_security_save_group".into(),
        ))
    }

    async fn delete_group(&self, _profile: &MqttProfile, _group_name: &str) -> Result<()> {
        Err(DomainError::NotImplemented(
            "mosquitto_dynamic_security_delete_group".into(),
        ))
    }

    async fn save_role(&self, _profile: &MqttProfile, _role: &MosquittoRole) -> Result<()> {
        Err(DomainError::NotImplemented(
            "mosquitto_dynamic_security_save_role".into(),
        ))
    }

    async fn delete_role(&self, _profile: &MqttProfile, _role_name: &str) -> Result<()> {
        Err(DomainError::NotImplemented(
            "mosquitto_dynamic_security_delete_role".into(),
        ))
    }
}

/// Mosquitto 静态 password_file/acl_file 读写接口；本地和 SSH 目标由适配器实现。
#[async_trait]
pub trait MosquittoStaticConfigDriver: Send + Sync {
    async fn read_file(
        &self,
        _profile: &MqttProfile,
        _kind: MosquittoStaticFileKind,
    ) -> Result<MosquittoStaticFile> {
        Err(DomainError::NotImplemented(
            "mosquitto_static_config_read".into(),
        ))
    }

    async fn write_file(&self, _profile: &MqttProfile, _file: &MosquittoStaticFile) -> Result<()> {
        Err(DomainError::NotImplemented(
            "mosquitto_static_config_write".into(),
        ))
    }
}
