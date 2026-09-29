//! 历史视图左侧分支面板。

use std::ops::Range;
use std::rc::Rc;

use gpui_kit::component::v_flex;
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement, Styled, px,
    uniform_list,
};
use ramag_domain::entities::{Branch, Remote, Tag};

use super::super::sidebar::{LeftRow, SidebarSection};
use super::super::vcs_view::VcsView;
use super::{HistoryLeftRowsCacheEntry, HistoryLeftRowsCacheKey};

impl VcsView {
    pub(super) fn render_history_left_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.history_left_rows();
        let body = uniform_list(
            "vcs-history-left-rows",
            rows.len(),
            cx.processor({
                let rows = rows.clone();
                move |this, range: Range<usize>, _window, cx| {
                    range
                        .map(|index| this.render_left_row(&rows[index], cx))
                        .collect::<Vec<_>>()
                }
            }),
        )
        .track_scroll(&self.history_left_scroll)
        .flex_1();

        v_flex()
            .id("vcs-history-left-pane")
            .size_full()
            .min_h_0()
            .px(px(8.0))
            .py(px(6.0))
            .child(body)
            .into_any_element()
    }

    fn history_left_rows(&self) -> Rc<Vec<LeftRow>> {
        let key = HistoryLeftRowsCacheKey {
            local_identity: self.local_branches.as_ptr() as usize,
            local_len: self.local_branches.len(),
            remote_identity: self.remote_branches.as_ptr() as usize,
            remote_len: self.remote_branches.len(),
            tags_identity: self.tags.as_ptr() as usize,
            tags_len: self.tags.len(),
            remotes_identity: self.remotes.as_ptr() as usize,
            remotes_len: self.remotes.len(),
            collapsed_local: self.collapsed_local,
            collapsed_remote: self.collapsed_remote,
            collapsed_tag: self.collapsed_tag,
            collapsed_remote_repos: self.collapsed_remote_repos,
        };
        {
            let cache = self.history_left_rows_cache.borrow();
            if let Some(rows) = cache.as_ref().and_then(|entry| entry.get(&key)) {
                return rows;
            }
        }

        let rows = build_history_left_rows(
            &self.local_branches,
            &self.remote_branches,
            &self.remotes,
            &self.tags,
            CollapsedSections {
                local: self.collapsed_local,
                remote: self.collapsed_remote,
                tag: self.collapsed_tag,
                remote_repos: self.collapsed_remote_repos,
            },
        );
        let rows = Rc::new(rows);
        self.history_left_rows_cache
            .replace(Some(HistoryLeftRowsCacheEntry {
                key,
                rows: rows.clone(),
            }));
        rows
    }
}

#[derive(Clone, Copy)]
struct CollapsedSections {
    local: bool,
    remote: bool,
    tag: bool,
    remote_repos: bool,
}

fn build_history_left_rows(
    local_branches: &[Branch],
    remote_branches: &[Branch],
    remotes: &[Remote],
    tags: &[Tag],
    collapsed: CollapsedSections,
) -> Vec<LeftRow> {
    let mut rows = Vec::new();
    rows.push(LeftRow::Header {
        title: "本地分支",
        count: local_branches.len(),
        collapsed: collapsed.local,
        section: SidebarSection::Local,
    });
    if !collapsed.local {
        for branch in local_branches {
            rows.push(LeftRow::Branch {
                branch: branch.clone(),
                is_remote: false,
            });
        }
    }

    rows.push(LeftRow::Header {
        title: "远程分支",
        count: remote_branches.len(),
        collapsed: collapsed.remote,
        section: SidebarSection::Remote,
    });
    if !collapsed.remote {
        if remote_branches.is_empty() {
            rows.push(LeftRow::Empty("暂无远程分支；获取后显示"));
        } else {
            for branch in remote_branches {
                rows.push(LeftRow::Branch {
                    branch: branch.clone(),
                    is_remote: true,
                });
            }
        }
    }

    rows.push(LeftRow::Header {
        title: "远程仓库",
        count: remotes.len(),
        collapsed: collapsed.remote_repos,
        section: SidebarSection::RemoteRepo,
    });
    if !collapsed.remote_repos {
        if remotes.is_empty() {
            rows.push(LeftRow::Empty("暂无远程仓库"));
        } else {
            for remote in remotes {
                rows.push(LeftRow::Remote {
                    remote: remote.clone(),
                });
            }
        }
    }

    rows.push(LeftRow::Header {
        title: "标签",
        count: tags.len(),
        collapsed: collapsed.tag,
        section: SidebarSection::Tag,
    });
    if !collapsed.tag {
        if tags.is_empty() {
            rows.push(LeftRow::Empty("暂无标签"));
        } else {
            for tag in tags {
                rows.push(LeftRow::Tag { tag: tag.clone() });
            }
        }
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::{BranchKind, CommitId, TagKind};

    fn branch(name: &str, commit: &str) -> Branch {
        Branch {
            name: name.into(),
            kind: BranchKind::Local,
            commit: CommitId(commit.into()),
            is_head: false,
            upstream: None,
            ahead: None,
            behind: None,
        }
    }

    #[test]
    fn history_rows_keep_rendered_objects_after_source_lists_reorder() {
        let main = branch("main", "commit-main");
        let feature = branch("feature", "commit-feature");
        let remote = Remote {
            name: "origin".into(),
            fetch_url: "https://example.test/old.git".into(),
            push_url: None,
        };
        let tag = Tag {
            name: "v1".into(),
            kind: TagKind::Lightweight,
            commit: CommitId("commit-tag".into()),
            message: None,
            tagger: None,
        };

        let mut source_branches = vec![main.clone(), feature.clone()];
        let mut source_remotes = vec![remote.clone()];
        let rows = build_history_left_rows(
            &source_branches,
            &[],
            &source_remotes,
            std::slice::from_ref(&tag),
            CollapsedSections {
                local: false,
                remote: false,
                tag: false,
                remote_repos: false,
            },
        );
        source_branches.swap(0, 1);
        source_remotes[0].fetch_url = "https://example.test/new.git".into();
        assert_eq!(source_branches[0].name, "feature");
        assert_eq!(source_remotes[0].fetch_url, "https://example.test/new.git");

        let branch_names = rows
            .iter()
            .filter_map(|row| match row {
                LeftRow::Branch { branch, .. } => Some(branch.name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(branch_names, ["main", "feature"]);
        assert!(rows.iter().any(|row| matches!(
            row,
            LeftRow::Remote { remote } if remote.fetch_url == "https://example.test/old.git"
        )));
        assert!(rows.iter().any(|row| matches!(
            row,
            LeftRow::Tag { tag } if tag.name == "v1"
        )));
    }
}
