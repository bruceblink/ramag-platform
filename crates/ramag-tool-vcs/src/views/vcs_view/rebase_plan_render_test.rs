use super::super::super::helpers::{ActiveView, FilesViewMode};
use super::add_vcs_window;
use crate::views::vcs_view::{CompareState, VcsView};
use gpui_kit::{TestAppContext, px, size};
use ramag_domain::entities::{
    FileChangeKind, FileStatus, RebaseAction, RebaseTodo, WorkingTreeStatus,
};

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
