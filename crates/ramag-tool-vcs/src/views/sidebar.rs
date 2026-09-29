//! 折叠段共享件：SidebarSection + section_header + history 左栏行类型 / 分发。
//! 左栏（本地/远程分支 + Tag）合并为单个 uniform_list，所有行统一 28px 等高

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _, WindowExt as _,
    button::ButtonVariants as _,
    h_flex,
    input::{Input, InputState},
    menu::{ContextMenuExt as _, PopupMenu},
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, ClickEvent, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, ScrollHandle, SharedString, Styled, Window, div, prelude::*, px,
};
use ramag_domain::entities::{Branch, Remote, Tag};

use super::helpers::HistoryRefFilter;
use super::vcs_view::VcsView;

/// 行高固定 28px：uniform_list 行级虚拟化要求所有行等高
pub(super) const LEFT_ROW_H: f32 = 28.0;

/// 折叠段标识（用于 section_header 点击切换状态）
#[derive(Debug, Clone, Copy)]
pub(super) enum SidebarSection {
    Local,
    Remote,
    Tag,
    /// 远程仓库配置（origin 等），区别于 `Remote`（远程分支引用）
    RemoteRepo,
}

/// 左栏扁平行：段表头 / 分支 / Tag / 空占位。
pub(super) enum LeftRow {
    Header {
        title: &'static str,
        count: usize,
        collapsed: bool,
        section: SidebarSection,
    },
    Branch {
        branch: Branch,
        is_remote: bool,
    },
    Tag {
        tag: Tag,
    },
    Remote {
        remote: Remote,
    },
    Empty(&'static str),
}

impl VcsView {
    /// uniform_list 单行分发（左栏：分支段 + Tag 段）
    pub(super) fn render_left_row(&self, row: &LeftRow, cx: &mut Context<Self>) -> AnyElement {
        match row {
            LeftRow::Header {
                title,
                count,
                collapsed,
                section,
            } => section_header(title, *count, *collapsed, *section, self.busy, cx),
            LeftRow::Branch { branch, is_remote } => {
                let selected = self.history_ref_filter.as_ref().is_some_and(|filter| {
                    filter.revision == HistoryRefFilter::branch(&branch.name, *is_remote).revision
                });
                super::sidebar_branches::branch_row(branch, self.busy, *is_remote, selected, cx)
                    .into_any_element()
            }
            LeftRow::Tag { tag } => {
                let selected = self.history_ref_filter.as_ref().is_some_and(|filter| {
                    filter.revision == HistoryRefFilter::tag(&tag.name).revision
                });
                super::sidebar_tags::tag_row(tag, self.busy, selected, cx).into_any_element()
            }
            LeftRow::Remote { remote } => {
                super::sidebar_remotes::remote_row(remote, self.busy, cx).into_any_element()
            }
            LeftRow::Empty(msg) => {
                let muted_fg = cx.theme().muted_foreground;
                h_flex()
                    .h(px(LEFT_ROW_H))
                    .flex_none()
                    .items_center()
                    .pl(px(4.0))
                    .text_xs()
                    .text_color(muted_fg)
                    .child(*msg)
                    .into_any_element()
            }
        }
    }
}

/// 段标题：折叠图标 + 名称 + 计数，整行可点折叠（固定 28px 高）
pub(super) fn section_header(
    title: &'static str,
    count: usize,
    collapsed: bool,
    sec: SidebarSection,
    busy: bool,
    cx: &mut Context<VcsView>,
) -> AnyElement {
    let theme = cx.theme();
    let muted_fg = theme.muted_foreground;
    let chev = if collapsed {
        IconName::ChevronRight
    } else {
        IconName::ChevronDown
    };
    let id = SharedString::from(format!(
        "vcs-side-section-{}",
        match sec {
            SidebarSection::Local => "local",
            SidebarSection::Remote => "remote",
            SidebarSection::Tag => "tag",
            SidebarSection::RemoteRepo => "remote-repo",
        }
    ));
    let hover_bg = theme.muted;
    let selector = id.to_string();

    let can_create = !matches!(sec, SidebarSection::Remote);
    let entity = cx.entity();

    let row = h_flex()
        .id(id)
        .debug_selector(move || selector.clone())
        .h(px(LEFT_ROW_H))
        .flex_none()
        .gap(px(4.0))
        .items_center()
        .px(px(2.0))
        .rounded(px(3.0))
        .cursor_pointer()
        .hover(move |this| this.bg(hover_bg))
        .on_click(cx.listener(move |this, _: &gpui_kit::ClickEvent, _, cx| {
            match sec {
                SidebarSection::Local => this.collapsed_local = !this.collapsed_local,
                SidebarSection::Remote => this.collapsed_remote = !this.collapsed_remote,
                SidebarSection::Tag => this.collapsed_tag = !this.collapsed_tag,
                SidebarSection::RemoteRepo => {
                    this.collapsed_remote_repos = !this.collapsed_remote_repos
                }
            }
            cx.notify();
        }))
        .child(Icon::new(chev).xsmall().text_color(muted_fg))
        .child(
            div()
                .flex_1()
                .text_xs()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(muted_fg)
                .child(format!("{title} ({count})")),
        );
    if can_create {
        row.context_menu(move |menu: PopupMenu, _, _| {
            let entity = entity.clone();
            menu.item(
                ramag_ui::menu_item_with_disabled(section_create_label(sec), busy).on_click(
                    move |_: &ClickEvent, window, app| {
                        open_sidebar_create_dialog(entity.clone(), sec, window, app);
                    },
                ),
            )
        })
        .into_any_element()
    } else {
        row.into_any_element()
    }
}

fn section_create_label(section: SidebarSection) -> &'static str {
    match section {
        SidebarSection::Local => "新建分支",
        SidebarSection::Tag => "新建 Tag",
        SidebarSection::RemoteRepo => "添加远程仓库",
        SidebarSection::Remote => "",
    }
}

pub(super) fn open_sidebar_create_dialog(
    view: Entity<VcsView>,
    section: SidebarSection,
    window: &mut Window,
    app: &mut App,
) {
    if view.read(app).busy {
        return;
    }
    match section {
        SidebarSection::Local => open_create_branch_dialog(view, window, app),
        SidebarSection::Tag => open_create_tag_dialog(view, window, app),
        SidebarSection::RemoteRepo => open_create_remote_dialog(view, window, app),
        SidebarSection::Remote => {}
    }
}

fn open_create_branch_dialog(view: Entity<VcsView>, window: &mut Window, app: &mut App) {
    let dialog_data = {
        let this = view.read(app);
        this.status
            .as_ref()
            .and_then(|status| status.head_commit.as_ref())
            .map(|_| {
                let head = this
                    .status
                    .as_ref()
                    .and_then(|status| status.head_branch.clone())
                    .unwrap_or_else(|| "(HEAD)".into());
                let local = this
                    .local_branches
                    .iter()
                    .map(|branch| (branch.name.clone(), branch.is_head))
                    .collect();
                let remote = this
                    .remote_branches
                    .iter()
                    .map(|branch| branch.name.clone())
                    .collect();
                (head, local, remote)
            })
    };
    let Some((head, local, remote)) = dialog_data else {
        view.update(app, |this, cx| {
            this.error = Some("请先创建首个提交，再新建分支".into());
            cx.notify();
        });
        return;
    };
    super::branch_picker::open_new_branch_dialog(view, head, local, remote, window, app);
}

/// 打开当前仓库的标签表单；正文随视口滚动，取消不会提交表单。
fn open_create_tag_dialog(view: Entity<VcsView>, window: &mut Window, app: &mut App) {
    let (name, message) = {
        let this = view.read(app);
        (
            this.create_tag_input.clone(),
            this.create_tag_message_input.clone(),
        )
    };
    for input in [&name, &message] {
        input.update(app, |state, cx| state.set_value("", window, cx));
    }
    let scroll = ScrollHandle::new();
    window.open_dialog(app, move |dialog, window, _| {
        let scroll = scroll.clone();
        let content_name = name.clone();
        let content_message = message.clone();
        dialog
            .title(ramag_ui::closable_dialog_title(
                "vcs-create-tag-close",
                "新建 Tag",
                |_, _| {},
            ))
            .close_button(false)
            .width(ramag_ui::responsive_dialog_width(window, 520.0))
            .margin_top(ramag_ui::responsive_dialog_top(window))
            .content(move |content, window, cx| {
                content.child(
                    create_dialog_body(window, &scroll)
                        .child(create_dialog_field(
                            "vcs-create-name",
                            "标签名称",
                            &content_name,
                            cx,
                        ))
                        .child(create_dialog_field(
                            "vcs-create-detail",
                            "备注（可选）",
                            &content_message,
                            cx,
                        )),
                )
            })
            .footer(create_dialog_footer(
                "vcs-create-tag",
                "创建",
                view.clone(),
                |this, cx| this.handle_create_tag(cx),
            ))
    });
}

/// 打开当前仓库的远程配置表单；标签始终可见，正文可在短窗口内滚动。
fn open_create_remote_dialog(view: Entity<VcsView>, window: &mut Window, app: &mut App) {
    let (name, url) = {
        let this = view.read(app);
        (
            this.create_remote_name_input.clone(),
            this.create_remote_url_input.clone(),
        )
    };
    for input in [&name, &url] {
        input.update(app, |state, cx| state.set_value("", window, cx));
    }
    let scroll = ScrollHandle::new();
    window.open_dialog(app, move |dialog, window, _| {
        let scroll = scroll.clone();
        let content_name = name.clone();
        let content_url = url.clone();
        dialog
            .title(ramag_ui::closable_dialog_title(
                "vcs-create-remote-close",
                "添加远程仓库",
                |_, _| {},
            ))
            .close_button(false)
            .width(ramag_ui::responsive_dialog_width(window, 560.0))
            .margin_top(ramag_ui::responsive_dialog_top(window))
            .content(move |content, window, cx| {
                content.child(
                    create_dialog_body(window, &scroll)
                        .child(create_dialog_field(
                            "vcs-create-name",
                            "远程名称",
                            &content_name,
                            cx,
                        ))
                        .child(create_dialog_field(
                            "vcs-create-detail",
                            "仓库地址（HTTPS / SSH）",
                            &content_url,
                            cx,
                        )),
                )
            })
            .footer(create_dialog_footer(
                "vcs-create-remote",
                "添加",
                view.clone(),
                |this, cx| this.handle_create_remote(cx),
            ))
    });
}

/// 给标题、操作区和内边距预留高度，两个字段只在独立正文中滚动。
fn create_dialog_body(window: &Window, scroll: &ScrollHandle) -> gpui_kit::Stateful<gpui_kit::Div> {
    let body_height = (ramag_ui::responsive_dialog_max_height(window) - px(140.0)).max(px(48.0));
    v_flex()
        .id("vcs-create-fields")
        .debug_selector(|| "vcs-create-fields".into())
        .w_full()
        .min_w_0()
        .h(body_height)
        .max_h(body_height)
        .overflow_y_scroll()
        .track_scroll(scroll)
        .vertical_scrollbar(scroll)
        .gap(px(8.0))
}

/// 字段标签独立于占位文字，输入后仍能辨认用途；文字和控件保持同一宽度。
fn create_dialog_field(
    id: &'static str,
    label: &'static str,
    input: &Entity<InputState>,
    cx: &App,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .min_w_0()
        .flex_none()
        .gap(px(4.0))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(
            div()
                .debug_selector(move || id.into())
                .w_full()
                .min_w_0()
                .child(Input::new(input).w_full().min_w_0().small()),
        )
}

/// 复用共享操作区，使取消和提交在窄窗口中保持独立的命中范围。
fn create_dialog_footer(
    id: &'static str,
    label: &'static str,
    view: gpui_kit::Entity<VcsView>,
    submit: impl Fn(&mut VcsView, &mut Context<VcsView>) + 'static,
) -> impl IntoElement {
    ramag_ui::dialog_action_footer(
        ramag_ui::clickable_button(format!("{id}-cancel"))
            .debug_selector(move || format!("{id}-cancel"))
            .ghost()
            .small()
            .label("取消")
            .on_click(|_: &ClickEvent, window, app| window.close_dialog(app)),
        ramag_ui::clickable_button(id)
            .debug_selector(move || id.into())
            .primary()
            .small()
            .label(label)
            .on_click(move |_: &ClickEvent, window, app| {
                view.update(app, |this, cx| submit(this, cx));
                window.close_dialog(app);
            }),
    )
}
