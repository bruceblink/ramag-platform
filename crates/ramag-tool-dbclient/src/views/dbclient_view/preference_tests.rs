use super::*;

#[test]
fn open_sessions_parser_accepts_new_and_legacy_formats() {
    let id = ramag_domain::entities::ConnectionId::new();
    let modern = serde_json::to_string(&OpenSessionsPref {
        ids: vec![id.clone()],
        active: Some(id.clone()),
    })
    .unwrap_or_default();
    let legacy = serde_json::to_string(&vec![id.clone()]).unwrap_or_default();

    assert!(matches!(
        parse_open_sessions(&modern),
        Ok((pref, false)) if pref.ids == vec![id.clone()] && pref.active == Some(id.clone())
    ));
    assert!(matches!(
        parse_open_sessions(&legacy),
        Ok((pref, false)) if pref.ids == vec![id] && pref.active.is_none()
    ));
    assert!(parse_open_sessions("not-json").is_err());
}

#[test]
fn open_sessions_parser_bounds_and_deduplicates_restore_data() {
    let first = ramag_domain::entities::ConnectionId::new();
    let mut ids = vec![first.clone(), first.clone()];
    ids.extend((0..MAX_CONNECTION_SESSIONS).map(|_| ramag_domain::entities::ConnectionId::new()));
    let json = serde_json::to_string(&OpenSessionsPref {
        ids,
        active: Some(first.clone()),
    })
    .unwrap_or_default();

    assert!(matches!(
        parse_open_sessions(&json),
        Ok((pref, true))
            if pref.ids.len() == MAX_CONNECTION_SESSIONS
                && pref.ids.first() == Some(&first)
                && pref.active == Some(first)
    ));
    assert!(parse_open_sessions(&" ".repeat(MAX_OPEN_SESSIONS_PREF_BYTES + 1)).is_err());
}

#[test]
fn restored_sessions_are_queued_without_materializing_inactive_tabs() {
    let first =
        ramag_domain::entities::ConnectionConfig::new_mysql("first", "127.0.0.1", 3306, "root");
    let mut duplicate = first.clone();
    duplicate.name = "duplicate-id".into();
    let second =
        ramag_domain::entities::ConnectionConfig::new_mysql("second", "127.0.0.1", 3307, "root");
    let active_id = second.id.clone();
    let mut sessions = Vec::new();

    let active = queue_restored_session_slots(
        &mut sessions,
        vec![first, duplicate, second],
        Some(&active_id),
    );

    assert_eq!(active, Some(1));
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].config.name, "first");
    assert_eq!(sessions[1].config.name, "second");
    assert!(sessions.iter().all(|session| session.entity.is_none()));
}

#[test]
fn restored_sessions_keep_the_existing_list_and_fall_back_when_active_is_trimmed() {
    let existing =
        ramag_domain::entities::ConnectionConfig::new_mysql("existing", "127.0.0.1", 3305, "root");
    let configs: Vec<_> = (0..(MAX_CONNECTION_SESSIONS + 2))
        .map(|index| {
            ramag_domain::entities::ConnectionConfig::new_mysql(
                format!("connection-{index}"),
                "127.0.0.1",
                3306,
                "root",
            )
        })
        .collect();
    let trimmed_active_id = configs[MAX_CONNECTION_SESSIONS + 1].id.clone();
    let mut sessions = vec![SessionSlot {
        entity: None,
        config: existing,
        stale: false,
    }];

    let active = queue_restored_session_slots(&mut sessions, configs, Some(&trimmed_active_id));

    assert_eq!(active, Some(0));
    assert_eq!(sessions.len(), MAX_CONNECTION_SESSIONS);
    assert_eq!(sessions[0].config.name, "existing");
    assert!(sessions.iter().all(|session| session.entity.is_none()));
}
