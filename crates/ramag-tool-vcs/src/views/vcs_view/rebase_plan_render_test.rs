use super::super::super::helpers::{ActiveView, FileTab, FileTabSource, FilesViewMode, GroupKind};
use super::add_vcs_window;
use crate::views::vcs_view::{CompareState, VcsView};
use gpui_kit::{TestAppContext, px, size};
use ramag_domain::entities::{
    Commit, CommitId, FileChangeKind, FileStatus, RebaseAction, RebaseTodo, ReflogEntry, Signature,
    WorkingTreeStatus,
};
use std::rc::Rc;

fn inject_rebase_plan(v: &mut VcsView) {
    let repo = super::mock_repo();
    v.open_repos = vec![repo.clone()];
    v.repo = Some(repo);
    v.active_view = ActiveView::Session;
    v.show_rebase_plan = true;
    v.rebase_plan_onto = "main".into();
    v.rebase_todos = vec![
        RebaseTodo {
            action: RebaseAction::Pick,
            hash: "a".repeat(40),
            subject: "first".into(),
        },
        RebaseTodo {
            action: RebaseAction::Pick,
            hash: "b".repeat(40),
            subject: "second".into(),
        },
    ];
}

/// Rebase 行的操作按钮使用提交 ID；计划重排后重新绘制，旧行不会复用成另一条提交。
#[gpui_kit::test]
fn vcs_view_renders_rebase_plan_after_todo_reorder(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);

    view.update(cx, |v, cx| {
        inject_rebase_plan(v);
        cx.notify();
    });
    cx.run_until_parked();

    view.update(cx, |v, cx| {
        v.rebase_todos.swap(0, 1);
        cx.notify();
    });
    cx.run_until_parked();

    view.read_with(cx, |v, _| {
        assert_eq!(v.rebase_todos[0].hash, "b".repeat(40));
        assert_eq!(v.rebase_todos[1].hash, "a".repeat(40));
    });
}

fn inject_workspace_changes(v: &mut VcsView) {
    let repo = super::mock_repo();
    v.open_repos = vec![repo.clone()];
    v.repo = Some(repo);
    v.active_view = ActiveView::Session;
    v.files_view_mode = FilesViewMode::Changes;
    v.status = Some(WorkingTreeStatus {
        files: vec![
            FileStatus {
                path: "src/lib.rs".into(),
                old_path: None,
                staged: Some(FileChangeKind::Modified),
                unstaged: Some(FileChangeKind::Modified),
            },
            FileStatus {
                path: "src/main.rs".into(),
                old_path: None,
                staged: None,
                unstaged: Some(FileChangeKind::Added),
            },
        ],
        ..Default::default()
    });
}

/// 工作区变更行按路径和分组保持标识；状态顺序改变后在常见窗口宽度下重新渲染不 panic。
#[gpui_kit::test]
fn vcs_view_renders_workspace_rows_after_status_reorder(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);

    view.update(cx, |v, cx| {
        inject_workspace_changes(v);
        cx.notify();
    });
    cx.run_until_parked();

    for (width, height) in [(360.0, 640.0), (720.0, 640.0), (1200.0, 800.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        view.update(cx, |v, cx| {
            v.status.as_mut().expect("workspace status").files.reverse();
            v.status_request_seq = v.status_request_seq.wrapping_add(1);
            v.changes_rows_cache.get_mut().take();
            cx.notify();
        });
        cx.run_until_parked();
    }

    view.read_with(cx, |v, _| {
        assert_eq!(
            v.status.as_ref().expect("workspace status").files[0].path,
            "src/main.rs"
        );
    });
}

fn inject_compare_files(v: &mut VcsView) {
    let repo = super::mock_repo();
    v.open_repos = vec![repo.clone()];
    v.repo = Some(repo);
    v.active_view = ActiveView::Session;
    v.files_view_mode = FilesViewMode::Changes;
    v.compare = Some(CompareState {
        from: "from-commit".into(),
        to: "to-commit".into(),
        from_label: "from".into(),
        to_label: "to".into(),
        files: std::rc::Rc::new(vec![
            FileStatus {
                path: "src/lib.rs".into(),
                old_path: None,
                staged: Some(FileChangeKind::Modified),
                unstaged: None,
            },
            FileStatus {
                path: "src/main.rs".into(),
                old_path: None,
                staged: Some(FileChangeKind::Added),
                unstaged: None,
            },
        ]),
        loading: false,
    });
}

/// 分支比较文件行按比较范围和路径保持标识；列表重排后在三种窗口尺寸下重新渲染不 panic。
#[gpui_kit::test]
fn vcs_view_renders_compare_rows_after_file_reorder(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);

    view.update(cx, |v, cx| {
        inject_compare_files(v);
        cx.notify();
    });
    cx.run_until_parked();

    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        view.update(cx, |v, cx| {
            let compare = v.compare.as_mut().expect("compare state");
            let mut files = compare.files.as_ref().clone();
            files.reverse();
            compare.files = std::rc::Rc::new(files);
            cx.notify();
        });
        cx.run_until_parked();
    }

    view.read_with(cx, |v, _| {
        assert_eq!(
            v.compare.as_ref().expect("compare state").files[0].path,
            "src/main.rs"
        );
    });
}

fn compare_state() -> CompareState {
    CompareState {
        from: "from-commit".into(),
        to: "to-commit".into(),
        from_label: "from".into(),
        to_label: "to".into(),
        files: Rc::new(Vec::new()),
        loading: false,
    }
}

fn file_tab(path: &str, source: FileTabSource) -> FileTab {
    FileTab {
        path: path.into(),
        source,
        cached_diff: None,
        cached_diff_syntax: None,
        cached_content: None,
    }
}

/// 关闭比较后按稳定目标恢复非比较标签；当前标签属于比较范围时选择相邻标签。
#[gpui_kit::test]
fn closing_compare_restores_stable_file_tab_selection(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);

    view.update(cx, |v, _| {
        v.file_tabs = vec![
            file_tab("before.rs", FileTabSource::Changes(GroupKind::Unstaged)),
            file_tab(
                "compare.rs",
                FileTabSource::Compare {
                    from: "from-commit".into(),
                    to: "to-commit".into(),
                },
            ),
            file_tab("after.rs", FileTabSource::ProjectFiles),
        ];
        v.active_file_tab_idx = Some(2);
        v.compare = Some(compare_state());
        v.clear_compare_state();

        assert_eq!(v.active_file_tab_idx, Some(1));
        assert_eq!(v.file_tabs[1].path, "after.rs");
        assert_eq!(v.selected_pf_path.as_deref(), Some("after.rs"));

        v.file_tabs = vec![
            file_tab("before.rs", FileTabSource::Changes(GroupKind::Unstaged)),
            file_tab(
                "compare.rs",
                FileTabSource::Compare {
                    from: "from-commit".into(),
                    to: "to-commit".into(),
                },
            ),
            file_tab("after.rs", FileTabSource::ProjectFiles),
        ];
        v.active_file_tab_idx = Some(1);
        v.selected_pf_path = None;
        v.compare = Some(compare_state());
        v.clear_compare_state();

        assert_eq!(v.active_file_tab_idx, Some(1));
        assert_eq!(v.file_tabs[1].path, "after.rs");
        assert_eq!(v.selected_pf_path.as_deref(), Some("after.rs"));

        v.file_tabs = vec![file_tab(
            "compare.rs",
            FileTabSource::Compare {
                from: "from-commit".into(),
                to: "to-commit".into(),
            },
        )];
        v.active_file_tab_idx = Some(0);
        v.selected_pf_path = None;
        v.compare = Some(compare_state());
        v.clear_compare_state();

        assert!(v.file_tabs.is_empty());
        assert!(v.active_file_tab_idx.is_none());
        assert!(v.selected_file.is_none());
        assert!(v.selected_pf_path.is_none());
        assert!(v.current_diff.is_none());
    });
}

/// 状态刷新移除 Changes 标签时按清理前位置恢复相邻标签，并保留重新归类后的标签。
#[gpui_kit::test]
fn status_refresh_restores_file_tab_position_after_change_removal(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    let repo = super::mock_repo();

    view.update(cx, |v, cx| {
        v.open_repos = vec![repo.clone()];
        v.repo = Some(repo.clone());
        v.status = Some(WorkingTreeStatus {
            files: vec![
                FileStatus {
                    path: "before.rs".into(),
                    old_path: None,
                    staged: None,
                    unstaged: Some(FileChangeKind::Modified),
                },
                FileStatus {
                    path: "after.rs".into(),
                    old_path: None,
                    staged: None,
                    unstaged: Some(FileChangeKind::Modified),
                },
            ],
            ..Default::default()
        });
        v.file_tabs = vec![
            file_tab("before.rs", FileTabSource::Changes(GroupKind::Unstaged)),
            file_tab("gone.rs", FileTabSource::Changes(GroupKind::Unstaged)),
            file_tab("project.rs", FileTabSource::ProjectFiles),
            file_tab(
                "commit.rs",
                FileTabSource::Commit {
                    commit_id: "commit-id".into(),
                    change_kind: None,
                },
            ),
        ];
        v.active_file_tab_idx = Some(1);
        v.sync_changes_tabs_with_status(cx);

        assert_eq!(v.active_file_tab_idx, Some(1));
        assert_eq!(v.file_tabs[1].path, "project.rs");
        assert_eq!(v.selected_pf_path.as_deref(), Some("project.rs"));

        v.status = Some(WorkingTreeStatus {
            files: vec![FileStatus {
                path: "redirected.rs".into(),
                old_path: None,
                staged: Some(FileChangeKind::Modified),
                unstaged: None,
            }],
            ..Default::default()
        });
        let mut redirected = file_tab("redirected.rs", FileTabSource::Changes(GroupKind::Unstaged));
        redirected.cached_diff = Some(Rc::new(super::test_diff()));
        v.file_tabs = vec![
            file_tab("before.rs", FileTabSource::ProjectFiles),
            redirected,
            file_tab(
                "after.rs",
                FileTabSource::Commit {
                    commit_id: "commit-id".into(),
                    change_kind: None,
                },
            ),
        ];
        v.active_file_tab_idx = Some(1);
        v.selected_pf_path = Some("before.rs".into());
        v.sync_changes_tabs_with_status(cx);

        assert_eq!(v.active_file_tab_idx, Some(1));
        assert_eq!(v.file_tabs[1].path, "redirected.rs");
        assert_eq!(
            v.file_tabs[1].source,
            FileTabSource::Changes(GroupKind::Staged)
        );
        assert_eq!(
            v.selected_file,
            Some(("redirected.rs".into(), GroupKind::Staged))
        );

        v.status = Some(WorkingTreeStatus {
            files: vec![FileStatus {
                path: "active.rs".into(),
                old_path: None,
                staged: None,
                unstaged: Some(FileChangeKind::Modified),
            }],
            ..Default::default()
        });
        v.file_tabs = vec![
            file_tab(
                "removed-before.rs",
                FileTabSource::Changes(GroupKind::Unstaged),
            ),
            file_tab("active.rs", FileTabSource::ProjectFiles),
            file_tab(
                "last.rs",
                FileTabSource::Commit {
                    commit_id: "commit-id".into(),
                    change_kind: None,
                },
            ),
        ];
        v.active_file_tab_idx = Some(1);
        v.selected_pf_path = Some("active.rs".into());
        v.sync_changes_tabs_with_status(cx);

        assert_eq!(v.active_file_tab_idx, Some(0));
        assert_eq!(v.file_tabs[0].path, "active.rs");
        assert_eq!(v.selected_pf_path.as_deref(), Some("active.rs"));
    });
}

/// 仓库会话缓存移除 Compare 标签后，恢复流程仍能激活原来的非比较标签。
#[gpui_kit::test]
fn restoring_repo_session_keeps_non_compare_file_tab_active(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    let repo = super::mock_repo();

    view.update(cx, |v, cx| {
        v.open_repos = vec![repo.clone()];
        v.repo = Some(repo.clone());
        v.file_tabs = vec![
            file_tab("before.rs", FileTabSource::ProjectFiles),
            file_tab(
                "compare.rs",
                FileTabSource::Compare {
                    from: "from-commit".into(),
                    to: "to-commit".into(),
                },
            ),
            file_tab(
                "after.rs",
                FileTabSource::Commit {
                    commit_id: "commit-id".into(),
                    change_kind: None,
                },
            ),
        ];
        v.active_file_tab_idx = Some(2);
        v.save_current_session_to_cache(cx);

        let cached = v
            .repo_session_cache
            .get(&repo.path)
            .expect("仓库会话应已缓存");
        assert_eq!(cached.active_file_tab_idx, Some(1));
        assert_eq!(cached.file_tabs.len(), 2);
        assert_eq!(cached.file_tabs[1].path, "after.rs");

        v.file_tabs.clear();
        v.active_file_tab_idx = None;
        assert!(v.restore_session_from_cache(&repo.path, cx));
        assert_eq!(v.active_file_tab_idx, Some(1));
        assert_eq!(v.selected_commit_file.as_deref(), Some("after.rs"));
        assert_eq!(v.file_tabs[1].path, "after.rs");
    });
    cx.run_until_parked();
}

fn inject_project_files(v: &mut VcsView) {
    let repo = super::mock_repo();
    v.open_repos = vec![repo.clone()];
    v.repo = Some(repo);
    v.active_view = ActiveView::Session;
    v.files_view_mode = FilesViewMode::Project;
    v.project_files = vec![
        "README.md".into(),
        "src/lib.rs".into(),
        "src/main.rs".into(),
    ];
    v.project_files_version = 1;
    v.project_expanded_dirs = std::collections::HashSet::from(["src".into()]);
    v.project_expanded_dirs_version = 1;
    v.status = Some(WorkingTreeStatus {
        files: vec![FileStatus {
            path: "src/lib.rs".into(),
            old_path: None,
            staged: None,
            unstaged: Some(FileChangeKind::Modified),
        }],
        ..Default::default()
    });
}

/// Project Files 的缓存行保存路径；源路径数组重排后，旧行仍能在三种窗口尺寸下安全重绘。
#[gpui_kit::test]
fn vcs_view_renders_project_rows_after_source_reorder(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);

    view.update(cx, |v, cx| {
        inject_project_files(v);
        cx.notify();
    });
    cx.run_until_parked();

    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        view.update(cx, |v, cx| {
            v.project_files.reverse();
            cx.notify();
        });
        cx.run_until_parked();
    }

    view.read_with(cx, |v, _| {
        assert_eq!(
            v.project_files.first().map(String::as_str),
            Some("src/main.rs")
        );
    });
}

fn inject_commit_detail(v: &mut VcsView) {
    let repo = super::mock_repo();
    let signature = Signature {
        name: "Ramag Test".into(),
        email: "ramag@example.test".into(),
        timestamp: chrono::Utc::now(),
    };
    v.open_repos = vec![repo.clone()];
    v.repo = Some(repo);
    v.active_view = ActiveView::Session;
    v.history_pane_visible = true;
    v.viewing_commit = Some(Rc::new(Commit {
        id: CommitId("commit-detail-test".into()),
        parents: Vec::new(),
        author: signature.clone(),
        committer: signature,
        subject: "commit detail".into(),
        body: String::new(),
        refs: Vec::new(),
    }));
    v.commit_files = Rc::new(vec![
        FileStatus {
            path: "src/lib.rs".into(),
            old_path: None,
            staged: Some(FileChangeKind::Modified),
            unstaged: None,
        },
        FileStatus {
            path: "src/main.rs".into(),
            old_path: None,
            staged: Some(FileChangeKind::Added),
            unstaged: None,
        },
    ]);
}

/// 提交详情文件树按提交和路径保持行标识；文件列表重排后在三种窗口尺寸下重新渲染。
#[gpui_kit::test]
fn vcs_view_renders_commit_detail_rows_after_file_reorder(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);

    view.update(cx, |v, cx| {
        inject_commit_detail(v);
        cx.notify();
    });
    cx.run_until_parked();

    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        view.update(cx, |v, cx| {
            let mut files = v.commit_files.as_ref().clone();
            files.reverse();
            v.commit_files = Rc::new(files);
            cx.notify();
        });
        cx.run_until_parked();
    }

    view.read_with(cx, |v, _| {
        assert_eq!(
            v.commit_files.first().map(|file| file.path.as_str()),
            Some("src/main.rs")
        );
    });
}

fn inject_reflog(v: &mut VcsView) {
    let repo = super::mock_repo();
    v.open_repos = vec![repo.clone()];
    v.repo = Some(repo);
    v.active_view = ActiveView::Session;
    v.history_pane_visible = true;
    v.showing_reflog = true;
    v.reflog_entries = Rc::new(vec![
        ReflogEntry {
            commit: CommitId("commit-a".into()),
            selector: "HEAD@{0}".into(),
            action: "commit".into(),
            subject: "first reflog entry".into(),
            timestamp: chrono::Utc::now(),
        },
        ReflogEntry {
            commit: CommitId("commit-b".into()),
            selector: "HEAD@{1}".into(),
            action: "checkout".into(),
            subject: "second reflog entry".into(),
            timestamp: chrono::Utc::now(),
        },
    ]);
}

/// Reflog 行按记录字段保持标识；列表重排后在三种窗口尺寸下重新渲染不 panic。
#[gpui_kit::test]
fn vcs_view_renders_reflog_rows_after_entry_reorder(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);

    view.update(cx, |v, cx| {
        inject_reflog(v);
        cx.notify();
    });
    cx.run_until_parked();

    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        view.update(cx, |v, cx| {
            let mut entries = v.reflog_entries.as_ref().clone();
            entries.reverse();
            v.reflog_entries = Rc::new(entries);
            cx.notify();
        });
        cx.run_until_parked();
    }

    view.read_with(cx, |v, _| {
        assert_eq!(
            v.reflog_entries
                .first()
                .map(|entry| entry.commit.0.as_str()),
            Some("commit-b")
        );
    });
}
