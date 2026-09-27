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
fn manual_export_round_trip_and_revocation_are_audited() -> Result<(), Box<dyn std::error::Error>> {
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
fn manual_export_import_creates_a_new_local_draft() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = CollaborationShare::new_local("接口说明", vec![document()])?;
    source.prepare_manual_export("alice")?;
    let encoded = source.manual_export_json()?;
    let imported = CollaborationShare::import_manual_export_json(&encoded, "bob")?;
    assert_ne!(imported.id, source.id);
    assert_eq!(imported.state, CollaborationShareState::LocalDraft);
    assert_eq!(imported.sync_policy, CollaborationSyncPolicy::LocalOnly);
    assert_eq!(
        imported.audit.last().map(|event| event.action),
        Some(CollaborationAuditAction::Imported)
    );
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
