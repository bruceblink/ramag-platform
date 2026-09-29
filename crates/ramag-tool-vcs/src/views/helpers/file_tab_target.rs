use super::{FileTab, FileTabSource, GroupKind};

/// 文件标签关闭和调试节点使用的稳定身份；不把会变化的数组位置作为目标。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum FileTabIdentity {
    Changes(GroupKind),
    ProjectFiles,
    Commit(String),
    Compare { from: String, to: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct FileTabTarget {
    pub(super) path: String,
    pub(super) identity: FileTabIdentity,
}

impl FileTabTarget {
    pub(crate) fn from_parts(path: String, source: &FileTabSource) -> Self {
        Self {
            path,
            identity: FileTabIdentity::from_source(source),
        }
    }

    pub(crate) fn element_id(&self) -> String {
        use std::hash::{Hash, Hasher};

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    pub(crate) fn matches(&self, tab: &FileTab) -> bool {
        self.path == tab.path && self.identity == FileTabIdentity::from_source(&tab.source)
    }
}

impl FileTabIdentity {
    fn from_source(source: &FileTabSource) -> Self {
        match source {
            FileTabSource::Changes(kind) => Self::Changes(*kind),
            FileTabSource::ProjectFiles => Self::ProjectFiles,
            // change_kind 只是提交文件的显示信息，不参与标签身份。
            FileTabSource::Commit { commit_id, .. } => Self::Commit(commit_id.clone()),
            FileTabSource::Compare { from, to } => Self::Compare {
                from: from.clone(),
                to: to.clone(),
            },
        }
    }
}

impl FileTab {
    pub(crate) fn target(&self) -> FileTabTarget {
        FileTabTarget::from_parts(self.path.clone(), &self.source)
    }
}

pub(crate) fn find_file_tab_index(tabs: &[FileTab], target: &FileTabTarget) -> Option<usize> {
    tabs.iter().position(|tab| target.matches(tab))
}

#[cfg(test)]
mod tests {
    use ramag_domain::entities::FileChangeKind;

    use super::super::{FileTab, FileTabSource, GroupKind};
    use super::{FileTabTarget, find_file_tab_index};

    #[test]
    fn file_tab_target_follows_tab_after_reorder() {
        let first = FileTab {
            path: "src/first.rs".into(),
            source: FileTabSource::Changes(GroupKind::Unstaged),
            cached_diff: None,
            cached_diff_syntax: None,
            cached_content: None,
        };
        let second = FileTab {
            path: "src/second.rs".into(),
            source: FileTabSource::ProjectFiles,
            cached_diff: None,
            cached_diff_syntax: None,
            cached_content: None,
        };
        let target = first.target();
        let reordered = vec![second, first];

        assert_eq!(find_file_tab_index(&reordered, &target), Some(1));
    }

    #[test]
    fn missing_file_tab_target_does_not_fall_back_to_another_index() {
        let tabs = vec![FileTab {
            path: "src/other.rs".into(),
            source: FileTabSource::Changes(GroupKind::Unstaged),
            cached_diff: None,
            cached_diff_syntax: None,
            cached_content: None,
        }];
        let target = FileTabTarget::from_parts(
            "src/removed.rs".into(),
            &FileTabSource::Changes(GroupKind::Unstaged),
        );

        assert_eq!(find_file_tab_index(&tabs, &target), None);
    }

    #[test]
    fn commit_tab_target_ignores_display_only_change_kind() {
        let target = FileTabTarget::from_parts(
            "src/lib.rs".into(),
            &FileTabSource::Commit {
                commit_id: "abc123".into(),
                change_kind: Some(FileChangeKind::Modified),
            },
        );
        let tab = FileTab {
            path: "src/lib.rs".into(),
            source: FileTabSource::Commit {
                commit_id: "abc123".into(),
                change_kind: Some(FileChangeKind::Renamed),
            },
            cached_diff: None,
            cached_diff_syntax: None,
            cached_content: None,
        };

        assert_eq!(find_file_tab_index(&[tab], &target), Some(0));
    }

    #[test]
    fn file_tab_element_id_is_stable_and_target_specific() {
        let source = FileTabSource::ProjectFiles;
        let target = FileTabTarget::from_parts("src/lib.rs".into(), &source);
        let same = FileTabTarget::from_parts("src/lib.rs".into(), &source);
        let other = FileTabTarget::from_parts("src/main.rs".into(), &source);

        assert_eq!(target.element_id(), same.element_id());
        assert_ne!(target.element_id(), other.element_id());
    }
}
