use gpui_kit::SharedString;
use ramag_domain::entities::{DiffLineKind, FileDiff};

use super::GroupKind;

pub(crate) fn stable_path_element_id(prefix: &str, path: &str) -> SharedString {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    prefix.hash(&mut hasher);
    path.hash(&mut hasher);
    SharedString::from(format!("vcs-{prefix}-{:016x}", hasher.finish()))
}

pub(crate) fn stable_branch_element_id(is_remote: bool, name: &str) -> SharedString {
    stable_path_element_id("side-branch", &format!("{is_remote}:{name}"))
}

pub(crate) fn stable_remote_element_id(name: &str) -> SharedString {
    stable_path_element_id("side-remote", name)
}

pub(crate) fn stable_tag_element_id(name: &str) -> SharedString {
    stable_path_element_id("side-tag", name)
}

/// 为当前文件 Diff 中的 hunk 生成与位置无关的稳定键。
pub(crate) fn stable_hunk_key(diff: &FileDiff, hunk_idx: usize) -> Option<String> {
    use std::hash::{Hash, Hasher};

    let hunk = diff.hunks.get(hunk_idx)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "diff-hunk".hash(&mut hasher);
    diff.path.hash(&mut hasher);
    diff.old_path.hash(&mut hasher);
    hunk.old_start.hash(&mut hasher);
    hunk.old_lines.hash(&mut hasher);
    hunk.new_start.hash(&mut hasher);
    hunk.new_lines.hash(&mut hasher);
    hunk.heading.hash(&mut hasher);
    for line in &hunk.lines {
        match line.kind {
            DiffLineKind::Context => 0u8,
            DiffLineKind::Add => 1,
            DiffLineKind::Delete => 2,
        }
        .hash(&mut hasher);
        line.old_lineno.hash(&mut hasher);
        line.new_lineno.hash(&mut hasher);
        line.text.hash(&mut hasher);
    }
    Some(format!("{:016x}", hasher.finish()))
}

pub(crate) fn find_hunk_index_by_key(diff: &FileDiff, key: &str) -> Option<usize> {
    diff.hunks.iter().enumerate().find_map(|(index, _)| {
        (stable_hunk_key(diff, index).as_deref() == Some(key)).then_some(index)
    })
}

pub(crate) fn stable_diff_line_element_id(
    prefix: &str,
    diff: &FileDiff,
    hunk_idx: usize,
    line_idx: usize,
) -> SharedString {
    let hunk_key = stable_hunk_key(diff, hunk_idx).unwrap_or_default();
    stable_path_element_id(prefix, &format!("{hunk_key}:{line_idx}"))
}

/// 为长 Context 折叠占位行生成与 hunk 顺序无关的稳定键。
pub(crate) fn stable_diff_spacer_key(diff: &FileDiff, hunk_idx: usize, run_start: usize) -> u64 {
    use std::hash::{Hash, Hasher};

    let hunk_key = stable_hunk_key(diff, hunk_idx).unwrap_or_default();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "diff-spacer".hash(&mut hasher);
    hunk_key.hash(&mut hasher);
    run_start.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn stable_file_element_id(prefix: &str, kind: GroupKind, path: &str) -> SharedString {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    prefix.hash(&mut hasher);
    kind.hash(&mut hasher);
    path.hash(&mut hasher);
    SharedString::from(format!("vcs-{prefix}-{:016x}", hasher.finish()))
}

pub(crate) fn stable_compare_file_element_id(from: &str, to: &str, path: &str) -> SharedString {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "compare-file".hash(&mut hasher);
    from.hash(&mut hasher);
    to.hash(&mut hasher);
    path.hash(&mut hasher);
    SharedString::from(format!("vcs-compare-file-{:016x}", hasher.finish()))
}

pub(crate) fn stable_commit_path_element_id(
    prefix: &str,
    commit: &str,
    path: &str,
) -> SharedString {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    prefix.hash(&mut hasher);
    commit.hash(&mut hasher);
    path.hash(&mut hasher);
    SharedString::from(format!("vcs-{prefix}-{:016x}", hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::{
        find_hunk_index_by_key, stable_branch_element_id, stable_commit_path_element_id,
        stable_compare_file_element_id, stable_diff_line_element_id, stable_diff_spacer_key,
        stable_file_element_id, stable_hunk_key, stable_path_element_id, stable_remote_element_id,
        stable_tag_element_id,
    };
    use crate::views::helpers::GroupKind;

    #[test]
    fn path_element_id_is_stable_and_path_specific() {
        assert_eq!(
            stable_path_element_id("history", "src/lib.rs"),
            stable_path_element_id("history", "src/lib.rs")
        );
        assert_ne!(
            stable_path_element_id("history", "src/lib.rs"),
            stable_path_element_id("history", "src/main.rs")
        );
    }

    #[test]
    fn first_push_remote_id_is_stable_and_remote_specific() {
        assert_eq!(
            stable_path_element_id("first-push-remote", "upstream"),
            stable_path_element_id("first-push-remote", "upstream")
        );
        assert_ne!(
            stable_path_element_id("first-push-remote", "upstream"),
            stable_path_element_id("first-push-remote", "fork")
        );
    }

    #[test]
    fn sidebar_ref_ids_are_stable_and_category_specific() {
        assert_eq!(
            stable_branch_element_id(false, "feature/ui"),
            stable_branch_element_id(false, "feature/ui")
        );
        assert_ne!(
            stable_branch_element_id(false, "feature/ui"),
            stable_branch_element_id(true, "feature/ui")
        );
        assert_ne!(
            stable_branch_element_id(false, "feature/ui"),
            stable_branch_element_id(false, "feature ui")
        );
        assert_ne!(
            stable_remote_element_id("origin"),
            stable_tag_element_id("origin")
        );
    }

    #[test]
    fn hunk_key_follows_content_after_hunks_reorder() {
        let hunk = |old_start: u32, text: &str| ramag_domain::entities::Hunk {
            old_start,
            old_lines: 1,
            new_start: old_start,
            new_lines: 1,
            heading: None,
            lines: vec![ramag_domain::entities::DiffLine {
                kind: ramag_domain::entities::DiffLineKind::Add,
                old_lineno: None,
                new_lineno: Some(old_start),
                text: text.into(),
            }],
        };
        let mut diff = ramag_domain::entities::FileDiff {
            path: "src/lib.rs".into(),
            old_path: None,
            change_kind: ramag_domain::entities::FileChangeKind::Modified,
            binary: false,
            old_mode: None,
            new_mode: None,
            hunks: vec![hunk(10, "first"), hunk(20, "second")],
        };
        let first_key = stable_hunk_key(&diff, 0);
        assert!(first_key.is_some(), "第一个 hunk 应有稳定键");
        let first_key = first_key.unwrap_or_default();
        let first_line_id = stable_diff_line_element_id("diff-row", &diff, 0, 0);
        diff.hunks.reverse();
        assert_eq!(find_hunk_index_by_key(&diff, &first_key), Some(1));
        assert_eq!(find_hunk_index_by_key(&diff, "missing"), None);
        assert_eq!(
            first_line_id,
            stable_diff_line_element_id("diff-row", &diff, 1, 0)
        );
        assert_ne!(
            first_line_id,
            stable_diff_line_element_id("diff-row", &diff, 1, 1)
        );
    }

    #[test]
    fn file_element_id_separates_change_groups() {
        assert_ne!(
            stable_file_element_id("row", GroupKind::Staged, "src/lib.rs"),
            stable_file_element_id("row", GroupKind::Unstaged, "src/lib.rs")
        );
        assert_eq!(
            stable_file_element_id("row", GroupKind::Staged, "src/lib.rs"),
            stable_file_element_id("row", GroupKind::Staged, "src/lib.rs")
        );
    }

    #[test]
    fn diff_spacer_key_follows_hunk_after_reorder() {
        let hunk = |old_start: u32, text: &str| ramag_domain::entities::Hunk {
            old_start,
            old_lines: 1,
            new_start: old_start,
            new_lines: 1,
            heading: None,
            lines: vec![ramag_domain::entities::DiffLine {
                kind: ramag_domain::entities::DiffLineKind::Context,
                old_lineno: Some(old_start),
                new_lineno: Some(old_start),
                text: text.into(),
            }],
        };
        let mut diff = ramag_domain::entities::FileDiff {
            path: "src/lib.rs".into(),
            old_path: None,
            change_kind: ramag_domain::entities::FileChangeKind::Modified,
            binary: false,
            old_mode: None,
            new_mode: None,
            hunks: vec![hunk(10, "first"), hunk(20, "second")],
        };
        let first_key = stable_diff_spacer_key(&diff, 0, 4);
        diff.hunks.reverse();
        assert_eq!(first_key, stable_diff_spacer_key(&diff, 1, 4));
        assert_ne!(first_key, stable_diff_spacer_key(&diff, 1, 5));
    }

    #[test]
    fn compare_file_element_id_includes_revision_range_and_path() {
        assert_eq!(
            stable_compare_file_element_id("from-a", "to-b", "src/lib.rs"),
            stable_compare_file_element_id("from-a", "to-b", "src/lib.rs")
        );
        assert_ne!(
            stable_compare_file_element_id("from-a", "to-b", "src/lib.rs"),
            stable_compare_file_element_id("from-a", "to-c", "src/lib.rs")
        );
        assert_ne!(
            stable_compare_file_element_id("from-a", "to-b", "src/lib.rs"),
            stable_compare_file_element_id("from-a", "to-b", "src/main.rs")
        );
    }

    #[test]
    fn commit_path_element_id_includes_commit_and_path() {
        assert_eq!(
            stable_commit_path_element_id("commit-file", "commit-a", "src/lib.rs"),
            stable_commit_path_element_id("commit-file", "commit-a", "src/lib.rs")
        );
        assert_ne!(
            stable_commit_path_element_id("commit-file", "commit-a", "src/lib.rs"),
            stable_commit_path_element_id("commit-file", "commit-b", "src/lib.rs")
        );
        assert_ne!(
            stable_commit_path_element_id("commit-file", "commit-a", "src/lib.rs"),
            stable_commit_path_element_id("commit-file", "commit-a", "src/main.rs")
        );
    }
}
