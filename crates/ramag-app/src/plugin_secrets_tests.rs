use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use futures::executor::block_on;
use parking_lot::Mutex;
use ramag_domain::entities::{ConnectionConfig, ConnectionId, QueryRecord, QueryRecordId};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::Storage;
use ramag_domain::{
    PluginDescriptor, PluginId, PluginSettingDefinition, PluginSettingKind, PluginSettingValue,
};

use super::*;

#[derive(Default)]
struct MemorySecretStorage {
    preferences: Mutex<HashMap<String, String>>,
}

impl MemorySecretStorage {
    fn raw_set(&self, key: &str, value: &str) {
        self.preferences
            .lock()
            .insert(key.to_owned(), value.to_owned());
    }
}

#[async_trait]
impl Storage for MemorySecretStorage {
    async fn list_connections(&self) -> Result<Vec<ConnectionConfig>> {
        Ok(Vec::new())
    }

    async fn get_connection(&self, _id: &ConnectionId) -> Result<Option<ConnectionConfig>> {
        Ok(None)
    }

    async fn save_connection(&self, _config: &ConnectionConfig) -> Result<()> {
        Ok(())
    }

    async fn delete_connection(&self, _id: &ConnectionId) -> Result<()> {
        Ok(())
    }

    async fn append_history(&self, _record: &QueryRecord) -> Result<()> {
        Ok(())
    }

    async fn list_history(
        &self,
        _connection_id: Option<&ConnectionId>,
        _limit: usize,
    ) -> Result<Vec<QueryRecord>> {
        Ok(Vec::new())
    }

    async fn delete_history(&self, _id: &QueryRecordId) -> Result<()> {
        Ok(())
    }

    async fn clear_history(&self, _connection_id: Option<&ConnectionId>) -> Result<()> {
        Ok(())
    }

    async fn get_preference(&self, key: &str) -> Result<Option<String>> {
        Ok(self.preferences.lock().get(key).cloned())
    }

    async fn set_preference(&self, key: &str, value: &str) -> Result<()> {
        self.raw_set(key, value);
        Ok(())
    }

    async fn seal(&self, plain: &[u8]) -> Result<Vec<u8>> {
        let mut encrypted = plain.to_vec();
        encrypted.reverse();
        Ok(encrypted)
    }

    async fn unseal(&self, cipher: &[u8]) -> Result<Vec<u8>> {
        let mut plain = cipher.to_vec();
        plain.reverse();
        Ok(plain)
    }
}

fn descriptor() -> PluginDescriptor {
    PluginDescriptor::new(
        PluginId::new("example.tool").unwrap(),
        "Example Tool",
        "example.tool",
    )
    .with_settings(vec![
        PluginSettingDefinition::new("token", PluginSettingKind::String).sensitive(true),
        PluginSettingDefinition::new("enabled", PluginSettingKind::Boolean)
            .with_default(PluginSettingValue::Boolean(true)),
    ])
}

fn secret_snapshot(value: &str) -> PluginSecretSnapshot {
    PluginSecretSnapshot::from_namespaced_values(
        &descriptor(),
        [(
            "plugin.example.tool.token",
            PluginSettingValue::String(value.into()),
        )],
    )
    .unwrap()
}

#[test]
fn secret_snapshot_accepts_only_sensitive_declared_values() {
    let snapshot = secret_snapshot("first");
    assert_eq!(
        snapshot.get("token"),
        Some(&PluginSettingValue::String("first".into()))
    );
    let updated = snapshot
        .update("token", PluginSettingValue::String("second".into()))
        .unwrap();
    assert_eq!(
        updated.get("token"),
        Some(&PluginSettingValue::String("second".into()))
    );

    assert!(matches!(
        PluginSecretSnapshot::from_namespaced_values(
            &descriptor(),
            [("plugin.example.tool.enabled", PluginSettingValue::Boolean(true))],
        ),
        Err(PluginSecretError::NotSensitive { key }) if key == "enabled"
    ));
}

#[test]
fn secret_store_encrypts_and_round_trips_without_plaintext_preference() {
    block_on(async {
        let storage = Arc::new(MemorySecretStorage::default());
        let store = PluginSecretStore::new(storage.clone());
        store.save(&secret_snapshot("super-secret")).await.unwrap();

        let stored = storage
            .preferences
            .lock()
            .get("plugin.example.tool.__secrets.v1")
            .cloned()
            .unwrap();
        assert!(stored.starts_with("encrypted-v1:"));
        assert!(!stored.contains("super-secret"));
        let loaded = store.load(&descriptor()).await.unwrap();
        assert_eq!(
            loaded.get("token"),
            secret_snapshot("super-secret").get("token")
        );
    });
}

#[test]
fn corrupt_primary_secret_recovers_encrypted_backup() {
    block_on(async {
        let storage = Arc::new(MemorySecretStorage::default());
        let store = PluginSecretStore::new(storage.clone());
        store.save(&secret_snapshot("old")).await.unwrap();
        store.save(&secret_snapshot("new")).await.unwrap();
        storage.raw_set("plugin.example.tool.__secrets.v1", "not-encrypted");

        let loaded = store.load(&descriptor()).await.unwrap();
        assert_eq!(loaded.get("token"), secret_snapshot("old").get("token"));
    });
}

#[test]
fn malformed_secret_without_backup_is_rejected_without_echoing_plaintext() {
    block_on(async {
        let storage = Arc::new(MemorySecretStorage::default());
        storage.raw_set(
            "plugin.example.tool.__secrets.v1",
            "secret-plaintext-must-not-be-accepted",
        );
        let store = PluginSecretStore::new(storage);

        let error = store.load(&descriptor()).await.unwrap_err();
        assert!(matches!(error, PluginSecretStoreError::CorruptPayload));
        assert!(!error.to_string().contains("secret-plaintext"));
    });
}

#[test]
fn empty_secret_storage_returns_empty_secret_snapshot() {
    block_on(async {
        let store = PluginSecretStore::new(Arc::new(MemorySecretStorage::default()));
        let loaded = store.load(&descriptor()).await.unwrap();
        assert_eq!(loaded.get("token"), None);
    });
}

#[test]
fn secret_snapshot_rejects_foreign_namespace_and_wrong_type() {
    assert!(matches!(
        PluginSecretSnapshot::from_namespaced_values(
            &descriptor(),
            [(
                "plugin.other.tool.token",
                PluginSettingValue::String("x".into())
            )],
        ),
        Err(PluginSecretError::NamespaceMismatch { .. })
    ));
    assert!(matches!(
        PluginSecretSnapshot::from_namespaced_values(
            &descriptor(),
            [(
                "plugin.example.tool.token",
                PluginSettingValue::Boolean(true)
            )],
        ),
        Err(PluginSecretError::InvalidValue { .. })
    ));
}

#[test]
fn secret_store_error_does_not_include_secret_payload() {
    let error = PluginSecretStoreError::Storage {
        source: DomainError::Storage("sealed storage unavailable".into()),
    };
    assert!(!error.to_string().contains("super-secret"));
}
