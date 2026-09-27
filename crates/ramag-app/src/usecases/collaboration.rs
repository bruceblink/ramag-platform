//! 本机优先协作共享包用例；所有正文读写都经过加密 Storage，不负责网络传输。

use std::sync::Arc;

use ramag_domain::entities::{
    CollaborationArtifact, CollaborationMutationError, CollaborationShare, CollaborationShareId,
    CollaborationValidationError,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::Storage;
use thiserror::Error;

/// 编排本机共享包的创建、版本更新、手动导出准备和撤销。
pub struct CollaborationService {
    storage: Arc<dyn Storage>,
}

impl CollaborationService {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use ramag_domain::entities::{CollaborationDataClass, CollaborationShareState};
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
}
