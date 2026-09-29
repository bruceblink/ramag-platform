#![allow(clippy::expect_used)]

use super::super::{add_vcs_window, inject_diff_session, test_diff};
use gpui_kit::TestAppContext;
use ramag_domain::entities::{DiffLine, DiffLineKind, FileDiff, Hunk};
use std::rc::Rc;

fn second_hunk() -> Hunk {
    Hunk {
        old_start: 20,
        old_lines: 3,
        new_start: 20,
        new_lines: 3,
        heading: Some("second".into()),
        lines: vec![
            DiffLine {
                kind: DiffLineKind::Context,
                old_lineno: Some(20),
                new_lineno: Some(20),
                text: "before".into(),
            },
            DiffLine {
                kind: DiffLineKind::Delete,
                old_lineno: Some(21),
                new_lineno: None,
                text: "old".into(),
            },
            DiffLine {
                kind: DiffLineKind::Add,
                old_lineno: None,
                new_lineno: Some(21),
                text: "new".into(),
            },
        ],
    }
}

fn two_hunk_diff() -> FileDiff {
    let mut diff = test_diff();
    diff.hunks.push(second_hunk());
    diff
}

fn install_diff(view: &mut super::super::VcsView, diff: FileDiff) {
    let diff = Rc::new(diff);
    view.current_diff = Some(diff.clone());
    view.current_diff_syntax = None;
    view.file_tabs[0].cached_diff = Some(diff);
    view.file_tabs[0].cached_diff_syntax = None;
}

/// Diff 重新加载并改变 hunk 顺序后，原片段按钮仍绑定原 hunk 内容。
#[gpui_kit::test]
fn diff_hunk_buttons_keep_stable_selectors_after_reorder(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    let initial = two_hunk_diff();
    let first_key =
        crate::views::helpers::stable_hunk_key(&initial, 0).expect("第一个 hunk 应有稳定键");
    view.update(cx, |view, cx| {
        inject_diff_session(view);
        install_diff(view, initial.clone());
        cx.notify();
    });
    cx.run_until_parked();

    let stage_selector: &'static str =
        Box::leak(format!("vcs-hunk-stage-{first_key}").into_boxed_str());
    let discard_selector: &'static str =
        Box::leak(format!("vcs-hunk-discard-{first_key}").into_boxed_str());
    assert!(cx.debug_bounds(stage_selector).is_some());
    assert!(cx.debug_bounds(discard_selector).is_some());

    let mut reordered = initial;
    reordered.hunks.reverse();
    view.update(cx, |view, cx| {
        install_diff(view, reordered);
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds(stage_selector).is_some());
    assert!(cx.debug_bounds(discard_selector).is_some());
}
