use ramag_domain::entities::{
    ApiHistoryRecord, ApiWorkspace, ApiWorkspaceId, ConnectionConfig, ConnectionId, QueryRecord,
    QueryRecordId,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::Storage;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub(super) struct WorkspaceTestStorage {
    pub(super) fail_listing: AtomicBool,
    pub(super) fail_history: AtomicBool,
    pub(super) fail_clear_history: AtomicBool,
    pub(super) workspaces: Mutex<Vec<ApiWorkspace>>,
    list_gate: Mutex<Option<WorkspaceListGate>>,
    save_gate: Mutex<Option<WorkspaceSaveGate>>,
    pub(super) save_calls: AtomicUsize,
    pub(super) clear_history_calls: AtomicUsize,
}

struct WorkspaceSaveGate {
    started: async_channel::Sender<()>,
    release: async_channel::Receiver<bool>,
}

struct WorkspaceListGate {
    started: async_channel::Sender<()>,
    release: async_channel::Receiver<bool>,
}

impl WorkspaceTestStorage {
    pub(super) fn new(fail_listing: bool, workspaces: Vec<ApiWorkspace>) -> Self {
        Self {
            fail_listing: AtomicBool::new(fail_listing),
            fail_history: AtomicBool::new(false),
            fail_clear_history: AtomicBool::new(false),
            workspaces: Mutex::new(workspaces),
            list_gate: Mutex::new(None),
            save_gate: Mutex::new(None),
            save_calls: AtomicUsize::new(0),
            clear_history_calls: AtomicUsize::new(0),
        }
    }

    pub(super) fn delay_next_listing(
        &self,
    ) -> (async_channel::Receiver<()>, async_channel::Sender<bool>) {
        let (started_sender, started_receiver) = async_channel::bounded(1);
        let (release_sender, release_receiver) = async_channel::bounded(1);
        *self.list_gate.lock().expect("锁定读取测试控制") = Some(WorkspaceListGate {
            started: started_sender,
            release: release_receiver,
        });
        (started_receiver, release_sender)
    }

    pub(super) fn delay_next_save(
        &self,
    ) -> (async_channel::Receiver<()>, async_channel::Sender<bool>) {
        let (started_sender, started_receiver) = async_channel::bounded(1);
        let (release_sender, release_receiver) = async_channel::bounded(1);
        *self.save_gate.lock().expect("锁定保存测试控制") = Some(WorkspaceSaveGate {
            started: started_sender,
            release: release_receiver,
        });
        (started_receiver, release_sender)
    }
}

#[async_trait::async_trait]
impl Storage for WorkspaceTestStorage {
    async fn list_api_workspaces(&self) -> Result<Vec<ApiWorkspace>> {
        let gate = self.list_gate.lock().expect("锁定读取测试控制").take();
        if let Some(gate) = gate {
            gate.started
                .send(())
                .await
                .map_err(|error| DomainError::Storage(error.to_string()))?;
            if !gate
                .release
                .recv()
                .await
                .map_err(|error| DomainError::Storage(error.to_string()))?
            {
                return Err(DomainError::Storage("list-secret-sentinel".into()));
            }
        }
        if self.fail_listing.load(Ordering::Relaxed) {
            return Err(DomainError::Storage(
                "secret-token and request-body sentinel".into(),
            ));
        }
        Ok(self.workspaces.lock().expect("锁定测试工作区").clone())
    }

    async fn list_api_history(
        &self,
        _workspace_id: &ApiWorkspaceId,
        _limit: usize,
    ) -> Result<Vec<ApiHistoryRecord>> {
        if self.fail_history.load(Ordering::Relaxed) {
            return Err(DomainError::Storage("history-secret-sentinel".into()));
        }
        Ok(Vec::new())
    }

    async fn save_api_workspace(&self, workspace: &ApiWorkspace) -> Result<()> {
        self.save_calls.fetch_add(1, Ordering::Relaxed);
        let gate = self.save_gate.lock().expect("锁定保存测试控制").take();
        if let Some(gate) = gate {
            gate.started
                .send(())
                .await
                .map_err(|error| DomainError::Storage(error.to_string()))?;
            if !gate
                .release
                .recv()
                .await
                .map_err(|error| DomainError::Storage(error.to_string()))?
            {
                return Err(DomainError::Storage("save-secret-sentinel".into()));
            }
        }
        let mut workspaces = self.workspaces.lock().expect("锁定测试工作区");
        if let Some(existing) = workspaces
            .iter_mut()
            .find(|existing| existing.id == workspace.id)
        {
            *existing = workspace.clone();
        } else {
            workspaces.push(workspace.clone());
        }
        Ok(())
    }

    async fn clear_api_history(&self, _workspace_id: &ApiWorkspaceId) -> Result<()> {
        self.clear_history_calls.fetch_add(1, Ordering::Relaxed);
        if self.fail_clear_history.load(Ordering::Relaxed) {
            Err(DomainError::Storage("clear-history-secret-sentinel".into()))
        } else {
            Ok(())
        }
    }

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

    async fn get_preference(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn set_preference(&self, _key: &str, _value: &str) -> Result<()> {
        Ok(())
    }
}
