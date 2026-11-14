use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use ramag_domain::Storage;
use ramag_domain::entities::{ConnectionConfig, ConnectionId, QueryRecord, QueryRecordId};
use ramag_domain::error::{DomainError, Result as DomainResult};

use super::*;

/// 为宿主初始化失败测试提供可控的设置读取和秘密解封错误。
#[derive(Clone, Copy)]
enum HostStorageFailure {
    SettingsRead,
    SecretUnseal,
}

struct HostTestStorage {
    preferences: Mutex<HashMap<String, String>>,
    failure: HostStorageFailure,
}

impl HostTestStorage {
    fn new(failure: HostStorageFailure) -> Self {
        Self {
            preferences: Mutex::new(HashMap::new()),
            failure,
        }
    }

    fn raw_set(&self, key: &str, value: &str) {
        self.preferences
            .lock()
            .insert(key.to_owned(), value.to_owned());
    }
}

#[async_trait]
impl Storage for HostTestStorage {
    async fn list_connections(&self) -> DomainResult<Vec<ConnectionConfig>> {
        Ok(Vec::new())
    }

    async fn get_connection(&self, _id: &ConnectionId) -> DomainResult<Option<ConnectionConfig>> {
        Ok(None)
    }

    async fn save_connection(&self, _config: &ConnectionConfig) -> DomainResult<()> {
        Ok(())
    }

    async fn delete_connection(&self, _id: &ConnectionId) -> DomainResult<()> {
        Ok(())
    }

    async fn append_history(&self, _record: &QueryRecord) -> DomainResult<()> {
        Ok(())
    }

    async fn list_history(
        &self,
        _connection_id: Option<&ConnectionId>,
        _limit: usize,
    ) -> DomainResult<Vec<QueryRecord>> {
        Ok(Vec::new())
    }

    async fn delete_history(&self, _id: &QueryRecordId) -> DomainResult<()> {
        Ok(())
    }

    async fn clear_history(&self, _connection_id: Option<&ConnectionId>) -> DomainResult<()> {
        Ok(())
    }

    async fn get_preference(&self, key: &str) -> DomainResult<Option<String>> {
        if matches!(self.failure, HostStorageFailure::SettingsRead)
            && key.ends_with("__snapshot.v1")
        {
            return Err(DomainError::Storage("设置快照不可读取".into()));
        }
        Ok(self.preferences.lock().get(key).cloned())
    }

    async fn set_preference(&self, key: &str, value: &str) -> DomainResult<()> {
        self.raw_set(key, value);
        Ok(())
    }

    async fn seal(&self, plain: &[u8]) -> DomainResult<Vec<u8>> {
        Ok(plain.iter().rev().copied().collect())
    }

    async fn unseal(&self, _cipher: &[u8]) -> DomainResult<Vec<u8>> {
        if matches!(self.failure, HostStorageFailure::SecretUnseal) {
            return Err(DomainError::Storage("秘密解封不可用".into()));
        }
        Ok(Vec::new())
    }
}

#[test]
fn settings_snapshot_failure_isolated_before_plugin_callback() {
    let storage = Arc::new(HostTestStorage::new(HostStorageFailure::SettingsRead));
    let events = Arc::new(Mutex::new(Vec::new()));
    let host = StaticPluginHost::with_storage_and_permissions(
        Arc::new(ToolRegistry::new()),
        storage,
        permissions_for_settings_plugin(),
        PluginSettingsMigrator::default(),
    );
    host.register_plugin(Arc::new(SettingsPlugin::new(events.clone())))
        .unwrap();

    let report = host.initialize_all();

    assert_eq!(report.succeeded.len(), 0);
    assert_eq!(report.failures.len(), 1);
    assert!(events.lock().is_empty());
    assert_eq!(host.registry().count(), 0);
    assert_eq!(host.state("settings.example"), Some(PluginState::Failed));
    let diagnostic = host
        .diagnostics()
        .into_iter()
        .find(|diagnostic| diagnostic.descriptor.id.as_str() == "settings.example")
        .expect("设置加载失败应保留初始化诊断");
    assert_eq!(
        diagnostic.failure.as_ref().map(|failure| failure.stage),
        Some(PluginLifecycleStage::Initialize)
    );
    assert!(
        diagnostic
            .failure
            .as_ref()
            .is_some_and(|failure| failure.message.contains("设置快照不可读取"))
    );
}

#[test]
fn secret_unseal_failure_isolated_before_plugin_callback() {
    let storage = Arc::new(HostTestStorage::new(HostStorageFailure::SecretUnseal));
    storage.raw_set("plugin.settings.example.__secrets.v1", "encrypted-v1:00");
    let events = Arc::new(Mutex::new(Vec::new()));
    let host = StaticPluginHost::with_storage_and_permissions(
        Arc::new(ToolRegistry::new()),
        storage,
        permissions_for_settings_plugin(),
        PluginSettingsMigrator::default(),
    );
    host.register_plugin(Arc::new(SettingsPlugin::new(events.clone())))
        .unwrap();

    let report = host.initialize_all();

    assert_eq!(report.succeeded.len(), 0);
    assert_eq!(report.failures.len(), 1);
    assert!(events.lock().is_empty());
    assert_eq!(host.registry().count(), 0);
    assert_eq!(host.state("settings.example"), Some(PluginState::Failed));
    let diagnostic = host
        .diagnostics()
        .into_iter()
        .find(|diagnostic| diagnostic.descriptor.id.as_str() == "settings.example")
        .expect("秘密解封失败应保留初始化诊断");
    assert_eq!(
        diagnostic.failure.as_ref().map(|failure| failure.stage),
        Some(PluginLifecycleStage::Initialize)
    );
    assert!(
        diagnostic
            .failure
            .as_ref()
            .is_some_and(|failure| failure.message.contains("秘密解封不可用"))
    );
}
