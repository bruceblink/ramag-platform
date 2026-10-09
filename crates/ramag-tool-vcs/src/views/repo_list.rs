//! 仓库管理页。

use std::ops::Range;
use std::rc::Rc;

use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Sizable as _, WindowExt as _,
    button::ButtonVariants as _, h_flex, input::Input, v_flex,
};
use gpui_kit::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, ParentElement, Styled, Window, div,
    prelude::*, px, uniform_list,
};

impl VcsView {
    pub(super) fn render_error_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let err = self.error.as_ref()?;
        let theme = cx.theme();
        let danger = theme.danger;
        Some(
            v_flex()
                .debug_selector(|| "vcs-error-banner".into())
                .w_full()
                .flex_none()
                .px(px(16.0))
                .py(px(8.0))
                .child(
                    ramag_ui::pulse_ui::pulse_panel(cx)
                        .debug_selector(|| "vcs-error-panel".into())
                        .p(px(8.0))
                        .gap(px(4.0))
                        .border_color(danger.opacity(0.32))
                        .child(
                            ramag_ui::pulse_ui::pulse_status_notice(
                                ramag_ui::pulse_ui::PulseStatus::Failed,
                                err.clone(),
                                cx,
                            )
                            .border_0()
                            .rounded(px(0.0))
                            .p(px(0.0)),
                        )
                        .child(
                            ramag_ui::responsive_toolbar()
                                .debug_selector(|| "vcs-error-toolbar".into())
                                .min_h(px(28.0))
                                .justify_end()
                                .child({
                                    let err_for_copy = err.clone();
                                    ramag_ui::clickable_button("vcs-error-copy")
                                        .debug_selector(|| "vcs-error-copy".into())
                                        .ghost()
                                        .xsmall()
                                        .flex_none()
                                        .label("复制")
                                        .on_click(move |_: &ClickEvent, _, app| {
                                            app.write_to_clipboard(
                                                gpui_kit::ClipboardItem::new_string(
                                                    err_for_copy.clone(),
                                                ),
                                            );
                                        })
                                })
                                .child(
                                    ramag_ui::clickable_button("vcs-error-clear")
                                        .debug_selector(|| "vcs-error-clear".into())
                                        .ghost()
                                        .xsmall()
                                        .flex_none()
                                        .icon(IconName::Close)
                                        .tooltip("清除")
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.clear_error(cx);
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}
use ramag_domain::entities::{RepoConfig, contains_case_insensitive};

use super::helpers::stable_path_element_id;
use super::vcs_view::VcsView;

const CONTENT_MAX_W: f32 = 1080.0;

pub(super) struct RepoListRowsCacheEntry {
    repos: Rc<Vec<RepoConfig>>,
    query_lower: String,
    indices: Rc<Vec<usize>>,
}

impl RepoListRowsCacheEntry {
    fn get(&self, repos: &Rc<Vec<RepoConfig>>, query_lower: &str) -> Option<Rc<Vec<usize>>> {
        (Rc::ptr_eq(&self.repos, repos) && self.query_lower == query_lower)
            .then(|| self.indices.clone())
    }
}

impl VcsView {
    pub(super) fn render_repo_list(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let muted_fg = theme.muted_foreground;
        let fg = theme.foreground;
        let accent = theme.accent;
        let border = theme.border;
        let row_hover = theme.muted;
        let busy = self.busy || self.loading || self.directory_picker_busy;
        let compact = window.viewport_size().width < px(720.0);

        let query = self
            .repo_search_input
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let repos_rc = self.recent_repos.clone();
        let filtered_indices = self.filtered_repo_indices(repos_rc.clone(), &query);
        let total = repos_rc.len();
        let visible_count = filtered_indices.len();

        let header = ramag_ui::pulse_ui::pulse_page_title(
            "仓库管理",
            Some(format!("最近打开的 {total} 个仓库")),
            cx,
        )
        .max_w(px(CONTENT_MAX_W))
        .id("vcs-repo-list-page-title")
        .debug_selector(|| "vcs-repo-list-page-title".into());
        let toolbar = ramag_ui::pulse_ui::pulse_home_toolbar(cx)
            .debug_selector(|| "vcs-repo-list-toolbar".into())
            .child(
                div()
                    .debug_selector(|| "vcs-repo-list-search-field".into())
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(360.0))
                            .min_w(if compact { px(160.0) } else { px(0.0) })
                            .child(
                                ramag_ui::cleanable_input(
                                    &self.repo_search_input,
                                    "vcs-repo-search-clear",
                                    false,
                                    cx,
                                )
                                .small()
                                .prefix(Icon::new(IconName::Search).small().text_color(muted_fg)),
                            ),
                    ),
            )
            .child(
                ramag_ui::clickable_button("vcs-repo-add")
                    .ghost()
                    .small()
                    .flex_none()
                    .icon(IconName::FolderOpen)
                    .label("打开")
                    .tooltip("打开")
                    .disabled(busy)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.pick_directory(cx);
                    })),
            )
            .child(
                ramag_ui::clickable_button("vcs-repo-clone")
                    .ghost()
                    .small()
                    .flex_none()
                    .icon(ramag_ui::icons::git_clone())
                    .label("克隆")
                    .tooltip("克隆")
                    .disabled(busy)
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.open_clone_dialog(window, cx);
                    })),
            )
            .child(
                ramag_ui::clickable_button("vcs-repo-init")
                    .ghost()
                    .small()
                    .flex_none()
                    .icon(IconName::Plus)
                    .label("初始化")
                    .tooltip("初始化")
                    .disabled(busy)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.pick_init_directory(cx);
                    })),
            );

        let body: AnyElement = if total == 0 {
            empty_state(cx)
        } else if visible_count == 0 {
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_sm()
                        .text_color(fg)
                        .child(format!("没有匹配「{query}」的仓库")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted_fg)
                        .child("尝试修改关键字或清空搜索"),
                )
                .into_any_element()
        } else {
            let rows = uniform_list(
                "vcs-repo-list-rows",
                visible_count,
                cx.processor({
                    let repos_rc = repos_rc.clone();
                    let filtered_indices = filtered_indices.clone();
                    move |_this, range: Range<usize>, _window, cx| {
                        range
                            .map(|row_index| {
                                let repo_index = filtered_indices[row_index];
                                repo_row(
                                    &repos_rc[repo_index],
                                    compact,
                                    busy,
                                    border,
                                    row_hover,
                                    accent,
                                    fg,
                                    muted_fg,
                                    cx,
                                )
                                .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    }
                }),
            )
            .w_full()
            .flex_1()
            .min_h_0();
            v_flex()
                .size_full()
                .min_h_0()
                .child(repo_heading(compact, cx))
                .child(rows)
                .into_any_element()
        };

        let mut root = ramag_ui::pulse_ui::pulse_home_frame(cx)
            .debug_selector(|| "vcs-repo-list".into())
            .size_full();
        if let Some(banner) = self.render_error_banner(cx) {
            root = root.child(banner);
        }
        root.child(header)
            .child(
                ramag_ui::pulse_ui::pulse_home_panel(cx)
                    .debug_selector(|| "vcs-repo-list-panel".into())
                    .child(toolbar)
                    .child(div().flex_1().min_h_0().overflow_hidden().child(body)),
            )
            .into_any_element()
    }

    fn filtered_repo_indices(
        &self,
        repos: Rc<Vec<RepoConfig>>,
        query_lower: &str,
    ) -> Rc<Vec<usize>> {
        {
            let cache = self.repo_list_rows_cache.borrow();
            if let Some(indices) = cache
                .as_ref()
                .and_then(|entry| entry.get(&repos, query_lower))
            {
                return indices;
            }
        }

        let mut indices: Vec<usize> = (0..repos.len())
            .filter(|&index| repo_matches_query(&repos[index], query_lower))
            .collect();
        indices.sort_by(|&left, &right| {
            let left = &repos[left];
            let right = &repos[right];
            right
                .last_opened_at
                .cmp(&left.last_opened_at)
                .then_with(|| left.name.cmp(&right.name))
        });
        let indices = Rc::new(indices);
        self.repo_list_rows_cache
            .replace(Some(RepoListRowsCacheEntry {
                repos,
                query_lower: query_lower.to_string(),
                indices: indices.clone(),
            }));
        indices
    }

    fn open_clone_dialog(&mut self, window: &mut gpui_kit::Window, cx: &mut Context<Self>) {
        self.clone_dest_path = None;
        self.clone_url_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let content_view = view.clone();
            dialog
                .title(ramag_ui::closable_dialog_title(
                    "vcs-clone-close",
                    "克隆仓库",
                    |_, _| {},
                ))
                .close_button(false)
                .width(px(600.0))
                .margin_top(px(150.0))
                .content(move |content, _, app| {
                    let input = content_view.read(app).clone_url_input.clone();
                    let destination = content_view
                        .read(app)
                        .clone_dest_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "选择目标父目录".into());
                    let busy = content_view.read(app).directory_picker_busy;
                    let pick_view = content_view.clone();
                    let clone_view = content_view.clone();
                    content.child(
                        v_flex()
                            .w_full()
                            .gap(px(12.0))
                            .child(Input::new(&input).small())
                            .child(
                                ramag_ui::clickable_button("vcs-clone-pick-dest")
                                    .outline()
                                    .small()
                                    .icon(IconName::Folder)
                                    .label(destination)
                                    .disabled(busy)
                                    .on_click(move |_: &ClickEvent, _, app| {
                                        pick_view.update(app, |this, cx| {
                                            this.pick_clone_destination(cx);
                                        });
                                    }),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .justify_end()
                                    .gap(px(8.0))
                                    .child(
                                        ramag_ui::clickable_button("vcs-clone-cancel")
                                            .ghost()
                                            .small()
                                            .label("取消")
                                            .on_click(|_: &ClickEvent, window, app| {
                                                window.close_dialog(app);
                                            }),
                                    )
                                    .child(
                                        ramag_ui::clickable_button("vcs-clone-execute")
                                            .primary()
                                            .small()
                                            .label("克隆")
                                            .disabled(
                                                content_view.read(app).clone_dest_path.is_none(),
                                            )
                                            .on_click(move |_: &ClickEvent, window, app| {
                                                let started = clone_view.update(app, |this, cx| {
                                                    let url = this
                                                        .clone_url_input
                                                        .read(cx)
                                                        .value()
                                                        .trim()
                                                        .to_string();
                                                    let Some(parent) = this.clone_dest_path.clone()
                                                    else {
                                                        return false;
                                                    };
                                                    let Some(name) = clone_repo_name(&url) else {
                                                        this.error = Some(
                                                            "请输入可识别仓库名的 Clone 地址"
                                                                .into(),
                                                        );
                                                        cx.notify();
                                                        return false;
                                                    };
                                                    this.clone_repo_async(
                                                        url,
                                                        parent.join(name),
                                                        cx,
                                                    );
                                                    true
                                                });
                                                if started {
                                                    window.close_dialog(app);
                                                }
                                            }),
                                    ),
                            ),
                    )
                })
        });
    }
}

fn repo_matches_query(repo: &RepoConfig, query_lower: &str) -> bool {
    query_lower.is_empty()
        || contains_case_insensitive(&repo.name, query_lower)
        || contains_case_insensitive(&repo.path, query_lower)
}

fn clone_repo_name(source: &str) -> Option<String> {
    let source = source.trim().trim_end_matches(['/', '\\']);
    if let Some((_, remainder)) = source.split_once("://")
        && !remainder.contains(['/', '\\'])
    {
        return None;
    }
    let tail = source.rsplit(['/', '\\']).next()?;
    let tail = tail.rsplit(':').next().unwrap_or(tail);
    let name = tail.strip_suffix(".git").unwrap_or(tail);
    (!name.is_empty() && name != "." && name != "..").then(|| name.to_string())
}

/// Headings share the row's fixed type, path and action widths.
fn repo_heading(compact: bool, cx: &gpui_kit::App) -> gpui_kit::Div {
    h_flex()
        .w_full()
        .flex_none()
        .h(px(32.0))
        .px(px(14.0))
        .gap(px(12.0))
        .items_center()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .border_b_1()
        .border_color(cx.theme().border)
        .when(!compact, |header| {
            header.child(div().w(px(76.0)).flex_none().child("类型"))
        })
        .child(
            div()
                .debug_selector(|| "vcs-repo-list-heading-name".into())
                .flex_1()
                .min_w_0()
                .when(compact, |cell| cell.pl(px(64.0)))
                .child("仓库"),
        )
        .when(!compact, |header| {
            header.child(div().w(px(360.0)).flex_none().child("路径"))
        })
        .child(div().w(px(36.0)).flex_none().child("操作"))
}

#[allow(clippy::too_many_arguments)]
fn repo_row(
    r: &RepoConfig,
    compact: bool,
    busy: bool,
    border: gpui_kit::Hsla,
    hover_bg: gpui_kit::Hsla,
    accent: gpui_kit::Hsla,
    fg: gpui_kit::Hsla,
    muted_fg: gpui_kit::Hsla,
    cx: &mut Context<VcsView>,
) -> impl IntoElement {
    let badge_fg = accent;
    let mut badge_bg = badge_fg;
    badge_bg.a = 0.12;

    let path_for_open = r.path.clone();
    let path_for_remove = r.path.clone();
    let stable_key = r.id.to_string();
    let row_id = stable_path_element_id("repo-row", &stable_key);
    let del_id = stable_path_element_id("repo-del", &stable_key);
    let row_selector = format!("vcs-repo-row-{stable_key}");
    let name_selector = format!("vcs-repo-row-name-{stable_key}");
    let path_selector = format!("vcs-repo-row-path-{stable_key}");
    let actions_selector = format!("vcs-repo-row-actions-{stable_key}");

    let mono = cx.theme().mono_font_family.clone();

    let badge = div()
        .flex_none()
        .w(if compact { px(56.0) } else { px(76.0) })
        .flex()
        .justify_center()
        .child(
            div()
                .px(px(8.0))
                .py(px(2.0))
                .rounded(px(4.0))
                .text_xs()
                .text_color(badge_fg)
                .bg(badge_bg)
                .child("Git"),
        );
    let name = div()
        .debug_selector(move || name_selector.clone())
        .flex_1()
        .min_w_0()
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(fg)
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .child(super::inline_text_preview(&r.name, 160));
    let path = div()
        .debug_selector(move || path_selector.clone())
        .flex_none()
        .w(if compact { px(0.0) } else { px(360.0) })
        .when(compact, |this| this.w_full().pl(px(64.0)))
        .text_xs()
        .text_color(muted_fg)
        .font_family(mono)
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .child(super::inline_text_preview(&r.path, 240));
    let actions = h_flex()
        .debug_selector(move || actions_selector.clone())
        .flex_none()
        .gap(px(4.0))
        .w(px(36.0))
        .justify_end()
        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
            cx.stop_propagation()
        })
        .child(
            ramag_ui::clickable_button(del_id)
                .ghost()
                .small()
                .icon(ramag_ui::icons::trash())
                .tooltip("移除")
                .disabled(busy)
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.confirm_remove_recent_repo(path_for_remove.clone(), window, cx);
                })),
        );

    let mut row = if compact { v_flex() } else { h_flex() };
    row = row
        .w_full()
        .items_center()
        .gap(px(12.0))
        .px(px(14.0))
        .py(if compact { px(10.0) } else { px(8.0) })
        .border_b_1()
        .border_color(border);
    if compact {
        row = row
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(8.0))
                    .child(badge)
                    .child(name)
                    .child(actions),
            )
            .child(path);
    } else {
        row = row.child(badge).child(name).child(path).child(actions);
    }
    row.id(row_id)
        .debug_selector(move || row_selector.clone())
        .cursor_pointer()
        .hover(move |this| this.bg(hover_bg))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.open_recent_repo(path_for_open.clone(), cx);
        }))
}

fn empty_state(cx: &mut Context<VcsView>) -> AnyElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .child(
            ramag_ui::clickable_button("vcs-repo-empty-pick")
                .primary()
                .icon(IconName::Plus)
                .label("打开")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.pick_directory(cx);
                })),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{RepoListRowsCacheEntry, clone_repo_name, repo_matches_query};
    use ramag_domain::entities::RepoConfig;
    use std::rc::Rc;

    #[test]
    fn clone_name_supports_urls_and_windows_paths() {
        assert_eq!(
            clone_repo_name("https://example.com/team/ramag.git"),
            Some("ramag".into())
        );
        assert_eq!(clone_repo_name(r"C:\work\ramag.git"), Some("ramag".into()));
        assert_eq!(
            clone_repo_name("git@example.com:ramag.git"),
            Some("ramag".into())
        );
        assert_eq!(clone_repo_name("https://example.com/"), None);
        assert_eq!(clone_repo_name("/"), None);
    }

    #[test]
    fn repo_search_matches_unicode_without_lowercasing_each_field() {
        let mut repo = RepoConfig::from_path("/tmp/Über-project");
        repo.name = "数据工具".into();

        assert!(repo_matches_query(&repo, "über"));
        assert!(repo_matches_query(&repo, "工具"));
        assert!(!repo_matches_query(&repo, "missing"));
    }

    #[test]
    fn repo_list_cache_requires_same_source_and_query() {
        let repos = Rc::new(vec![RepoConfig::from_path("/tmp/repo")]);
        let indices = Rc::new(vec![0]);
        let cache = RepoListRowsCacheEntry {
            repos: repos.clone(),
            query_lower: "repo".into(),
            indices: indices.clone(),
        };

        let cached = cache.get(&repos, "repo");
        assert!(
            cached
                .as_ref()
                .is_some_and(|value| Rc::ptr_eq(value, &indices))
        );
        assert!(cache.get(&repos, "other").is_none());
        assert!(
            cache
                .get(&Rc::new(repos.as_ref().clone()), "repo")
                .is_none()
        );
    }
}
