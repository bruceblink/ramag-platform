use super::super::super::helpers::ActiveView;
use super::add_vcs_window;
use crate::views::vcs_view::VcsView;
use gpui_kit::TestAppContext;
use ramag_domain::entities::{RebaseAction, RebaseTodo};

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
