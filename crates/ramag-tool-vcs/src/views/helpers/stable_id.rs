use gpui_kit::SharedString;

use super::GroupKind;

pub(crate) fn stable_path_element_id(prefix: &str, path: &str) -> SharedString {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    prefix.hash(&mut hasher);
    path.hash(&mut hasher);
    SharedString::from(format!("vcs-{prefix}-{:016x}", hasher.finish()))
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
        stable_commit_path_element_id, stable_compare_file_element_id, stable_file_element_id,
        stable_path_element_id,
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
