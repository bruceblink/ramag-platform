//! Server Objects 元数据状态、加载流程和对象树行渲染。

use std::collections::HashSet;

use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable as _, h_flex};
use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use ramag_domain::entities::{ServerObjectGroup, contains_case_insensitive};
use tracing::error;

use super::{TableTreePanel, row::TreeRow};

#[derive(Default)]
pub(super) struct ServerObjectsState {
    pub(super) loading: bool,
    pub(super) groups: Vec<ServerObjectGroup>,
    pub(super) error: Option<String>,
    pub(super) is_expanded: bool,
    pub(super) open_groups: HashSet<String>,
    pub(super) groups_initialized: bool,
    pub(super) request_generation: u64,
}

impl ServerObjectsState {
    /// 切换连接时清空旧对象，避免把上一个数据库的用户或 collation 显示到新连接。
    pub(super) fn reset_for_connection(&mut self) {
        self.loading = false;
        self.groups.clear();
        self.error = None;
        self.is_expanded = true;
        self.open_groups.clear();
        self.groups_initialized = false;
        self.request_generation = self.request_generation.wrapping_add(1);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExplorerRowKind {
    Root,
    Group,
    Item,
    VirtualRoot,
    VirtualItem,
}

impl TableTreePanel {
    /// 读取 Server Objects；请求代际和连接 ID 同时校验，阻止旧连接结果写回。
    pub(super) fn load_server_objects(&mut self, cx: &mut Context<Self>) {
        let Some(conn) = self.connection.clone() else {
            return;
        };
        self.server_objects.request_generation =
            self.server_objects.request_generation.wrapping_add(1);
        let request_generation = self.server_objects.request_generation;
        let metadata_generation = self.metadata_generation;
        self.server_objects.loading = true;
        self.server_objects.error = None;
        self.invalidate_tree_rows();
        cx.notify();

        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.list_server_objects(&conn).await;
            if let Err(error) = &result {
                error!(
                    operation = "sql_metadata_server_objects",
                    connection_id = %conn.id,
                    driver = ?conn.driver,
                    connection = %conn.name,
                    error = %error,
                    "load server objects failed"
                );
            }
            let _ = this.update(cx, |this, cx| {
                let is_current = this.metadata_generation == metadata_generation
                    && this.server_objects.request_generation == request_generation
                    && this.connection.as_ref().map(|current| &current.id) == Some(&conn.id);
                if !is_current {
                    return;
                }
                this.server_objects.loading = false;
                match result {
                    Ok(groups) => {
                        let names: HashSet<&str> =
                            groups.iter().map(|group| group.name.as_str()).collect();
                        if !this.server_objects.groups_initialized {
                            this.server_objects.open_groups =
                                groups.iter().map(|group| group.name.clone()).collect();
                            this.server_objects.groups_initialized = true;
                        } else {
                            this.server_objects
                                .open_groups
                                .retain(|name| names.contains(name.as_str()));
                        }
                        this.server_objects.groups = groups;
                    }
                    Err(error) => {
                        this.server_objects.error = Some(error.to_string());
                    }
                }
                this.invalidate_tree_rows();
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn toggle_server_objects(&mut self, cx: &mut Context<Self>) {
        self.server_objects.is_expanded = !self.server_objects.is_expanded;
        self.invalidate_tree_rows();
        cx.notify();
    }

    pub(super) fn toggle_server_object_group(
        &mut self,
        group_name: String,
        cx: &mut Context<Self>,
    ) {
        if !self.server_objects.open_groups.insert(group_name.clone()) {
            self.server_objects.open_groups.remove(&group_name);
        }
        self.invalidate_tree_rows();
        cx.notify();
    }
}

/// 将 Server Objects 状态展开成与 schema/table 行相同的扁平树行。
pub(super) fn append_rows(rows: &mut Vec<TreeRow>, state: &ServerObjectsState, filter: &str) {
    let has_filter = !filter.is_empty();
    let root_matches = !has_filter
        || contains_case_insensitive("Server Objects", filter)
        || state.groups.iter().any(|group| {
            contains_case_insensitive(&group.name, filter)
                || group.items.iter().any(|item| {
                    contains_case_insensitive(&item.name, filter)
                        || item
                            .detail
                            .as_deref()
                            .is_some_and(|detail| contains_case_insensitive(detail, filter))
                })
        });
    if !root_matches {
        return;
    }

    let is_expanded = state.is_expanded || has_filter;
    rows.push(TreeRow::ServerObject {
        kind: ExplorerRowKind::Root,
        group_index: 0,
        label: "Server Objects".into(),
        detail: None,
        count: 0,
        is_expanded,
    });
    if !is_expanded {
        return;
    }
    if state.loading && state.groups.is_empty() {
        rows.push(TreeRow::SchemaPlaceholder {
            text: "加载 Server Objects…".into(),
            is_error: false,
        });
    } else if state.loading {
        rows.push(TreeRow::SchemaPlaceholder {
            text: "正在刷新 Server Objects…".into(),
            is_error: false,
        });
    }
    if let Some(error) = &state.error {
        rows.push(TreeRow::SchemaPlaceholder {
            text: format!("Server Objects 刷新失败：{error}"),
            is_error: true,
        });
    }

    for (group_index, group) in state.groups.iter().enumerate() {
        let group_matches = !has_filter
            || contains_case_insensitive(&group.name, filter)
            || group.items.iter().any(|item| {
                contains_case_insensitive(&item.name, filter)
                    || item
                        .detail
                        .as_deref()
                        .is_some_and(|detail| contains_case_insensitive(detail, filter))
            });
        if !group_matches {
            continue;
        }
        let group_expanded = has_filter || state.open_groups.contains(&group.name);
        rows.push(TreeRow::ServerObject {
            kind: ExplorerRowKind::Group,
            group_index,
            label: group.name.clone(),
            detail: None,
            count: group.items.len(),
            is_expanded: group_expanded,
        });
        if !group_expanded {
            continue;
        }
        for item in &group.items {
            if has_filter
                && !contains_case_insensitive(&item.name, filter)
                && !item
                    .detail
                    .as_deref()
                    .is_some_and(|detail| contains_case_insensitive(detail, filter))
            {
                continue;
            }
            rows.push(TreeRow::ServerObject {
                kind: ExplorerRowKind::Item,
                group_index,
                label: item.name.clone(),
                detail: item.detail.clone(),
                count: 0,
                is_expanded: false,
            });
        }
    }
}

/// 根据行层级生成 DataGrip 风格的紧凑对象节点；子项的二次点击复制名称。
struct ServerObjectRowSpec<'a> {
    kind: ExplorerRowKind,
    group_index: usize,
    label: &'a str,
    detail: Option<&'a str>,
    count: usize,
    is_expanded: bool,
}

struct ServerObjectRowPalette {
    muted_fg: gpui_kit::Hsla,
    fg: gpui_kit::Hsla,
}

fn render_row(
    panel: &TableTreePanel,
    spec: ServerObjectRowSpec<'_>,
    palette: ServerObjectRowPalette,
    cx: &mut Context<TableTreePanel>,
) -> AnyElement {
    let ServerObjectRowSpec {
        kind,
        group_index,
        label,
        detail,
        count,
        is_expanded,
    } = spec;
    let ServerObjectRowPalette { muted_fg, fg } = palette;
    let chevron = if is_expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    let indent = match kind {
        ExplorerRowKind::Root | ExplorerRowKind::VirtualRoot => 8.0,
        ExplorerRowKind::Group => 28.0,
        ExplorerRowKind::Item | ExplorerRowKind::VirtualItem => 48.0,
    };
    let icon = match kind {
        ExplorerRowKind::Root => IconName::HardDrive,
        ExplorerRowKind::Group => IconName::Folder,
        ExplorerRowKind::Item => IconName::File,
        ExplorerRowKind::VirtualRoot => IconName::Network,
        ExplorerRowKind::VirtualItem => IconName::Eye,
    };
    let row_id = format!("server-object-{:?}-{}-{}", kind, group_index, label);
    let row_selector = format!("server-object-row-{:?}-{}-{}", kind, group_index, label);
    let label_selector = format!("server-object-label-{:?}-{}-{}", kind, group_index, label);
    let detail_selector = format!("server-object-detail-{:?}-{}-{}", kind, group_index, label);
    let label = if count == 0 {
        label.to_string()
    } else {
        format!("{label} {count}")
    };
    let copy_value = label.clone();
    let detail = detail.map(str::to_string);
    let muted_bg = cx.theme().muted;
    let mut row = h_flex()
        .id(row_id)
        .debug_selector(move || row_selector.clone())
        .w_full()
        .h(px(28.0))
        .flex_none()
        .items_center()
        .gap_1()
        .pl(px(indent))
        .pr_2()
        .cursor_pointer()
        .hover(move |this| this.bg(muted_bg))
        .child(Icon::new(chevron).xsmall().text_color(muted_fg))
        .child(Icon::new(icon).xsmall().text_color(muted_fg))
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .gap_1()
                .child(
                    div()
                        .debug_selector(move || label_selector.clone())
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(fg)
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(label),
                )
                .children(detail.as_ref().map(|value| {
                    let detail_selector = detail_selector.clone();
                    div()
                        .debug_selector(move || detail_selector.clone())
                        .flex_none()
                        .text_xs()
                        .text_color(muted_fg)
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(format!("· {value}"))
                })),
        );

    match kind {
        ExplorerRowKind::Root => {
            row = row.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.toggle_server_objects(cx);
            }));
        }
        ExplorerRowKind::Group => {
            let group_name = panel
                .server_objects
                .groups
                .get(group_index)
                .map(|group| group.name.clone())
                .unwrap_or_default();
            row = row.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.toggle_server_object_group(group_name.clone(), cx);
            }));
        }
        ExplorerRowKind::Item => {
            row = row.on_click(cx.listener(move |_, event: &ClickEvent, window, cx| {
                if event.modifiers().secondary()
                    && ramag_ui::is_primary_modifier_double_click(event)
                {
                    ramag_ui::copy_text_with_notification(copy_value.clone(), window, cx);
                }
            }));
        }
        ExplorerRowKind::VirtualRoot | ExplorerRowKind::VirtualItem => {
            unreachable!("virtual rows use the virtual view renderer");
        }
    }
    row.into_any_element()
}

pub(super) fn render_tree_row(
    panel: &TableTreePanel,
    row: &TreeRow,
    muted_fg: gpui_kit::Hsla,
    fg: gpui_kit::Hsla,
    cx: &mut Context<TableTreePanel>,
) -> AnyElement {
    let TreeRow::ServerObject {
        kind,
        group_index,
        label,
        detail,
        count,
        is_expanded,
        ..
    } = row
    else {
        unreachable!("server object renderer received another row kind");
    };
    render_row(
        panel,
        ServerObjectRowSpec {
            kind: *kind,
            group_index: *group_index,
            label,
            detail: detail.as_deref(),
            count: *count,
            is_expanded: *is_expanded,
        },
        ServerObjectRowPalette { muted_fg, fg },
        cx,
    )
}
