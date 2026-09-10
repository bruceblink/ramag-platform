use ramag_domain::entities::{
    MAX_MQTT_PROFILE_NAME_BYTES, MosquittoConfigTarget, MosquittoStaticConfig, MqttProfile,
    MqttTransport,
};
use ramag_domain::traits::Storage;
use redb::ReadableDatabase;

use crate::RedbStorage;
use crate::repos;

use super::make_test_storage;

fn sample_profile(name: &str, host: &str) -> MqttProfile {
    MqttProfile::new(name, host, 1883)
}

#[tokio::test]
async fn mqtt_profiles_are_encrypted_sorted_and_round_trip() {
    let (storage, _tmp) = make_test_storage();
    let mut secure = sample_profile("production", "secret.example.com");
    secure.transport = MqttTransport::Tls;
    secure.port = 8883;
    secure.username = Some("operator".into());
    secure.password = Some("secret-password".into());
    secure.management.enabled = true;
    secure.management.admin_username = Some("admin".into());
    secure.management.admin_password = Some("admin-password".into());
    secure.management.static_config = Some(MosquittoStaticConfig {
        target: MosquittoConfigTarget::Ssh {
            profile_id: ramag_domain::entities::SshProfileId::new(),
        },
        password_file: Some("/etc/mosquitto/passwd".into()),
        acl_file: Some("/etc/mosquitto/acl".into()),
    });
    let local = sample_profile("local", "127.0.0.1");

    storage.save_mqtt_profile(&secure).await.unwrap();
    storage.save_mqtt_profile(&local).await.unwrap();

    let listed = storage.list_mqtt_profiles().await.unwrap();
    assert_eq!(listed, vec![local.clone(), secure.clone()]);
    assert_eq!(
        storage.get_mqtt_profile(&secure.id).await.unwrap(),
        Some(secure.clone())
    );

    let read_txn = storage.db.begin_read().unwrap();
    let table = read_txn
        .open_table(repos::mqtt_profile_repo::MQTT_PROFILES_TABLE)
        .unwrap();
    let raw = table.get(secure.id.to_string().as_str()).unwrap().unwrap();
    assert!(!raw.value().contains("production"));
    assert!(!raw.value().contains("secret.example.com"));
    assert!(!raw.value().contains("secret-password"));
    assert!(!raw.value().contains("admin-password"));
}

#[tokio::test]
async fn mqtt_profile_delete_and_missing_lookup_are_scoped_to_one_profile() {
    let (storage, _tmp) = make_test_storage();
    let deleted = sample_profile("deleted", "deleted.example.com");
    let preserved = sample_profile("preserved", "preserved.example.com");
    storage.save_mqtt_profile(&deleted).await.unwrap();
    storage.save_mqtt_profile(&preserved).await.unwrap();

    storage.delete_mqtt_profile(&deleted.id).await.unwrap();

    assert!(
        storage
            .get_mqtt_profile(&deleted.id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        storage.list_mqtt_profiles().await.unwrap(),
        vec![preserved.clone()]
    );
}

#[tokio::test]
async fn mqtt_profiles_survive_reopen_and_require_the_original_key() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("mqtt.redb");
    let key = [0x52; 32];
    let profile = sample_profile("reopen", "localhost");

    {
        let storage = RedbStorage::open_with_key(&path, &key).unwrap();
        storage.save_mqtt_profile(&profile).await.unwrap();
        assert!(crate::database_has_encrypted_records(&storage.db).unwrap());
    }

    let reopened = RedbStorage::open_with_key(&path, &key).unwrap();
    assert_eq!(
        reopened.get_mqtt_profile(&profile.id).await.unwrap(),
        Some(profile)
    );
    drop(reopened);
    assert!(RedbStorage::open_with_key(&path, &[0x53; 32]).is_err());
}

#[tokio::test]
async fn invalid_mqtt_profile_is_rejected_before_writing() {
    let (storage, _tmp) = make_test_storage();
    let mut invalid = sample_profile("invalid", "localhost");
    invalid.name = "n".repeat(MAX_MQTT_PROFILE_NAME_BYTES + 1);

    assert!(storage.save_mqtt_profile(&invalid).await.is_err());
    assert!(storage.list_mqtt_profiles().await.unwrap().is_empty());
}
