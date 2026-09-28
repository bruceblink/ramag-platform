use super::*;

#[test]
fn workspace_rows_cache_requires_all_inputs_to_match() {
    let rows = Rc::new(Vec::new());
    let key = WorkspaceRowsCacheKey {
        status_request_seq: 11,
        files_identity: 17,
        files_len: 3,
        collapsed_version: 2,
        query: "src".into(),
    };
    let cache = WorkspaceRowsCacheEntry {
        key: key.clone(),
        rows: rows.clone(),
    };

    let cached = cache.get(&key);
    assert!(cached.is_some());
    if let Some(cached) = cached {
        assert!(Rc::ptr_eq(&cached, &rows));
    }

    let mut changed = key.clone();
    changed.status_request_seq += 1;
    assert!(cache.get(&changed).is_none());

    let mut changed = key.clone();
    changed.query = "tests".into();
    assert!(cache.get(&changed).is_none());

    changed = key;
    changed.collapsed_version += 1;
    assert!(cache.get(&changed).is_none());
}

#[test]
fn parent_directory_set_excludes_file_names() {
    let dirs = collect_parent_dirs(["src/ui/view.rs", "README.md"]);

    assert_eq!(
        dirs,
        HashSet::from(["src".to_string(), "src/ui".to_string()])
    );
}

#[test]
fn git_file_name_removes_tree_directory_prefix() {
    assert_eq!(git_file_name("crates/ramag-ui/src/lib.rs"), "lib.rs");
    assert_eq!(git_file_name("README.md"), "README.md");
}

#[test]
fn bulk_operation_paths_are_snapshotted_before_status_reorders() {
    let status = WorkingTreeStatus {
        files: vec![
            FileStatus {
                path: "src/first.rs".into(),
                old_path: None,
                staged: None,
                unstaged: Some(FileChangeKind::Modified),
            },
            FileStatus {
                path: "src/second.rs".into(),
                old_path: None,
                staged: None,
                unstaged: Some(FileChangeKind::Modified),
            },
        ],
        ..Default::default()
    };
    let indices = [0, 1, 1, 99];
    let snapshot = snapshot_file_paths(&status, &indices);

    let refreshed_status = WorkingTreeStatus {
        files: vec![
            FileStatus {
                path: "src/new.rs".into(),
                old_path: None,
                staged: None,
                unstaged: Some(FileChangeKind::Added),
            },
            status.files[1].clone(),
            status.files[0].clone(),
        ],
        ..Default::default()
    };

    assert_eq!(snapshot, ["src/first.rs", "src/second.rs"]);
    assert_ne!(
        snapshot,
        snapshot_file_paths(&refreshed_status, &indices),
        "刷新后的数组下标不应重算批量操作目标"
    );
}
