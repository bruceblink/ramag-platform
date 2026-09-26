use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
struct MemoryStorage {
    preferences: Mutex<HashMap<String, String>>,
    fail_next_primary_write: AtomicBool,
}

impl MemoryStorage {
    fn raw_set(&self, key: &str, value: &str) {
        self.preferences
            .lock()
            .insert(key.to_owned(), value.to_owned());
    }

    fn raw_remove(&self, key: &str) {
        self.preferences.lock().remove(key);
    }
}

#[async_trait]
impl Storage for MemoryStorage {
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
        if key.ends_with("__snapshot.v1")
            && self.fail_next_primary_write.swap(false, Ordering::AcqRel)
        {
            return Err(DomainError::Storage("模拟主设置写入失败".into()));
        }
        self.raw_set(key, value);
        Ok(())
    }
}

fn descriptor() -> PluginDescriptor {
    PluginDescriptor::new(
        PluginId::new("example.tool").unwrap(),
        "Example Tool",
        "example.tool",
    )
    .with_settings(vec![
        PluginSettingDefinition::new("mode", PluginSettingKind::Enum)
            .with_enum_values(["safe", "full"])
            .with_default(PluginSettingValue::String("safe".into())),
    ])
}

fn snapshot(mode: &str) -> PluginSettingsSnapshot {
    PluginSettingsSnapshot::from_namespaced_values(
        &descriptor(),
        [(
            "plugin.example.tool.mode",
            PluginSettingValue::String(mode.into()),
        )],
    )
    .unwrap()
}

#[test]
fn save_and_load_round_trip_uses_primary_record() {
    block_on(async {
        let storage = Arc::new(MemoryStorage::default());
        let store = PluginSettingsStore::new(storage);
        store.save(&snapshot("full")).await.unwrap();

        let loaded = store.load(&descriptor()).await.unwrap();
        assert!(!loaded.recovered_from_backup);
        assert_eq!(loaded.snapshot.get("mode"), snapshot("full").get("mode"));
    });
}

#[test]
fn corrupt_primary_recovers_previous_valid_snapshot() {
    block_on(async {
        let storage = Arc::new(MemoryStorage::default());
        let store = PluginSettingsStore::new(storage.clone());
        store.save(&snapshot("safe")).await.unwrap();
        store.save(&snapshot("full")).await.unwrap();
        storage.raw_set(&primary_key(&descriptor()), "not-json");

        let loaded = store.load(&descriptor()).await.unwrap();
        assert!(loaded.recovered_from_backup);
        assert_eq!(loaded.snapshot.get("mode"), snapshot("safe").get("mode"));
    });
}

#[test]
fn missing_primary_uses_backup_and_two_corrupt_records_are_rejected() {
    block_on(async {
        let storage = Arc::new(MemoryStorage::default());
        let store = PluginSettingsStore::new(storage.clone());
        store.save(&snapshot("safe")).await.unwrap();
        store.save(&snapshot("full")).await.unwrap();
        storage.raw_remove(&primary_key(&descriptor()));

        let loaded = store.load(&descriptor()).await.unwrap();
        assert!(loaded.recovered_from_backup);
        assert_eq!(loaded.snapshot.get("mode"), snapshot("safe").get("mode"));

        storage.raw_set(&backup_key(&descriptor()), "also-not-json");
        assert!(matches!(
            store.load(&descriptor()).await,
            Err(PluginSettingsStoreError::RecoveryFailed)
        ));
    });
}

#[test]
fn failed_primary_write_keeps_previous_primary_record_readable() {
    block_on(async {
        let storage = Arc::new(MemoryStorage::default());
        let store = PluginSettingsStore::new(storage.clone());
        store.save(&snapshot("safe")).await.unwrap();
        storage
            .fail_next_primary_write
            .store(true, Ordering::Release);

        assert!(matches!(
            store.save(&snapshot("full")).await,
            Err(PluginSettingsStoreError::Storage { .. })
        ));
        let loaded = store.load(&descriptor()).await.unwrap();
        assert!(!loaded.recovered_from_backup);
        assert_eq!(loaded.snapshot.get("mode"), snapshot("safe").get("mode"));
    });
}

#[test]
fn empty_storage_returns_descriptor_defaults() {
    block_on(async {
        let store = PluginSettingsStore::new(Arc::new(MemoryStorage::default()));
        let loaded = store.load(&descriptor()).await.unwrap();
        assert!(!loaded.recovered_from_backup);
        assert_eq!(
            loaded.snapshot.get("mode"),
            Some(&PluginSettingValue::String("safe".into()))
        );
    });
}
