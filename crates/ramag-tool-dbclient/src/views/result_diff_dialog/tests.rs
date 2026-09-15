use super::ResultDiffFilter;
use crate::views::result_diff::ResultDiffCategory;

#[test]
fn result_diff_filters_keep_changed_pairs_together() {
    assert!(ResultDiffFilter::All.includes(ResultDiffCategory::Context));
    assert!(ResultDiffFilter::Changed.includes(ResultDiffCategory::Changed));
    assert!(ResultDiffFilter::Added.includes(ResultDiffCategory::Added));
    assert!(ResultDiffFilter::Removed.includes(ResultDiffCategory::Removed));

    assert!(!ResultDiffFilter::Changed.includes(ResultDiffCategory::Added));
    assert!(!ResultDiffFilter::Changed.includes(ResultDiffCategory::Removed));
    assert!(!ResultDiffFilter::Added.includes(ResultDiffCategory::Changed));
    assert!(!ResultDiffFilter::Removed.includes(ResultDiffCategory::Changed));
}

#[test]
fn result_diff_filter_labels_are_stable() {
    assert_eq!(ResultDiffFilter::All.label(), "全部");
    assert_eq!(ResultDiffFilter::Changed.label(), "修改");
    assert_eq!(ResultDiffFilter::Added.label(), "新增");
    assert_eq!(ResultDiffFilter::Removed.label(), "删除");
}
