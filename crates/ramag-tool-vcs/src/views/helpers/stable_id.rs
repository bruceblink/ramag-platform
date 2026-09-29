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

#[cfg(test)]
mod tests {
    use super::{stable_file_element_id, stable_path_element_id};
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
}
