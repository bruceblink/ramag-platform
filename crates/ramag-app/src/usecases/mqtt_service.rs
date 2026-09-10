//! MQTT 配置、消息数据面和 Mosquitto 管理用例。

use std::sync::{Arc, atomic::AtomicBool};

use ramag_domain::entities::{
    MAX_MOSQUITTO_NAME_BYTES, MosquittoClient, MosquittoDynamicSecuritySnapshot, MosquittoGroup,
    MosquittoRole, MosquittoStaticFile, MosquittoStaticFileKind, MqttBrokerSnapshot,
    MqttMessageSink, MqttProfile, MqttProfileId, MqttPublishRequest, MqttPublishResult,
    MqttSubscribeRequest,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::{
    MosquittoDynamicSecurityDriver, MosquittoStaticConfigDriver, MqttDriver, Storage,
};
use tracing::{info, warn};

pub struct MqttService {
    driver: Arc<dyn MqttDriver>,
    dynamic_security_driver: Arc<dyn MosquittoDynamicSecurityDriver>,
    static_config_driver: Arc<dyn MosquittoStaticConfigDriver>,
    storage: Arc<dyn Storage>,
}

impl MqttService {
    pub fn new(driver: Arc<dyn MqttDriver>, storage: Arc<dyn Storage>) -> Self {
        Self {
            driver,
            dynamic_security_driver: Arc::new(UnsupportedMosquittoDynamicSecurityDriver),
            static_config_driver: Arc::new(UnsupportedMosquittoStaticConfigDriver),
            storage,
        }
    }

    pub fn with_dynamic_security_driver(
        mut self,
        driver: Arc<dyn MosquittoDynamicSecurityDriver>,
    ) -> Self {
        self.dynamic_security_driver = driver;
        self
    }

    pub fn with_static_config_driver(
        mut self,
        driver: Arc<dyn MosquittoStaticConfigDriver>,
    ) -> Self {
        self.static_config_driver = driver;
        self
    }

    pub fn transport_capabilities(&self) -> ramag_domain::entities::MqttTransportCapabilities {
        self.driver.transport_capabilities()
    }

    pub async fn list_profiles(&self) -> Result<Vec<MqttProfile>> {
        let result = self.storage.list_mqtt_profiles().await;
        log_storage_result("mqtt_profile_list", &result);
        result
    }

    pub async fn get_profile(&self, id: &MqttProfileId) -> Result<Option<MqttProfile>> {
        let result = self.storage.get_mqtt_profile(id).await;
        log_storage_result("mqtt_profile_get", &result);
        result
    }

    pub async fn save_profile(&self, profile: &MqttProfile) -> Result<()> {
        profile.validate().map_err(DomainError::InvalidConfig)?;
        let result = self.storage.save_mqtt_profile(profile).await;
        log_storage_result("mqtt_profile_save", &result);
        result
    }

    pub async fn delete_profile(&self, id: &MqttProfileId) -> Result<()> {
        let result = self.storage.delete_mqtt_profile(id).await;
        log_storage_result("mqtt_profile_delete", &result);
        result
    }

    pub async fn test_connection(&self, profile: &MqttProfile) -> Result<()> {
        validate_profile(profile)?;
        let started = std::time::Instant::now();
        let result = self.driver.test_connection(profile).await;
        log_runtime_result("mqtt_test_connection", profile, started, result.is_ok());
        result
    }

    pub async fn publish(
        &self,
        profile: &MqttProfile,
        request: &MqttPublishRequest,
    ) -> Result<MqttPublishResult> {
        validate_profile(profile)?;
        request.validate().map_err(DomainError::InvalidConfig)?;
        let started = std::time::Instant::now();
        let result = self
            .driver
            .publish(profile, request)
            .await
            .and_then(|result| validate_publish_result(request, result));
        tracing::info!(
            operation = "mqtt_publish",
            profile_id = %profile.id,
            topic = %request.topic,
            payload_bytes = request.payload.len(),
            elapsed_ms = started.elapsed().as_millis(),
            success = result.is_ok(),
            "MQTT publish completed"
        );
        if let Err(error) = &result {
            warn!(operation = "mqtt_publish", profile_id = %profile.id, topic = %request.topic, error = %error, "MQTT publish failed");
        }
        result
    }

    pub async fn subscribe(
        &self,
        profile: &MqttProfile,
        request: &MqttSubscribeRequest,
        sink: MqttMessageSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        validate_profile(profile)?;
        request.validate().map_err(DomainError::InvalidConfig)?;
        let started = std::time::Instant::now();
        let result = self
            .driver
            .subscribe(profile, request, sink, cancelled)
            .await;
        log_runtime_result("mqtt_subscribe", profile, started, result.is_ok());
        result
    }

    pub async fn broker_snapshot(&self, profile: &MqttProfile) -> Result<MqttBrokerSnapshot> {
        validate_profile(profile)?;
        let started = std::time::Instant::now();
        let result = self
            .driver
            .broker_snapshot(profile)
            .await
            .and_then(|snapshot| snapshot.validate().map_err(DomainError::InvalidConfig));
        tracing::info!(
            operation = "mqtt_broker_snapshot",
            profile_id = %profile.id,
            topic_count = result.as_ref().map_or(0, |snapshot| snapshot.topics.len()),
            online_client_count = result.as_ref().map_or(0, |snapshot| snapshot.online_clients.len()),
            topics_complete = result.as_ref().is_ok_and(|snapshot| snapshot.topics_complete),
            online_clients_complete = result.as_ref().is_ok_and(|snapshot| snapshot.online_clients_complete),
            elapsed_ms = started.elapsed().as_millis(),
            success = result.is_ok(),
            "MQTT broker snapshot completed"
        );
        result
    }

    pub async fn dynamic_security_snapshot(
        &self,
        profile: &MqttProfile,
    ) -> Result<MosquittoDynamicSecuritySnapshot> {
        ensure_management_enabled(profile)?;
        let result = self
            .dynamic_security_driver
            .snapshot(profile)
            .await
            .and_then(|snapshot| snapshot.validate().map_err(DomainError::InvalidConfig));
        log_management_result("mosquitto_dynamic_security_snapshot", profile, &result);
        result
    }

    pub async fn save_client(&self, profile: &MqttProfile, client: &MosquittoClient) -> Result<()> {
        ensure_management_enabled(profile)?;
        client.validate().map_err(DomainError::InvalidConfig)?;
        let result = self
            .dynamic_security_driver
            .save_client(profile, client)
            .await;
        log_management_result("mosquitto_dynamic_security_save_client", profile, &result);
        result
    }

    pub async fn delete_client(&self, profile: &MqttProfile, username: &str) -> Result<()> {
        ensure_management_enabled(profile)?;
        validate_management_name("客户端用户名", username)?;
        let result = self
            .dynamic_security_driver
            .delete_client(profile, username)
            .await;
        log_management_result("mosquitto_dynamic_security_delete_client", profile, &result);
        result
    }

    pub async fn save_group(&self, profile: &MqttProfile, group: &MosquittoGroup) -> Result<()> {
        ensure_management_enabled(profile)?;
        group.validate().map_err(DomainError::InvalidConfig)?;
        let result = self
            .dynamic_security_driver
            .save_group(profile, group)
            .await;
        log_management_result("mosquitto_dynamic_security_save_group", profile, &result);
        result
    }

    pub async fn delete_group(&self, profile: &MqttProfile, group_name: &str) -> Result<()> {
        ensure_management_enabled(profile)?;
        validate_management_name("Group 名称", group_name)?;
        let result = self
            .dynamic_security_driver
            .delete_group(profile, group_name)
            .await;
        log_management_result("mosquitto_dynamic_security_delete_group", profile, &result);
        result
    }

    pub async fn save_role(&self, profile: &MqttProfile, role: &MosquittoRole) -> Result<()> {
        ensure_management_enabled(profile)?;
        role.validate().map_err(DomainError::InvalidConfig)?;
        let result = self.dynamic_security_driver.save_role(profile, role).await;
        log_management_result("mosquitto_dynamic_security_save_role", profile, &result);
        result
    }

    pub async fn delete_role(&self, profile: &MqttProfile, role_name: &str) -> Result<()> {
        ensure_management_enabled(profile)?;
        validate_management_name("Role 名称", role_name)?;
        let result = self
            .dynamic_security_driver
            .delete_role(profile, role_name)
            .await;
        log_management_result("mosquitto_dynamic_security_delete_role", profile, &result);
        result
    }

    pub async fn read_static_file(
        &self,
        profile: &MqttProfile,
        kind: MosquittoStaticFileKind,
    ) -> Result<MosquittoStaticFile> {
        ensure_static_file_configured(profile, kind)?;
        let result = self
            .static_config_driver
            .read_file(profile, kind)
            .await
            .and_then(|file| validate_static_file(profile, file));
        log_management_result("mosquitto_static_config_read", profile, &result);
        result
    }

    pub async fn write_static_file(
        &self,
        profile: &MqttProfile,
        file: &MosquittoStaticFile,
    ) -> Result<()> {
        ensure_static_file_configured(profile, file.kind)?;
        file.validate().map_err(DomainError::InvalidConfig)?;
        let expected_path = file.kind.configured_path(profile).ok_or_else(|| {
            DomainError::InvalidConfig(format!("未配置 Mosquitto {} 路径", file.kind.label()))
        })?;
        if file.path != expected_path {
            return Err(DomainError::InvalidConfig(
                "Mosquitto 静态配置路径与已保存配置不一致".into(),
            ));
        }
        let result = self.static_config_driver.write_file(profile, file).await;
        log_management_result("mosquitto_static_config_write", profile, &result);
        result
    }
}

fn validate_profile(profile: &MqttProfile) -> Result<()> {
    profile.validate().map_err(DomainError::InvalidConfig)
}

fn ensure_management_enabled(profile: &MqttProfile) -> Result<()> {
    validate_profile(profile)?;
    if profile.management.enabled {
        Ok(())
    } else {
        Err(DomainError::InvalidConfig(
            "未启用 Mosquitto 管理能力".into(),
        ))
    }
}

fn ensure_static_file_configured(
    profile: &MqttProfile,
    kind: MosquittoStaticFileKind,
) -> Result<()> {
    ensure_management_enabled(profile)?;
    if kind.configured_path(profile).is_some() {
        Ok(())
    } else {
        Err(DomainError::InvalidConfig(format!(
            "未配置 Mosquitto {} 路径",
            kind.label()
        )))
    }
}

fn validate_static_file(
    profile: &MqttProfile,
    file: MosquittoStaticFile,
) -> Result<MosquittoStaticFile> {
    file.validate().map_err(DomainError::InvalidConfig)?;
    let expected_path = file.kind.configured_path(profile).ok_or_else(|| {
        DomainError::InvalidConfig(format!("未配置 Mosquitto {} 路径", file.kind.label()))
    })?;
    if file.path != expected_path {
        return Err(DomainError::InvalidConfig(
            "Mosquitto 静态配置返回路径与已保存配置不一致".into(),
        ));
    }
    Ok(file)
}

fn validate_management_name(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(DomainError::InvalidConfig(format!("{label}不能为空")));
    }
    if value.len() > MAX_MOSQUITTO_NAME_BYTES {
        return Err(DomainError::InvalidConfig(format!(
            "{label}过长：最多 {MAX_MOSQUITTO_NAME_BYTES} bytes"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(DomainError::InvalidConfig(format!(
            "{label}不能包含控制字符"
        )));
    }
    Ok(())
}

fn validate_publish_result(
    request: &MqttPublishRequest,
    result: MqttPublishResult,
) -> Result<MqttPublishResult> {
    result.validate().map_err(DomainError::InvalidConfig)?;
    if result.topic != request.topic || result.qos != request.qos {
        return Err(DomainError::InvalidConfig(
            "MQTT 发布结果与请求不一致".into(),
        ));
    }
    Ok(result)
}

fn log_storage_result<T>(operation: &'static str, result: &Result<T>) {
    if let Err(error) = result {
        warn!(operation, error = %error, "MQTT local storage operation failed");
    }
}

fn log_runtime_result(
    operation: &'static str,
    profile: &MqttProfile,
    started: std::time::Instant,
    success: bool,
) {
    info!(
        operation,
        profile_id = %profile.id,
        elapsed_ms = started.elapsed().as_millis(),
        success,
        "MQTT operation completed"
    );
}

fn log_management_result<T>(operation: &'static str, profile: &MqttProfile, result: &Result<T>) {
    info!(operation, profile_id = %profile.id, success = result.is_ok(), "Mosquitto management operation completed");
    if let Err(error) = result {
        warn!(operation, profile_id = %profile.id, error = %error, "Mosquitto management operation failed");
    }
}

struct UnsupportedMosquittoDynamicSecurityDriver;

impl MosquittoDynamicSecurityDriver for UnsupportedMosquittoDynamicSecurityDriver {}

struct UnsupportedMosquittoStaticConfigDriver;

impl MosquittoStaticConfigDriver for UnsupportedMosquittoStaticConfigDriver {}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::{MosquittoManagementConfig, MosquittoStaticConfig};

    fn management_profile() -> MqttProfile {
        let mut profile = MqttProfile::new("broker", "localhost", 1883);
        profile.management = MosquittoManagementConfig {
            enabled: true,
            admin_username: Some("admin".into()),
            admin_password: Some("password".into()),
            static_config: Some(MosquittoStaticConfig {
                target: Default::default(),
                password_file: Some("/tmp/passwd".into()),
                acl_file: None,
            }),
        };
        profile
    }

    #[test]
    fn static_file_validation_requires_the_saved_path() {
        let profile = management_profile();
        let file = MosquittoStaticFile {
            kind: MosquittoStaticFileKind::Password,
            path: "/tmp/other".into(),
            content: "hash".into(),
        };
        assert!(validate_static_file(&profile, file).is_err());
    }

    #[test]
    fn management_operations_require_an_enabled_profile() {
        let profile = MqttProfile::new("broker", "localhost", 1883);
        assert!(ensure_management_enabled(&profile).is_err());
    }
}
