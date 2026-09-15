use super::super::result_diff::ResultDiffCategory;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResultDiffFilter {
    All,
    Changed,
    Added,
    Removed,
}

impl ResultDiffFilter {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "全部",
            Self::Changed => "修改",
            Self::Added => "新增",
            Self::Removed => "删除",
        }
    }

    pub(super) fn includes(self, category: ResultDiffCategory) -> bool {
        match self {
            Self::All => true,
            Self::Changed => category == ResultDiffCategory::Changed,
            Self::Added => category == ResultDiffCategory::Added,
            Self::Removed => category == ResultDiffCategory::Removed,
        }
    }
}
