//! MQTT Broker 配置 CRUD。整条配置经主密钥加密，避免认证参数和管理路径明文落盘。

use std::sync::Arc;

use parking_lot::RwLock;
use redb::{Database, ReadableDatabase as _, ReadableTable, TableDefinition};
use tracing::{debug, info};

use ramag_domain::entities::{
    MAX_MQTT_PROFILE_LIST_BYTES, MAX_MQTT_PROFILE_RECORD_BYTES, MAX_MQTT_PROFILES, MqttProfile,
};
use ramag_domain::error::{DomainError, Result};

use crate::encryption::Cipher;
use crate::repos::bounded_json;

/// 键为 MqttProfileId UUID，值为加密后的 MqttProfile JSON hex。
pub(crate) const MQTT_PROFILES_TABLE: TableDefinition<&str, &str> =
    TableDefinition::new("mqtt_profiles");

fn encode_profile(profile: &MqttProfile, cipher: &Cipher) -> Result<String> {
    profile.validate().map_err(DomainError::InvalidConfig)?;
    let json = bounded_json::serialize(profile, MAX_MQTT_PROFILE_RECORD_BYTES, "MQTT 配置")?;
    cipher.encrypt(&json)
}

fn decode_profile(key: &str, value: &str, cipher: &Cipher) -> Result<MqttProfile> {
    bounded_json::ensure_len(
        value.len(),
        MAX_MQTT_PROFILE_RECORD_BYTES * 2 + 64,
        &format!("MQTT 配置 {key}"),
    )?;
    let json = cipher
        .decrypt(value)
        .map_err(|error| DomainError::Storage(format!("解密 MQTT 配置 {key} 失败：{error}")))?;
    let profile: MqttProfile = serde_json::from_str(&json)
        .map_err(|error| DomainError::Storage(format!("反序列化 MQTT 配置 {key} 失败：{error}")))?;
    profile
        .validate()
        .map_err(|error| DomainError::Storage(format!("解密后的 MQTT 配置 {key} 无效：{error}")))?;
    if profile.id.to_string() != key {
        return Err(DomainError::Storage(format!(
            "MQTT 配置键与内容 ID 不一致：{key}"
        )));
    }
    Ok(profile)
}

/// 读取并按名称排序所有 MQTT 配置，集合大小受数量和字节预算共同限制。
pub(crate) fn list(db: Arc<Database>, cipher: Arc<RwLock<Cipher>>) -> Result<Vec<MqttProfile>> {
    let read_txn = db
        .begin_read()
        .map_err(|error| DomainError::Storage(format!("启动读事务失败：{error}")))?;
    let table = read_txn
        .open_table(MQTT_PROFILES_TABLE)
        .map_err(|error| DomainError::Storage(format!("打开 mqtt_profiles 表失败：{error}")))?;
    let cipher = cipher.read();
    let mut profiles = Vec::new();
    let mut retained_bytes = 0usize;
    for entry in table
        .iter()
        .map_err(|error| DomainError::Storage(format!("遍历 MQTT 配置失败：{error}")))?
    {
        let (key, value) =
            entry.map_err(|error| DomainError::Storage(format!("读取 MQTT 配置失败：{error}")))?;
        let (_, next_bytes) = bounded_json::next_collection_budget(
            profiles.len(),
            retained_bytes,
            value.value().len(),
            MAX_MQTT_PROFILES,
            MAX_MQTT_PROFILE_LIST_BYTES,
            "MQTT 配置列表",
        )?;
        retained_bytes = next_bytes;
        profiles.push(decode_profile(key.value(), value.value(), &cipher)?);
    }
    profiles.sort_by(|left, right| left.name.cmp(&right.name));
    debug!(
        operation = "mqtt_profile_list",
        count = profiles.len(),
        "mqtt profile listing completed"
    );
    Ok(profiles)
}

pub(crate) fn get(
    db: Arc<Database>,
    cipher: Arc<RwLock<Cipher>>,
    id: String,
) -> Result<Option<MqttProfile>> {
    let read_txn = db
        .begin_read()
        .map_err(|error| DomainError::Storage(format!("启动读事务失败：{error}")))?;
    let table = read_txn
        .open_table(MQTT_PROFILES_TABLE)
        .map_err(|error| DomainError::Storage(format!("打开 mqtt_profiles 表失败：{error}")))?;
    let value = table
        .get(id.as_str())
        .map_err(|error| DomainError::Storage(format!("读取 MQTT 配置 {id} 失败：{error}")))?;
    value
        .map(|value| decode_profile(&id, value.value(), &cipher.read()))
        .transpose()
}

pub(crate) fn save(
    db: Arc<Database>,
    cipher: Arc<RwLock<Cipher>>,
    profile: MqttProfile,
) -> Result<()> {
    let value = encode_profile(&profile, &cipher.read())?;
    let id = profile.id.to_string();
    let write_txn = db
        .begin_write()
        .map_err(|error| DomainError::Storage(format!("启动写事务失败：{error}")))?;
    {
        let mut table = write_txn
            .open_table(MQTT_PROFILES_TABLE)
            .map_err(|error| DomainError::Storage(format!("打开 mqtt_profiles 表失败：{error}")))?;
        let mut count = 0usize;
        let mut total_bytes = 0usize;
        let mut replaced_bytes = None;
        for entry in table
            .iter()
            .map_err(|error| DomainError::Storage(format!("遍历 MQTT 配置失败：{error}")))?
        {
            let (key, existing) = entry
                .map_err(|error| DomainError::Storage(format!("读取 MQTT 配置失败：{error}")))?;
            (count, total_bytes) = bounded_json::next_collection_budget(
                count,
                total_bytes,
                existing.value().len(),
                MAX_MQTT_PROFILES,
                MAX_MQTT_PROFILE_LIST_BYTES,
                "MQTT 配置列表",
            )?;
            if key.value() == id {
                replaced_bytes = Some(existing.value().len());
            }
        }
        let final_count = count + usize::from(replaced_bytes.is_none());
        let final_bytes = total_bytes
            .checked_sub(replaced_bytes.unwrap_or(0))
            .and_then(|bytes| bytes.checked_add(value.len()))
            .ok_or_else(|| DomainError::Storage("MQTT 配置列表总大小溢出".into()))?;
        bounded_json::ensure_collection_budget(
            final_count,
            final_bytes,
            MAX_MQTT_PROFILES,
            MAX_MQTT_PROFILE_LIST_BYTES,
            "MQTT 配置列表",
        )?;
        table
            .insert(id.as_str(), value.as_str())
            .map_err(|error| DomainError::Storage(format!("写入 MQTT 配置 {id} 失败：{error}")))?;
    }
    write_txn
        .commit()
        .map_err(|error| DomainError::Storage(format!("提交事务失败：{error}")))?;
    info!(operation = "mqtt_profile_save", profile_id = %id, "mqtt profile saved");
    Ok(())
}

pub(crate) fn delete(db: Arc<Database>, id: String) -> Result<()> {
    let write_txn = db
        .begin_write()
        .map_err(|error| DomainError::Storage(format!("启动写事务失败：{error}")))?;
    {
        let mut table = write_txn
            .open_table(MQTT_PROFILES_TABLE)
            .map_err(|error| DomainError::Storage(format!("打开 mqtt_profiles 表失败：{error}")))?;
        table
            .remove(id.as_str())
            .map_err(|error| DomainError::Storage(format!("删除 MQTT 配置 {id} 失败：{error}")))?;
    }
    write_txn
        .commit()
        .map_err(|error| DomainError::Storage(format!("提交事务失败：{error}")))?;
    info!(operation = "mqtt_profile_delete", profile_id = %id, "mqtt profile deleted");
    Ok(())
}

pub(crate) fn ensure_table(write_txn: &redb::WriteTransaction) -> Result<()> {
    let _ = write_txn
        .open_table(MQTT_PROFILES_TABLE)
        .map_err(|error| DomainError::Storage(format!("打开 mqtt_profiles 表失败：{error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::{MosquittoConfigTarget, MosquittoStaticConfig};

    #[test]
    fn encrypted_record_does_not_contain_mqtt_secrets() {
        let cipher = Cipher::new(&[9; 32]);
        let mut profile = MqttProfile::new("production", "broker.example.com", 8883);
        profile.transport = ramag_domain::entities::MqttTransport::Tls;
        profile.username = Some("operator".into());
        profile.password = Some("secret-password".into());
        profile.management.enabled = true;
        profile.management.admin_username = Some("admin".into());
        profile.management.admin_password = Some("admin-password".into());
        profile.management.static_config = Some(MosquittoStaticConfig {
            target: MosquittoConfigTarget::Ssh {
                profile_id: ramag_domain::entities::SshProfileId::new(),
            },
            password_file: Some("/etc/mosquitto/passwd".into()),
            acl_file: Some("/etc/mosquitto/acl".into()),
        });

        let encoded = encode_profile(&profile, &cipher).unwrap();
        assert!(!encoded.contains("production"));
        assert!(!encoded.contains("broker.example.com"));
        assert!(!encoded.contains("secret-password"));
        assert!(!encoded.contains("admin-password"));
        assert_eq!(
            decode_profile(&profile.id.to_string(), &encoded, &cipher).unwrap(),
            profile
        );
    }

    #[test]
    fn decode_rejects_key_content_id_mismatch() {
        let cipher = Cipher::new(&[9; 32]);
        let profile = MqttProfile::new("local", "localhost", 1883);
        let encoded = encode_profile(&profile, &cipher).unwrap();

        assert!(decode_profile("other-id", &encoded, &cipher).is_err());
    }
}
