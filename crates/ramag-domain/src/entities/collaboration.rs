//! 本机优先协作共享包：只描述经过用户选择的文档或结果，不负责网络传输。

use std::collections::HashSet;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// 单个共享包允许携带的入口数量。
pub const MAX_COLLABORATION_ARTIFACTS: usize = 32;
/// 本机最多保留的共享包数量。
pub const MAX_COLLABORATION_SHARES: usize = 256;
/// 共享包标题和入口标题的字节上限。
pub const MAX_COLLABORATION_TITLE_BYTES: usize = 256;
pub const MAX_COLLABORATION_ARTIFACT_TITLE_BYTES: usize = 256;
/// 单个入口和整个共享包的明文大小上限。
pub const MAX_COLLABORATION_ARTIFACT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_COLLABORATION_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;
/// 单个共享包保留的审计事件数量。
pub const MAX_COLLABORATION_AUDIT_EVENTS: usize = 128;
pub const MAX_COLLABORATION_ACTOR_BYTES: usize = 128;

/// 共享包的稳定身份。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CollaborationShareId(pub Uuid);

impl CollaborationShareId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for CollaborationShareId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CollaborationShareId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// 用户明确选择的共享内容类型；不接受连接配置或任意凭据对象。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationArtifactKind {
    Document,
    QueryResultPreview,
}

impl fmt::Display for CollaborationArtifactKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Document => "文档",
            Self::QueryResultPreview => "查询结果预览",
        })
    }
}

/// 内容分类决定它能否离开本机；分类来自调用方的明确选择，不从正文推测秘密。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationDataClass {
    NonSensitive,
    UserSelectedBusinessData,
    Sensitive,
    Credential,
    ConnectionConfig,
}

impl CollaborationDataClass {
    fn may_stay_local(self) -> bool {
        !matches!(self, Self::Credential | Self::ConnectionConfig)
    }

    fn may_manual_export(self) -> bool {
        matches!(self, Self::NonSensitive | Self::UserSelectedBusinessData)
    }
}

/// 本切片只有本机草稿和用户明确触发的手动导出；不存在后台自动同步状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationSyncPolicy {
    LocalOnly,
    ManualExport,
}

/// 共享包生命周期；撤销只影响本机包，不能伪称远端已经撤回。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationShareState {
    LocalDraft,
    Shared,
    Revoked,
}

/// 可审计的本机状态变更，不包含正文、凭据或连接信息。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationAuditAction {
    Created,
    Updated,
    Shared,
    Revoked,
    ConflictDetected,
}

/// 共享包内的一个用户选择项；正文仍按上限保存，不在审计记录中重复。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationArtifact {
    pub id: CollaborationShareId,
    pub kind: CollaborationArtifactKind,
    pub title: String,
    pub payload: String,
    pub data_class: CollaborationDataClass,
}

impl CollaborationArtifact {
    /// 创建默认非敏感文档；调用方若选择查询结果或敏感草稿，必须显式修改分类。
    pub fn document(title: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            id: CollaborationShareId::new(),
            kind: CollaborationArtifactKind::Document,
            title: title.into(),
            payload: payload.into(),
            data_class: CollaborationDataClass::NonSensitive,
        }
    }

    pub fn query_result_preview(
        title: impl Into<String>,
        payload: impl Into<String>,
        data_class: CollaborationDataClass,
    ) -> Self {
        Self {
            id: CollaborationShareId::new(),
            kind: CollaborationArtifactKind::QueryResultPreview,
            title: title.into(),
            payload: payload.into(),
            data_class,
        }
    }

    fn validate(
        &self,
        policy: CollaborationSyncPolicy,
    ) -> Result<(), CollaborationValidationError> {
        validate_text(
            "共享入口标题",
            &self.title,
            MAX_COLLABORATION_ARTIFACT_TITLE_BYTES,
        )?;
        if self.payload.is_empty() {
            return Err(CollaborationValidationError::EmptyPayload { kind: self.kind });
        }
        if self.payload.len() > MAX_COLLABORATION_ARTIFACT_BYTES {
            return Err(CollaborationValidationError::PayloadTooLarge {
                kind: self.kind,
                max: MAX_COLLABORATION_ARTIFACT_BYTES,
            });
        }
        if !self.data_class.may_stay_local() {
            return Err(CollaborationValidationError::ForbiddenDataClass {
                kind: self.data_class,
            });
        }
        if policy == CollaborationSyncPolicy::ManualExport && !self.data_class.may_manual_export() {
            return Err(CollaborationValidationError::RestrictedExport {
                kind: self.data_class,
            });
        }
        Ok(())
    }
}

/// 一条不含正文的审计事件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationAuditEvent {
    pub action: CollaborationAuditAction,
    pub revision: u64,
    pub actor: String,
    pub occurred_at: DateTime<Utc>,
}

/// 可本机加密保存、也可在用户明确触发时导出的共享包。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationShare {
    pub id: CollaborationShareId,
    pub title: String,
    pub revision: u64,
    pub state: CollaborationShareState,
    pub sync_policy: CollaborationSyncPolicy,
    pub artifacts: Vec<CollaborationArtifact>,
    pub audit: Vec<CollaborationAuditEvent>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CollaborationShare {
    /// 创建本机草稿；此时内容只会留在主密钥保护的本地存储中。
    pub fn new_local(
        title: impl Into<String>,
        artifacts: Vec<CollaborationArtifact>,
    ) -> Result<Self, CollaborationValidationError> {
        let now = Utc::now();
        let mut share = Self {
            id: CollaborationShareId::new(),
            title: title.into(),
            revision: 1,
            state: CollaborationShareState::LocalDraft,
            sync_policy: CollaborationSyncPolicy::LocalOnly,
            artifacts,
            audit: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        share.push_audit(CollaborationAuditAction::Created, "local-user")?;
        share.validate()?;
        Ok(share)
    }

    /// 校验所有数量、正文边界、状态和敏感数据处理规则。
    pub fn validate(&self) -> Result<(), CollaborationValidationError> {
        validate_text("共享包标题", &self.title, MAX_COLLABORATION_TITLE_BYTES)?;
        if self.revision == 0 {
            return Err(CollaborationValidationError::InvalidRevision);
        }
        if self.artifacts.is_empty() {
            return Err(CollaborationValidationError::NoArtifacts);
        }
        if self.artifacts.len() > MAX_COLLABORATION_ARTIFACTS {
            return Err(CollaborationValidationError::TooManyArtifacts {
                max: MAX_COLLABORATION_ARTIFACTS,
            });
        }
        let mut ids = HashSet::with_capacity(self.artifacts.len());
        let mut payload_bytes = 0usize;
        for artifact in &self.artifacts {
            if !ids.insert(&artifact.id) {
                return Err(CollaborationValidationError::DuplicateArtifact);
            }
            artifact.validate(self.sync_policy)?;
            payload_bytes = payload_bytes.checked_add(artifact.payload.len()).ok_or(
                CollaborationValidationError::PayloadTooLarge {
                    kind: artifact.kind,
                    max: MAX_COLLABORATION_PAYLOAD_BYTES,
                },
            )?;
        }
        if payload_bytes > MAX_COLLABORATION_PAYLOAD_BYTES {
            return Err(CollaborationValidationError::TotalPayloadTooLarge {
                max: MAX_COLLABORATION_PAYLOAD_BYTES,
            });
        }
        if self.audit.is_empty() {
            return Err(CollaborationValidationError::MissingAudit);
        }
        if self.audit.len() > MAX_COLLABORATION_AUDIT_EVENTS {
            return Err(CollaborationValidationError::TooManyAuditEvents {
                max: MAX_COLLABORATION_AUDIT_EVENTS,
            });
        }
        for event in &self.audit {
            validate_text("审计操作者", &event.actor, MAX_COLLABORATION_ACTOR_BYTES)?;
            if event.revision == 0 || event.revision > self.revision {
                return Err(CollaborationValidationError::InvalidAuditRevision);
            }
        }
        match (self.state, self.sync_policy) {
            (CollaborationShareState::LocalDraft, CollaborationSyncPolicy::LocalOnly)
            | (CollaborationShareState::Shared, CollaborationSyncPolicy::ManualExport)
            | (CollaborationShareState::Revoked, _) => Ok(()),
            (CollaborationShareState::LocalDraft, CollaborationSyncPolicy::ManualExport)
            | (CollaborationShareState::Shared, CollaborationSyncPolicy::LocalOnly) => {
                Err(CollaborationValidationError::InvalidStatePolicy)
            }
        }
    }

    /// 以 revision 保护本机编辑；旧客户端不能覆盖新内容。
    pub fn update_local(
        &mut self,
        expected_revision: u64,
        title: impl Into<String>,
        artifacts: Vec<CollaborationArtifact>,
        actor: &str,
    ) -> Result<(), CollaborationMutationError> {
        if self.state == CollaborationShareState::Revoked {
            return Err(CollaborationMutationError::Revoked);
        }
        if self.revision != expected_revision {
            return Err(CollaborationMutationError::Conflict {
                expected: expected_revision,
                actual: self.revision,
            });
        }
        let mut candidate = self.clone();
        candidate.title = title.into();
        candidate.artifacts = artifacts;
        candidate.bump_revision()?;
        candidate.push_audit(CollaborationAuditAction::Updated, actor)?;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// 将本机草稿转换为用户明确触发的手动导出状态；不表示已经发送到远端。
    pub fn prepare_manual_export(&mut self, actor: &str) -> Result<(), CollaborationMutationError> {
        if self.state == CollaborationShareState::Revoked {
            return Err(CollaborationMutationError::Revoked);
        }
        let mut candidate = self.clone();
        candidate.sync_policy = CollaborationSyncPolicy::ManualExport;
        candidate.state = CollaborationShareState::Shared;
        candidate.bump_revision()?;
        candidate.push_audit(CollaborationAuditAction::Shared, actor)?;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// 撤销本机包；历史审计保留，未来远端撤回需要单独的传输协议。
    pub fn revoke(&mut self, actor: &str) -> Result<(), CollaborationMutationError> {
        if self.state == CollaborationShareState::Revoked {
            return Ok(());
        }
        self.bump_revision()?;
        self.state = CollaborationShareState::Revoked;
        self.push_audit(CollaborationAuditAction::Revoked, actor)?;
        self.validate()?;
        Ok(())
    }

    /// 记录冲突但不改写正文，供应用层在发现过期 revision 后保存证据。
    pub fn record_conflict(&mut self, actor: &str) -> Result<(), CollaborationMutationError> {
        self.bump_revision()?;
        self.push_audit(CollaborationAuditAction::ConflictDetected, actor)?;
        self.validate()?;
        Ok(())
    }

    /// 只允许已审核的手动导出状态序列化，避免调用方误把本机草稿当作共享包。
    pub fn manual_export_json(&self) -> Result<String, CollaborationValidationError> {
        self.validate()?;
        if self.state != CollaborationShareState::Shared
            || self.sync_policy != CollaborationSyncPolicy::ManualExport
        {
            return Err(CollaborationValidationError::NotExportable);
        }
        serde_json::to_string(self)
            .map_err(|error| CollaborationValidationError::Serialize(error.to_string()))
    }

    fn bump_revision(&mut self) -> Result<(), CollaborationMutationError> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(CollaborationMutationError::RevisionExhausted)?;
        self.updated_at = Utc::now();
        Ok(())
    }

    fn push_audit(
        &mut self,
        action: CollaborationAuditAction,
        actor: &str,
    ) -> Result<(), CollaborationValidationError> {
        validate_text("审计操作者", actor, MAX_COLLABORATION_ACTOR_BYTES)?;
        if self.audit.len() >= MAX_COLLABORATION_AUDIT_EVENTS {
            return Err(CollaborationValidationError::TooManyAuditEvents {
                max: MAX_COLLABORATION_AUDIT_EVENTS,
            });
        }
        self.audit.push(CollaborationAuditEvent {
            action,
            revision: self.revision,
            actor: actor.to_string(),
            occurred_at: self.updated_at,
        });
        Ok(())
    }
}

/// 修改共享包时的冲突或状态错误；正文不进入错误文本。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CollaborationMutationError {
    #[error("共享包 revision 冲突：期望 {expected}，当前 {actual}")]
    Conflict { expected: u64, actual: u64 },
    #[error("共享包已撤销")]
    Revoked,
    #[error("共享包 revision 已耗尽")]
    RevisionExhausted,
    #[error("共享包校验失败：{0}")]
    Invalid(#[from] CollaborationValidationError),
}

/// 共享包校验错误；只含字段和上限，不回显正文。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CollaborationValidationError {
    #[error("{label}不能为空")]
    EmptyText { label: &'static str },
    #[error("{label}超过 {max} bytes 上限")]
    TextTooLong { label: &'static str, max: usize },
    #[error("共享包没有入口")]
    NoArtifacts,
    #[error("共享入口数量超过 {max} 项")]
    TooManyArtifacts { max: usize },
    #[error("共享入口 ID 重复")]
    DuplicateArtifact,
    #[error("{kind}入口内容不能为空")]
    EmptyPayload { kind: CollaborationArtifactKind },
    #[error("{kind}入口内容超过 {max} bytes 上限")]
    PayloadTooLarge {
        kind: CollaborationArtifactKind,
        max: usize,
    },
    #[error("共享包总内容超过 {max} bytes 上限")]
    TotalPayloadTooLarge { max: usize },
    #[error("禁止把 {kind:?} 放入共享包")]
    ForbiddenDataClass { kind: CollaborationDataClass },
    #[error("{kind:?} 内容不能进入手动导出包")]
    RestrictedExport { kind: CollaborationDataClass },
    #[error("共享包 revision 无效")]
    InvalidRevision,
    #[error("共享包缺少审计记录")]
    MissingAudit,
    #[error("审计 revision 无效")]
    InvalidAuditRevision,
    #[error("审计事件超过 {max} 项上限")]
    TooManyAuditEvents { max: usize },
    #[error("共享包状态与同步策略不匹配")]
    InvalidStatePolicy,
    #[error("当前共享包不可导出")]
    NotExportable,
    #[error("序列化共享包失败：{0}")]
    Serialize(String),
}

fn validate_text(
    label: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), CollaborationValidationError> {
    if value.trim().is_empty() {
        return Err(CollaborationValidationError::EmptyText { label });
    }
    if value.len() > max_bytes {
        return Err(CollaborationValidationError::TextTooLong {
            label,
            max: max_bytes,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> CollaborationArtifact {
        CollaborationArtifact::document("README", "本机优先协作")
    }

    #[test]
    fn local_share_keeps_sensitive_data_local_and_exports_only_safe_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let sensitive = CollaborationArtifact {
            data_class: CollaborationDataClass::Sensitive,
            ..CollaborationArtifact::document("本机草稿", "内部结果")
        };
        let mut share = CollaborationShare::new_local("工作说明", vec![sensitive])?;
        assert!(share.manual_export_json().is_err());
        assert!(share.prepare_manual_export("alice").is_err());
        assert_eq!(share.state, CollaborationShareState::LocalDraft);
        Ok(())
    }

    #[test]
    fn manual_export_round_trip_and_revocation_are_audited()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut share = CollaborationShare::new_local("工作说明", vec![document()])?;
        let first_revision = share.revision;
        share.prepare_manual_export("alice")?;
        let encoded = share.manual_export_json()?;
        let decoded: CollaborationShare = serde_json::from_str(&encoded)?;
        assert_eq!(decoded.state, CollaborationShareState::Shared);
        assert!(decoded.revision > first_revision);

        share.revoke("alice")?;
        assert_eq!(share.state, CollaborationShareState::Revoked);
        assert!(share.manual_export_json().is_err());
        assert_eq!(
            share.audit.last().map(|event| event.action),
            Some(CollaborationAuditAction::Revoked)
        );
        Ok(())
    }

    #[test]
    fn stale_update_records_conflict_without_overwriting_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut share = CollaborationShare::new_local("旧标题", vec![document()])?;
        let revision = share.revision;
        share.update_local(revision, "新标题", vec![document()], "alice")?;
        let current_title = share.title.clone();
        let result = share.update_local(revision, "过期标题", vec![document()], "bob");
        assert!(matches!(
            result,
            Err(CollaborationMutationError::Conflict { .. })
        ));
        assert_eq!(share.title, current_title);
        Ok(())
    }

    #[test]
    fn credentials_and_connection_configs_are_always_rejected() {
        for data_class in [
            CollaborationDataClass::Credential,
            CollaborationDataClass::ConnectionConfig,
        ] {
            let artifact = CollaborationArtifact {
                data_class,
                ..document()
            };
            assert!(matches!(
                CollaborationShare::new_local("blocked", vec![artifact]),
                Err(CollaborationValidationError::ForbiddenDataClass { .. })
            ));
        }
    }
}
