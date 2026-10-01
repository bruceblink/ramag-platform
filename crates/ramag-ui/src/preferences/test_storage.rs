use async_trait::async_trait;
use ramag_domain::entities::{ConnectionConfig, ConnectionId, QueryRecord, QueryRecordId};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::Storage;
use ramag_infra_storage::RedbStorage;
use smol::block_on;
use std::sync::atomic::{AtomicBool, Ordering};

/// Headless settings tests use the production database and its real disk format.
/// This adapter blocks each storage operation until the production worker pool
/// finishes, so GPUI's test scheduler does not need to process a cross-thread wake.
/// It validates persistence behavior, but does not show that runtime UI operations
/// remain nonblocking while storage is busy.
pub(crate) struct SettingsTestStorage {
    inner: RedbStorage,
    fail_next_preference_write: AtomicBool,
}

impl SettingsTestStorage {
    /// Wraps an already-open test database without changing its storage behavior.
    pub(crate) fn new(inner: RedbStorage) -> Self {
        Self {
            inner,
            fail_next_preference_write: AtomicBool::new(false),
        }
    }

    /// Arm one deterministic preference-write failure for rollback acceptance.
    pub(crate) fn fail_next_preference_write(&self) {
        self.fail_next_preference_write
            .store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl Storage for SettingsTestStorage {
    async fn list_connections(&self) -> Result<Vec<ConnectionConfig>> {
        block_on(self.inner.list_connections())
    }

    async fn get_connection(&self, id: &ConnectionId) -> Result<Option<ConnectionConfig>> {
        block_on(self.inner.get_connection(id))
    }

    async fn save_connection(&self, config: &ConnectionConfig) -> Result<()> {
        block_on(self.inner.save_connection(config))
    }

    async fn delete_connection(&self, id: &ConnectionId) -> Result<()> {
        block_on(self.inner.delete_connection(id))
    }

    async fn append_history(&self, record: &QueryRecord) -> Result<()> {
        block_on(self.inner.append_history(record))
    }

    async fn list_history(
        &self,
        connection_id: Option<&ConnectionId>,
        limit: usize,
    ) -> Result<Vec<QueryRecord>> {
        block_on(self.inner.list_history(connection_id, limit))
    }

    async fn delete_history(&self, id: &QueryRecordId) -> Result<()> {
        block_on(self.inner.delete_history(id))
    }

    async fn clear_history(&self, connection_id: Option<&ConnectionId>) -> Result<()> {
        block_on(self.inner.clear_history(connection_id))
    }

    async fn get_preference(&self, key: &str) -> Result<Option<String>> {
        block_on(self.inner.get_preference(key))
    }

    async fn set_preference(&self, key: &str, value: &str) -> Result<()> {
        if self
            .fail_next_preference_write
            .swap(false, Ordering::SeqCst)
        {
            return Err(DomainError::Storage(
                "simulated preference write failure".into(),
            ));
        }
        block_on(self.inner.set_preference(key, value))
    }
}
