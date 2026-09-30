use super::*;

#[test]
fn repo_session_cache_is_bounded_and_lru() {
    let mut cache = std::collections::HashMap::new();
    let mut order = std::collections::VecDeque::new();
    for index in 0..REPO_SESSION_CACHE_LIMIT {
        cache_repo_session(
            &mut cache,
            &mut order,
            format!("repo-{index}"),
            RepoSessionState::default(),
        );
    }
    cache_repo_session(
        &mut cache,
        &mut order,
        "repo-0".into(),
        RepoSessionState::default(),
    );
    cache_repo_session(
        &mut cache,
        &mut order,
        "repo-new".into(),
        RepoSessionState::default(),
    );

    assert_eq!(cache.len(), REPO_SESSION_CACHE_LIMIT);
    assert!(cache.contains_key("repo-0"));
    assert!(!cache.contains_key("repo-1"));
}

#[test]
fn repo_session_drops_loaded_file_payloads() {
    let mut tabs = vec![FileTab {
        path: "src/lib.rs".into(),
        source: FileTabSource::ProjectFiles,
        cached_diff: None,
        cached_diff_syntax: None,
        cached_content: Some(super::super::helpers::FileContentSnapshot {
            path: "src/lib.rs".into(),
            text: std::rc::Rc::new("content".into()),
            line_count: 1,
            revision: 0,
            dirty: false,
            truncated: false,
            binary: false,
            error: None,
        }),
    }];

    strip_file_tab_payloads(&mut tabs);

    assert!(tabs[0].cached_content.is_none());
    assert!(tabs[0].cached_diff.is_none());
}

#[test]
fn cached_session_tabs_restore_non_compare_target_or_neighbor() {
    let mut tabs = vec![
        FileTab {
            path: "before.rs".into(),
            source: FileTabSource::ProjectFiles,
            cached_diff: None,
            cached_diff_syntax: None,
            cached_content: None,
        },
        FileTab {
            path: "compare.rs".into(),
            source: FileTabSource::Compare {
                from: "from".into(),
                to: "to".into(),
            },
            cached_diff: None,
            cached_diff_syntax: None,
            cached_content: None,
        },
        FileTab {
            path: "after.rs".into(),
            source: FileTabSource::Commit {
                commit_id: "commit".into(),
                change_kind: None,
            },
            cached_diff: None,
            cached_diff_syntax: None,
            cached_content: None,
        },
    ];

    let active_after = prepare_cached_file_tabs(&mut tabs, Some(2));
    assert_eq!(active_after, Some(1));
    assert_eq!(
        tabs.iter().map(|tab| tab.path.as_str()).collect::<Vec<_>>(),
        vec!["before.rs", "after.rs"]
    );

    let mut compare_active = vec![
        FileTab {
            path: "before.rs".into(),
            source: FileTabSource::ProjectFiles,
            cached_diff: None,
            cached_diff_syntax: None,
            cached_content: None,
        },
        FileTab {
            path: "compare.rs".into(),
            source: FileTabSource::Compare {
                from: "from".into(),
                to: "to".into(),
            },
            cached_diff: None,
            cached_diff_syntax: None,
            cached_content: None,
        },
        FileTab {
            path: "after.rs".into(),
            source: FileTabSource::Commit {
                commit_id: "commit".into(),
                change_kind: None,
            },
            cached_diff: None,
            cached_diff_syntax: None,
            cached_content: None,
        },
    ];
    assert_eq!(
        prepare_cached_file_tabs(&mut compare_active, Some(1)),
        Some(1)
    );
    assert_eq!(compare_active[1].path, "after.rs");

    let mut compare_only = vec![FileTab {
        path: "compare.rs".into(),
        source: FileTabSource::Compare {
            from: "from".into(),
            to: "to".into(),
        },
        cached_diff: None,
        cached_diff_syntax: None,
        cached_content: None,
    }];
    assert_eq!(prepare_cached_file_tabs(&mut compare_only, Some(0)), None);
    assert!(compare_only.is_empty());
}

#[test]
fn completed_save_clears_only_the_matching_revision() {
    let mut tabs = vec![FileTab {
        path: "src/lib.rs".into(),
        source: FileTabSource::ProjectFiles,
        cached_diff: None,
        cached_diff_syntax: None,
        cached_content: Some(FileContentSnapshot {
            path: "src/lib.rs".into(),
            text: std::rc::Rc::new("new".into()),
            line_count: 1,
            revision: 2,
            dirty: true,
            truncated: false,
            binary: false,
            error: None,
        }),
    }];

    let stale = mark_project_file_revision_saved(&mut tabs, "src/lib.rs", 1);
    assert!(stale.is_some_and(|snapshot| snapshot.dirty));
    let current = mark_project_file_revision_saved(&mut tabs, "src/lib.rs", 2);
    assert!(current.is_some_and(|snapshot| !snapshot.dirty));
}
