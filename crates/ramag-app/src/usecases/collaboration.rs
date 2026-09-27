//! 本机优先协作共享包用例；所有正文读写都经过加密 Storage，不负责网络传输。

use std::sync::Arc;

use ramag_domain::entities::{
    CollaborationArtifact, CollaborationMutationError, CollaborationShare, CollaborationShareId,
    CollaborationValidationError,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::{CollaborationRelay, Storage};
use thiserror::Error;

/// 编排本机共享包的创建、版本更新、手动导出准备和撤销。
pub struct CollaborationService {
    storage: Arc<dyn Storage>,
    relay: Option<Arc<dyn CollaborationRelay>>,
}

impl CollaborationService {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self {
            storage,
            relay: None,
        }
    }

    pub fn with_relay(storage: Arc<dyn Storage>, relay: Arc<dyn CollaborationRelay>) -> Self {
        Self {
            storage,
            relay: Some(relay),
        }
    }

    pub async fn list(&self) -> Result<Vec<CollaborationShare>> {
        self.storage.list_collaboration_shares().await
    }

    pub async fn get(&self, id: &CollaborationShareId) -> Result<Option<CollaborationShare>> {
        self.storage.get_collaboration_share(id).await
    }

    /// 创建只保存在本机加密存储中的草稿。
    pub async fn create_local(
        &self,
        title: impl Into<String>,
        artifacts: Vec<CollaborationArtifact>,
    ) -> std::result::Result<CollaborationShare, CollaborationServiceError> {
        let share = CollaborationShare::new_local(title, artifacts)?;
        self.storage.save_collaboration_share(&share).await?;
        Ok(share)
    }

    /// 使用期望 revision 更新；冲突时只追加审计，不覆盖当前正文。
    pub async fn update_local(
        &self,
        id: &CollaborationShareId,
        expected_revision: u64,
        title: impl Into<String>,
        artifacts: Vec<CollaborationArtifact>,
        actor: &str,
    ) -> std::result::Result<CollaborationShare, CollaborationServiceError> {
        let mut share = self.find_required(id).await?;
        match share.update_local(expected_revision, title, artifacts, actor) {
            Ok(()) => {}
            Err(error @ CollaborationMutationError::Conflict { .. }) => {
                share.record_conflict(actor)?;
                self.storage.save_collaboration_share(&share).await?;
                return Err(error.into());
            }
            Err(error) => return Err(error.into()),
        }
        self.storage.save_collaboration_share(&share).await?;
        Ok(share)
    }

    /// 将草稿切换为可手动导出的状态；不会自动发送或创建远端记录。
    pub async fn prepare_manual_export(
        &self,
        id: &CollaborationShareId,
        actor: &str,
    ) -> std::result::Result<CollaborationShare, CollaborationServiceError> {
        let mut share = self.find_required(id).await?;
        share.prepare_manual_export(actor)?;
        self.storage.save_collaboration_share(&share).await?;
        Ok(share)
    }

    /// 重新读取并校验可导出的共享包；调用方可在用户确认后交给平台剪贴板。
    pub async fn manual_export_json(
        &self,
        id: &CollaborationShareId,
    ) -> std::result::Result<String, CollaborationServiceError> {
        let share = self.find_required(id).await?;
        Ok(share.manual_export_json()?)
    }

    /// 用户确认后发布安全导出包；没有配置 Relay 时明确拒绝，不自动降级为网络请求。
    pub async fn publish_remote(
        &self,
        id: &CollaborationShareId,
        endpoint: &str,
    ) -> std::result::Result<ramag_domain::CollaborationRemoteReceipt, CollaborationServiceError>
    {
        let relay = self
            .relay
            .as_ref()
            .ok_or(CollaborationServiceError::RelayUnavailable)?;
        let payload = self.manual_export_json(id).await?;
        relay
            .publish(endpoint, &payload)
            .await
            .map_err(CollaborationServiceError::Relay)
    }

    /// 读取远端包后重新按本机导入规则校验，并生成新的本机草稿。
    pub async fn import_remote(
        &self,
        endpoint: &str,
        remote_id: &str,
        actor: &str,
    ) -> std::result::Result<CollaborationShare, CollaborationServiceError> {
        let relay = self
            .relay
            .as_ref()
            .ok_or(CollaborationServiceError::RelayUnavailable)?;
        let remote = relay
            .fetch(endpoint, remote_id)
            .await
            .map_err(CollaborationServiceError::Relay)?;
        let share = CollaborationShare::import_manual_export_json(&remote.payload, actor)?;
        self.storage.save_collaboration_share(&share).await?;
        Ok(share)
    }

    /// 导入用户明确提供的导出文本，并以新的本机草稿 ID 保存，避免覆盖现有包。
    pub async fn import_manual_export(
        &self,
        encoded: &str,
        actor: &str,
    ) -> std::result::Result<CollaborationShare, CollaborationServiceError> {
        let share = CollaborationShare::import_manual_export_json(encoded, actor)?;
        self.storage.save_collaboration_share(&share).await?;
        Ok(share)
    }

    pub async fn revoke(
        &self,
        id: &CollaborationShareId,
        actor: &str,
    ) -> std::result::Result<CollaborationShare, CollaborationServiceError> {
        let mut share = self.find_required(id).await?;
        share.revoke(actor)?;
        self.storage.save_collaboration_share(&share).await?;
        Ok(share)
    }

    pub async fn delete(&self, id: &CollaborationShareId) -> Result<()> {
        self.storage.delete_collaboration_share(id).await
    }

    async fn find_required(
        &self,
        id: &CollaborationShareId,
    ) -> std::result::Result<CollaborationShare, CollaborationServiceError> {
        self.storage
            .get_collaboration_share(id)
            .await?
            .ok_or_else(|| CollaborationServiceError::NotFound { id: id.clone() })
    }
}

#[derive(Debug, Error)]
pub enum CollaborationServiceError {
    #[error("协作共享包 `{id}` 不存在")]
    NotFound { id: CollaborationShareId },
    #[error("共享包校验失败：{0}")]
    Validation(#[from] CollaborationValidationError),
    #[error("共享包修改失败：{0}")]
    Mutation(#[from] CollaborationMutationError),
    #[error(transparent)]
    Storage(#[from] DomainError),
    #[error("远程协作 Relay 未配置")]
    RelayUnavailable,
    #[error("远程协作传输失败：{0}")]
    Relay(DomainError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures::executor::block_on;
    use ramag_domain::entities::{
        CollaborationDataClass, CollaborationRemotePackage, CollaborationRemoteReceipt,
        CollaborationShareState,
    };
    use ramag_domain::error::DomainError;
    use ramag_domain::traits::CollaborationRelay;
    use ramag_infra_storage::RedbStorage;
    use std::path::Path;
    use tempfile::TempDir;

    fn service() -> (CollaborationService, TempDir) {
        let directory = TempDir::new().unwrap();
        let storage =
            RedbStorage::open_with_key(&directory.path().join("collaboration.redb"), &[9; 32])
                .unwrap();
        (CollaborationService::new(Arc::new(storage)), directory)
    }

    struct FakeRelay {
        payload: std::sync::Mutex<Option<String>>,
    }

    #[async_trait]
    impl CollaborationRelay for FakeRelay {
        async fn publish(
            &self,
            _endpoint: &str,
            payload: &str,
        ) -> ramag_domain::error::Result<CollaborationRemoteReceipt> {
            *self.payload.lock().unwrap() = Some(payload.to_owned());
            Ok(CollaborationRemoteReceipt {
                remote_id: "remote-1".into(),
                revision: 1,
                expires_at: None,
            })
        }

        async fn fetch(
            &self,
            _endpoint: &str,
            _remote_id: &str,
        ) -> ramag_domain::error::Result<CollaborationRemotePackage> {
            let payload = self
                .payload
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| DomainError::NotFound("fake relay empty".into()))?;
            Ok(CollaborationRemotePackage {
                receipt: CollaborationRemoteReceipt {
                    remote_id: "remote-1".into(),
                    revision: 1,
                    expires_at: None,
                },
                payload,
            })
        }
    }

    #[test]
    fn local_lifecycle_is_encrypted_and_audited() {
        let (service, directory) = service();
        let share = block_on(service.create_local(
            "本机草稿",
            vec![CollaborationArtifact::document("说明", "safe")],
        ))
        .unwrap();
        let prepared = block_on(service.prepare_manual_export(&share.id, "alice")).unwrap();
        assert_eq!(prepared.state, CollaborationShareState::Shared);
        let revoked = block_on(service.revoke(&share.id, "alice")).unwrap();
        assert_eq!(revoked.state, CollaborationShareState::Revoked);
        let stored = block_on(service.get(&share.id)).unwrap().unwrap();
        assert_eq!(stored.audit.len(), 3);
        assert!(Path::new(directory.path()).exists());
    }

    #[test]
    fn stale_update_is_rejected_and_conflict_is_persisted() {
        let (service, _directory) = service();
        let share = block_on(service.create_local(
            "旧标题",
            vec![CollaborationArtifact::document("说明", "safe")],
        ))
        .unwrap();
        let updated = block_on(service.update_local(
            &share.id,
            share.revision,
            "新标题",
            vec![CollaborationArtifact::document("说明", "new")],
            "alice",
        ))
        .unwrap();
        let error = block_on(service.update_local(
            &share.id,
            share.revision,
            "过期标题",
            vec![CollaborationArtifact::document("说明", "stale")],
            "bob",
        ))
        .unwrap_err();
        assert!(matches!(
            error,
            CollaborationServiceError::Mutation(CollaborationMutationError::Conflict { .. })
        ));
        let stored = block_on(service.get(&share.id)).unwrap().unwrap();
        assert_eq!(stored.title, updated.title);
        assert_eq!(
            stored.audit.last().map(|event| event.action),
            Some(ramag_domain::entities::CollaborationAuditAction::ConflictDetected)
        );
    }

    #[test]
    fn sensitive_local_draft_cannot_be_exported() {
        let (service, _directory) = service();
        let artifact = CollaborationArtifact {
            data_class: CollaborationDataClass::Sensitive,
            ..CollaborationArtifact::document("内部", "sensitive")
        };
        let share = block_on(service.create_local("内部草稿", vec![artifact])).unwrap();
        let error = block_on(service.prepare_manual_export(&share.id, "alice")).unwrap_err();
        assert!(matches!(error, CollaborationServiceError::Mutation(_)));
    }

    #[test]
    fn imported_export_becomes_a_new_local_record() {
        let (service, _directory) = service();
        let source = block_on(service.create_local(
            "可导出说明",
            vec![CollaborationArtifact::document("说明", "safe")],
        ))
        .unwrap();
        let exported = block_on(service.prepare_manual_export(&source.id, "alice")).unwrap();
        let encoded = exported.manual_export_json().unwrap();
        let imported = block_on(service.import_manual_export(&encoded, "bob")).unwrap();
        assert_ne!(source.id, imported.id);
        assert_eq!(block_on(service.list()).unwrap().len(), 2);
    }

    #[test]
    fn service_revalidates_export_before_handoff() {
        let (service, _directory) = service();
        let share = block_on(service.create_local(
            "交接说明",
            vec![CollaborationArtifact::document("说明", "safe")],
        ))
        .unwrap();
        assert!(block_on(service.manual_export_json(&share.id)).is_err());
        let prepared = block_on(service.prepare_manual_export(&share.id, "alice")).unwrap();
        let encoded = block_on(service.manual_export_json(&prepared.id)).unwrap();
        assert!(encoded.contains("交接说明"));
    }

    #[test]
    fn relay_publishes_only_validated_export_and_imports_new_local_record() {
        let directory = TempDir::new().unwrap();
        let storage = ramag_infra_storage::RedbStorage::open_with_key(
            &directory.path().join("collaboration.redb"),
            &[9; 32],
        )
        .unwrap();
        let relay = Arc::new(FakeRelay {
            payload: std::sync::Mutex::new(None),
        });
        let service = CollaborationService::with_relay(Arc::new(storage), relay.clone());
        let share = block_on(service.create_local(
            "远程交接",
            vec![CollaborationArtifact::document("说明", "safe")],
        ))
        .unwrap();
        assert!(block_on(service.publish_remote(&share.id, "https://relay.example")).is_err());
        let share = block_on(service.prepare_manual_export(&share.id, "alice")).unwrap();
        let receipt = block_on(service.publish_remote(&share.id, "https://relay.example")).unwrap();
        assert_eq!(receipt.remote_id, "remote-1");
        let imported =
            block_on(service.import_remote("https://relay.example", "remote-1", "bob")).unwrap();
        assert_ne!(imported.id, share.id);
        assert_eq!(imported.state, CollaborationShareState::LocalDraft);
    }
}
