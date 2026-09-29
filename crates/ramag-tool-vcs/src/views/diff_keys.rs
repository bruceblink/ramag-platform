//! Diff 扁平 key：unified（每行单独）/ split（左右配对，对齐删除/新增）

use std::collections::HashSet;

use ramag_domain::entities::{DiffLineKind, FileDiff};

use super::helpers::stable_diff_spacer_key;

/// Unified 模式扁平 key：hunk header 或单行
#[derive(Clone, Copy)]
pub(super) enum UnifiedKey {
    Header { hunk_idx: usize },
    Line { hunk_idx: usize, line_idx: usize },
}

/// Split 模式扁平 key：hunk header / 左右配对行（删/增对侧空白对齐）/ Spacer 压缩长 Context
#[derive(Clone, Copy)]
pub(super) enum SplitKey {
    Header {
        hunk_idx: usize,
    },
    Pair {
        hunk_idx: usize,
        left: Option<usize>,
        right: Option<usize>,
    },
    Spacer {
        /// 被折叠的行数（首尾 KEEP 行不算）
        skipped: usize,
        /// 由文件 Diff、hunk 内容和 Context 段首行计算的稳定展开键。
        stable_key: u64,
    },
}

/// 连续 Context 行数阈值：超过此数视觉拆开两段变更（保留首尾各 SPLIT_SPACER_KEEP 行）
const SPLIT_SPACER_THRESHOLD: usize = 6;
const SPLIT_SPACER_KEEP: usize = 2;

/// 把 FileDiff 扁平化成 UnifiedKey 序列（hunk header + 每行）
/// `changes_only=true` 时跳过 Context 行；hunk 内全是 context（无变更）也不渲染该 hunk header
pub(super) fn build_unified_keys(diff: &FileDiff, changes_only: bool) -> Vec<UnifiedKey> {
    let mut out = Vec::new();
    for (h_idx, h) in diff.hunks.iter().enumerate() {
        let has_change = !changes_only
            || h.lines
                .iter()
                .any(|l| !matches!(l.kind, DiffLineKind::Context));
        if !has_change {
            continue;
        }
        out.push(UnifiedKey::Header { hunk_idx: h_idx });
        for (l_idx, line) in h.lines.iter().enumerate() {
            if changes_only && matches!(line.kind, DiffLineKind::Context) {
                continue;
            }
            out.push(UnifiedKey::Line {
                hunk_idx: h_idx,
                line_idx: l_idx,
            });
        }
    }
    out
}

/// 删除 / 新增按出现顺序左右配对。changes_only 跳过 Context；
/// collapse=true 时长 Context（≥THRESHOLD）压成 Spacer；collapse=false（FullFile）全铺开展示所有内容
pub(super) fn build_split_keys(
    diff: &FileDiff,
    changes_only: bool,
    collapse: bool,
    expanded_spacers: &HashSet<u64>,
) -> Vec<SplitKey> {
    let mut out = Vec::new();
    for (h_idx, h) in diff.hunks.iter().enumerate() {
        let has_change = !changes_only
            || h.lines
                .iter()
                .any(|l| !matches!(l.kind, DiffLineKind::Context));
        if !has_change {
            continue;
        }
        out.push(SplitKey::Header { hunk_idx: h_idx });
        let mut pending_left: Vec<usize> = Vec::new();
        let mut pending_right: Vec<usize> = Vec::new();
        // 当前 Context 段（连续 Context 行的索引集合）
        let mut ctx_run: Vec<usize> = Vec::new();
        let flush_ctx = |run: &mut Vec<usize>, out: &mut Vec<SplitKey>| {
            if run.is_empty() {
                return;
            }
            let run_start = run[0];
            let stable_key = stable_diff_spacer_key(diff, h_idx, run_start);
            let user_expanded = expanded_spacers.contains(&stable_key);
            if collapse && !changes_only && run.len() >= SPLIT_SPACER_THRESHOLD && !user_expanded {
                // 保留前 KEEP 行 + Spacer + 后 KEEP 行
                let n = run.len();
                for &i in run.iter().take(SPLIT_SPACER_KEEP) {
                    out.push(SplitKey::Pair {
                        hunk_idx: h_idx,
                        left: Some(i),
                        right: Some(i),
                    });
                }
                out.push(SplitKey::Spacer {
                    skipped: n - SPLIT_SPACER_KEEP * 2,
                    stable_key,
                });
                for &i in run.iter().skip(n - SPLIT_SPACER_KEEP) {
                    out.push(SplitKey::Pair {
                        hunk_idx: h_idx,
                        left: Some(i),
                        right: Some(i),
                    });
                }
            } else if !changes_only {
                // 短段（< 阈值）OR 用户已点击展开 → 全部 Context 行铺开
                for &i in run.iter() {
                    out.push(SplitKey::Pair {
                        hunk_idx: h_idx,
                        left: Some(i),
                        right: Some(i),
                    });
                }
            }
            run.clear();
        };
        for (i, line) in h.lines.iter().enumerate() {
            match line.kind {
                DiffLineKind::Delete => {
                    flush_ctx(&mut ctx_run, &mut out);
                    pending_left.push(i);
                }
                DiffLineKind::Add => {
                    flush_ctx(&mut ctx_run, &mut out);
                    pending_right.push(i);
                }
                DiffLineKind::Context => {
                    flush_pairs(h_idx, &mut pending_left, &mut pending_right, &mut out);
                    ctx_run.push(i);
                }
            }
        }
        flush_pairs(h_idx, &mut pending_left, &mut pending_right, &mut out);
        flush_ctx(&mut ctx_run, &mut out);
    }
    out
}

fn flush_pairs(
    hunk_idx: usize,
    left: &mut Vec<usize>,
    right: &mut Vec<usize>,
    out: &mut Vec<SplitKey>,
) {
    // 全部配对（左旧右新，对侧空时 None）—— 保持 split 视觉对称：
    // 删除行只在左栏，新增行只在右栏，对侧空白对齐，不跨栏
    let n = left.len().max(right.len());
    for i in 0..n {
        out.push(SplitKey::Pair {
            hunk_idx,
            left: left.get(i).copied(),
            right: right.get(i).copied(),
        });
    }
    left.clear();
    right.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::{DiffLine, FileChangeKind, FileDiff, Hunk};

    fn line(kind: DiffLineKind, text: &str) -> DiffLine {
        DiffLine {
            kind,
            old_lineno: None,
            new_lineno: None,
            text: text.into(),
        }
    }

    fn diff(lines: Vec<DiffLine>) -> FileDiff {
        FileDiff {
            path: "f".into(),
            old_path: None,
            change_kind: FileChangeKind::Modified,
            binary: false,
            old_mode: None,
            new_mode: None,
            hunks: vec![Hunk {
                old_start: 1,
                old_lines: 0,
                new_start: 1,
                new_lines: 0,
                heading: None,
                lines,
            }],
        }
    }

    fn pairs(keys: &[SplitKey]) -> Vec<(Option<usize>, Option<usize>)> {
        keys.iter()
            .filter_map(|k| match k {
                SplitKey::Pair { left, right, .. } => Some((*left, *right)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn unified_header_then_each_line() {
        let d = diff(vec![
            line(DiffLineKind::Context, "a"),
            line(DiffLineKind::Add, "b"),
        ]);
        let keys = build_unified_keys(&d, false);
        assert_eq!(keys.len(), 3); // Header + 2 行
        assert!(matches!(keys[0], UnifiedKey::Header { hunk_idx: 0 }));
        assert!(matches!(
            keys[2],
            UnifiedKey::Line {
                hunk_idx: 0,
                line_idx: 1
            }
        ));
    }

    #[test]
    fn unified_changes_only_skips_context() {
        let d = diff(vec![
            line(DiffLineKind::Context, "a"),
            line(DiffLineKind::Add, "b"),
        ]);
        let keys = build_unified_keys(&d, true);
        assert_eq!(keys.len(), 2, "Header + 仅 add 行（context 跳过）");
        assert!(matches!(keys[1], UnifiedKey::Line { line_idx: 1, .. }));
    }

    #[test]
    fn split_delete_left_add_right() {
        // 删 d / 增 x → 配对：左删右增
        let d = diff(vec![
            line(DiffLineKind::Delete, "d"),
            line(DiffLineKind::Add, "x"),
        ]);
        let keys = build_split_keys(&d, false, true, &HashSet::new());
        assert_eq!(pairs(&keys), vec![(Some(0), Some(1))], "删除在左、新增在右");
    }

    #[test]
    fn split_unequal_padded_with_none() {
        // 2 删 1 增 → 第二行右侧补 None（对侧空白对齐，不跨栏）
        let d = diff(vec![
            line(DiffLineKind::Delete, "d0"),
            line(DiffLineKind::Delete, "d1"),
            line(DiffLineKind::Add, "a0"),
        ]);
        let keys = build_split_keys(&d, false, true, &HashSet::new());
        assert_eq!(pairs(&keys), vec![(Some(0), Some(2)), (Some(1), None)]);
    }

    #[test]
    fn split_context_pairs_both_sides() {
        // context 行左右同 line_idx 配对
        let d = diff(vec![line(DiffLineKind::Context, "ctx")]);
        let keys = build_split_keys(&d, false, true, &HashSet::new());
        assert_eq!(pairs(&keys), vec![(Some(0), Some(0))], "context 两侧同行");
    }

    #[test]
    fn split_spacer_expansion_follows_hunk_after_reorder() {
        let hunk = |start: u32, marker: &str| {
            let mut lines = (0..8)
                .map(|index| line(DiffLineKind::Context, &format!("{marker}-{index}")))
                .collect::<Vec<_>>();
            lines.push(line(DiffLineKind::Add, marker));
            ramag_domain::entities::Hunk {
                old_start: start,
                old_lines: 9,
                new_start: start,
                new_lines: 9,
                heading: None,
                lines,
            }
        };
        let mut diff = FileDiff {
            path: "f".into(),
            old_path: None,
            change_kind: ramag_domain::entities::FileChangeKind::Modified,
            binary: false,
            old_mode: None,
            new_mode: None,
            hunks: vec![hunk(1, "first"), hunk(20, "second")],
        };
        let first_key = stable_diff_spacer_key(&diff, 0, 0);
        let expanded = HashSet::from([first_key]);
        let initial = build_split_keys(&diff, false, true, &expanded);
        assert!(!initial.iter().any(|key| {
            matches!(key, SplitKey::Spacer { stable_key, .. } if *stable_key == first_key)
        }));

        diff.hunks.reverse();
        let reordered = build_split_keys(&diff, false, true, &expanded);
        assert!(!reordered.iter().any(|key| {
            matches!(key, SplitKey::Spacer { stable_key, .. } if *stable_key == first_key)
        }));
        let second_key = stable_diff_spacer_key(&diff, 0, 0);
        assert!(reordered.iter().any(|key| {
            matches!(key, SplitKey::Spacer { stable_key, .. } if *stable_key == second_key)
        }));
    }
}
