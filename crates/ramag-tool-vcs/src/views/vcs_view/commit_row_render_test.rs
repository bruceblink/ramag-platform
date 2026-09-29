use super::{add_vcs_window, mock_repo};
use crate::views::helpers::{ActiveView, stable_path_element_id};
use crate::views::vcs_view::VcsView;
use gpui_kit::{TestAppContext, px, size};
use ramag_domain::entities::{Commit, CommitId, Signature};
use std::rc::Rc;

/// 构造带完整提交 ID 和作者信息的测试提交，供渲染回归复用。
fn history_commit(id: &str, subject: &str) -> Commit {
    let signature = Signature {
        name: "Author".into(),
        email: "author@example.test".into(),
        timestamp: chrono::Utc::now(),
    };
    Commit {
        id: CommitId(id.into()),
        parents: Vec::new(),
        author: signature.clone(),
        committer: signature,
        subject: subject.into(),
        body: String::new(),
        refs: Vec::new(),
    }
}

/// 把两条相同前缀的提交注入历史面板，覆盖窄窗口布局和节点定位。
fn inject_history_session(v: &mut VcsView) {
    let repo = mock_repo();
    v.open_repos = vec![repo.clone()];
    v.repo = Some(repo);
    v.active_view = ActiveView::Session;
    v.history_pane_visible = true;
    let first = Rc::new(history_commit(
        "aaaaaaaaaaaa1111111111111111111111111111",
        "first commit with a long subject that should remain readable",
    ));
    let second = Rc::new(history_commit(
        "aaaaaaaaaaaa2222222222222222222222222222",
        "second commit",
    ));
    v.history_commits = Rc::new(vec![first, second]);
    v.history_graph_rows = Rc::new(vec![
        crate::views::commit_graph::CommitGraphRow {
            lane: 0,
            total_lanes: 1,
            is_merge: false,
        },
        crate::views::commit_graph::CommitGraphRow {
            lane: 0,
            total_lanes: 1,
            is_merge: false,
        },
    ]);
    v.history_has_more = false;
    v.loading_history = false;
}

/// 提交历史行在窄窗口中保持在内容区内，且相同前缀的提交仍可单独定位。
#[gpui_kit::test]
fn history_commit_rows_fit_narrow_content_and_keep_stable_selectors(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    view.update(cx, |view, cx| {
        inject_history_session(view);
        cx.notify();
    });

    for (width, height) in [(360.0, 640.0), (720.0, 640.0), (1440.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        cx.run_until_parked();

        let content = cx
            .debug_bounds("vcs-history-content")
            .expect("历史内容区应参与布局");
        for commit in [
            "aaaaaaaaaaaa1111111111111111111111111111",
            "aaaaaaaaaaaa2222222222222222222222222222",
        ] {
            let selector = stable_path_element_id("commit-row", commit);
            let selector: &'static str = Box::leak(selector.to_string().into_boxed_str());
            let row = cx
                .debug_bounds(selector)
                .expect("提交行应可按稳定选择器定位");
            assert!(
                row.left() >= content.left() && row.right() <= content.right(),
                "提交行不能超出历史内容区: row={row:?}, content={content:?}"
            );
        }
    }
}
