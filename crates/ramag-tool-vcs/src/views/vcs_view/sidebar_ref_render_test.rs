#![allow(clippy::expect_used)]

use super::{add_vcs_window, inject_diff_session};
use crate::views::helpers::{
    stable_branch_element_id, stable_path_element_id, stable_remote_element_id,
    stable_tag_element_id,
};
use gpui_kit::{TestAppContext, px, size};
use ramag_domain::entities::{Branch, BranchKind, Remote, Tag, TagKind};

/// 侧栏引用名称包含路径字符或空格时，重排后仍能按稳定选择器定位目标行。
#[gpui_kit::test]
fn history_ref_rows_use_safe_stable_selectors_for_named_refs(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    view.update(cx, |view, cx| {
        inject_diff_session(view);
        view.history_pane_visible = true;
        view.collapsed_remote = false;
        view.collapsed_remote_repos = false;
        view.collapsed_tag = false;
        let branch = |name: &str, kind: BranchKind, commit: &str| Branch {
            name: name.into(),
            kind,
            commit: ramag_domain::entities::CommitId(commit.into()),
            is_head: false,
            upstream: None,
            ahead: None,
            behind: None,
        };
        view.local_branches = vec![
            branch("feature/ui", BranchKind::Local, "local-feature"),
            branch("main", BranchKind::Local, "local-main"),
        ];
        view.remote_branches = vec![
            branch("feature ui", BranchKind::Remote, "remote-feature"),
            branch("origin/main", BranchKind::Remote, "remote-main"),
        ];
        view.remotes = vec![
            Remote {
                name: "origin/team".into(),
                fetch_url: "https://example.test/repo.git".into(),
                push_url: None,
            },
            Remote {
                name: "upstream".into(),
                fetch_url: "https://example.test/upstream.git".into(),
                push_url: None,
            },
        ];
        view.tags = vec![
            Tag {
                name: "release v1".into(),
                kind: TagKind::Lightweight,
                commit: ramag_domain::entities::CommitId("tag-release".into()),
                message: None,
                tagger: None,
            },
            Tag {
                name: "v0".into(),
                kind: TagKind::Lightweight,
                commit: ramag_domain::entities::CommitId("tag-v0".into()),
                message: None,
                tagger: None,
            },
        ];
        cx.notify();
    });
    cx.simulate_resize(size(px(1200.0), px(1200.0)));
    cx.run_until_parked();
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        assert_eq!(view.local_branches.len(), 2);
        assert_eq!(view.remote_branches.len(), 2);
        assert_eq!(view.remotes.len(), 2);
        assert_eq!(view.tags.len(), 2);
        assert!(!view.collapsed_local);
        assert!(!view.collapsed_remote);
        assert!(!view.collapsed_remote_repos);
        assert!(!view.collapsed_tag);
    });

    assert!(
        cx.debug_bounds("vcs-history-left-column").is_some(),
        "历史左栏应参与布局"
    );
    let list = cx
        .debug_bounds("vcs-history-left-rows")
        .expect("历史左栏列表应参与布局");
    assert!(list.size.width > px(0.0), "历史左栏列表不能没有宽度");
    assert!(list.size.height > px(0.0), "历史左栏列表不能没有高度");

    let selectors = [
        stable_branch_element_id(false, "feature/ui"),
        stable_branch_element_id(true, "feature ui"),
        stable_remote_element_id("origin/team"),
        stable_tag_element_id("release v1"),
        stable_path_element_id("side-branch-more", "false:feature/ui"),
        stable_path_element_id("side-remote-more", "origin/team"),
        stable_path_element_id("side-tag-more", "release v1"),
    ];
    let selectors: Vec<&'static str> = selectors
        .into_iter()
        .map(|selector| &*Box::leak(selector.to_string().into_boxed_str()))
        .collect();
    for selector in &selectors {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "历史侧栏目标应可按稳定选择器定位: {selector}"
        );
    }

    view.update(cx, |view, cx| {
        view.local_branches = view.local_branches.iter().rev().cloned().collect();
        view.remote_branches = view.remote_branches.iter().rev().cloned().collect();
        view.remotes = view.remotes.iter().rev().cloned().collect();
        view.tags = view.tags.iter().rev().cloned().collect();
        cx.notify();
    });
    cx.run_until_parked();

    for selector in &selectors {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "重排后历史侧栏目标应可按稳定选择器定位: {selector}"
        );
    }
}
