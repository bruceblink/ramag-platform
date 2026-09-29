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

fn unified_diff() -> FileDiff {
    FileDiff {
        path: "a.rs".into(),
        old_path: None,
        change_kind: ramag_domain::entities::FileChangeKind::Modified,
        binary: false,
        old_mode: None,
        new_mode: None,
        hunks: vec![Hunk {
            old_start: 0,
            old_lines: 0,
            new_start: 1,
            new_lines: 1,
            heading: None,
            lines: vec![DiffLine {
                kind: DiffLineKind::Add,
                old_lineno: None,
                new_lineno: Some(1),
                text: "added".into(),
            }],
        }],
    }
}

fn spacer_hunk(start: u32, marker: &str) -> Hunk {
    let mut lines = (0..8)
        .map(|index| DiffLine {
            kind: DiffLineKind::Context,
            old_lineno: Some(start + index),
            new_lineno: Some(start + index),
            text: format!("{marker}-{index}"),
        })
        .collect::<Vec<_>>();
    lines.push(DiffLine {
        kind: DiffLineKind::Add,
        old_lineno: None,
        new_lineno: Some(start + 8),
        text: marker.into(),
    });
    Hunk {
        old_start: start,
        old_lines: 8,
        new_start: start,
        new_lines: 9,
        heading: Some(marker.into()),
        lines,
    }
}

fn spacer_diff() -> FileDiff {
    FileDiff {
        path: "a.rs".into(),
        old_path: None,
        change_kind: ramag_domain::entities::FileChangeKind::Modified,
        binary: false,
        old_mode: None,
        new_mode: None,
        hunks: vec![spacer_hunk(1, "first"), spacer_hunk(20, "second")],
    }
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
    let split_line_id =
        crate::views::helpers::stable_diff_line_element_id("diff-gutter", &initial, 0, 0);
    let split_line_selector: &'static str =
        Box::leak(format!("L-{split_line_id}").into_boxed_str());
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
    assert!(cx.debug_bounds(split_line_selector).is_some());

    let mut reordered = initial;
    reordered.hunks.reverse();
    view.update(cx, |view, cx| {
        install_diff(view, reordered);
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds(stage_selector).is_some());
    assert!(cx.debug_bounds(discard_selector).is_some());
    assert!(cx.debug_bounds(split_line_selector).is_some());

    let unified = unified_diff();
    let unified_line_id =
        crate::views::helpers::stable_diff_line_element_id("diff-line", &unified, 0, 0);
    let unified_line_selector: &'static str =
        Box::leak(unified_line_id.to_string().into_boxed_str());
    view.update(cx, |view, cx| {
        install_diff(view, unified);
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds(unified_line_selector).is_some());
}

/// 长 Context 的折叠占位行在 hunk 顺序变化后继续绑定原内容，展开状态也不转移到另一段。
#[gpui_kit::test]
fn diff_spacer_keeps_stable_selector_and_expansion_after_reorder(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    let initial = spacer_diff();
    let first_key = crate::views::helpers::stable_diff_spacer_key(&initial, 0, 0);
    let second_key = crate::views::helpers::stable_diff_spacer_key(&initial, 1, 0);
    let first_selector: &'static str =
        Box::leak(format!("vcs-diff-spacer-L-{first_key:016x}").into_boxed_str());
    let second_selector: &'static str =
        Box::leak(format!("vcs-diff-spacer-L-{second_key:016x}").into_boxed_str());

    view.update(cx, |view, cx| {
        inject_diff_session(view);
        install_diff(view, initial.clone());
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds(first_selector).is_some());
    assert!(cx.debug_bounds(second_selector).is_some());

    let mut reordered = initial;
    reordered.hunks.reverse();
    view.update(cx, |view, cx| {
        view.expanded_diff_spacers.insert(first_key);
        install_diff(view, reordered);
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(first_selector).is_none(),
        "原 hunk 的占位行展开后不应继续显示"
    );
    assert!(
        cx.debug_bounds(second_selector).is_some(),
        "另一 hunk 的占位行不应被原展开状态吞掉"
    );
}
