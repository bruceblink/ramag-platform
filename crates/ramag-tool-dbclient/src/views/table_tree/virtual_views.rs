//! Virtual views 元数据状态与 DataGrip 风格对象树行。

use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use ramag_domain::entities::{VirtualView, contains_case_insensitive};
use tracing::error;

use super::server_objects::ExplorerRowKind;
use super::{TableTreePanel, TreeEvent, row::TreeRow};

#[derive(Default)]
pub(super) struct VirtualViewsState {
    pub(super) loading: bool,
    pub(super) views: Vec<VirtualView>,
    pub(super) error: Option<String>,
    pub(super) is_expanded: bool,
    pub(super) request_generation: u64,
}

impl VirtualViewsState {
    /// 切换连接时清空旧会话快照，避免把上一个连接的虚拟节点带入新连接。
    pub(super) fn reset_for_connection(&mut self) {
        self.loading = false;
        self.views.clear();
        self.error = None;
        self.is_expanded = true;
        self.request_generation = self.request_generation.wrapping_add(1);
    }
}

impl TableTreePanel {
    /// 加载只读虚拟视图目录；连接代际检查阻止旧连接结果回写。
    pub(super) fn load_virtual_views(&mut self, cx: &mut Context<Self>) {
        let Some(conn) = self.connection.clone() else {
            return;
        };
        self.virtual_views.request_generation =
            self.virtual_views.request_generation.wrapping_add(1);
        let request_generation = self.virtual_views.request_generation;
        let metadata_generation = self.metadata_generation;
        self.virtual_views.loading = true;
        self.virtual_views.error = None;
        self.invalidate_tree_rows();
        cx.notify();

        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.list_virtual_views(&conn).await;
            if let Err(error) = &result {
                error!(
                    operation = "sql_metadata_virtual_views",
                    connection_id = %conn.id,
                    driver = ?conn.driver,
                    connection = %conn.name,
                    error = %error,
                    "load virtual views failed"
                );
            }
            let _ = this.update(cx, |this, cx| {
                let is_current = this.metadata_generation == metadata_generation
                    && this.virtual_views.request_generation == request_generation
                    && this.connection.as_ref().map(|current| &current.id) == Some(&conn.id);
                if !is_current {
                    return;
                }
                this.virtual_views.loading = false;
                match result {
                    Ok(views) => this.virtual_views.views = views,
                    Err(error) => this.virtual_views.error = Some(error.to_string()),
                }
                this.invalidate_tree_rows();
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn toggle_virtual_views(&mut self, cx: &mut Context<Self>) {
        self.virtual_views.is_expanded = !self.virtual_views.is_expanded;
        self.invalidate_tree_rows();
        cx.notify();
    }
}

/// 将只读虚拟视图目录转换为对象树行，并保持空结果和刷新失败可见。
pub(super) fn append_rows(rows: &mut Vec<TreeRow>, state: &VirtualViewsState, filter: &str) {
    let has_filter = !filter.is_empty();
    let root_matches = !has_filter
        || contains_case_insensitive("Virtual views", filter)
        || state.views.iter().any(|view| {
            contains_case_insensitive(&view.name, filter)
                || view
                    .detail
                    .as_deref()
                    .is_some_and(|detail| contains_case_insensitive(detail, filter))
        });
    if !root_matches {
        return;
    }

    let is_expanded = state.is_expanded || has_filter;
    rows.push(TreeRow::ServerObject {
        kind: ExplorerRowKind::VirtualRoot,
        group_index: 0,
        label: "Virtual views".into(),
        detail: None,
        count: state.views.len(),
        is_expanded,
    });
    if !is_expanded {
        return;
    }
    if state.loading && state.views.is_empty() {
        rows.push(TreeRow::SchemaPlaceholder {
            text: "加载 Virtual views…".into(),
            is_error: false,
        });
    } else if state.loading {
        rows.push(TreeRow::SchemaPlaceholder {
            text: "正在刷新 Virtual views…".into(),
            is_error: false,
        });
    }
    if let Some(error) = &state.error {
        rows.push(TreeRow::SchemaPlaceholder {
            text: format!("Virtual views 刷新失败：{error}"),
            is_error: true,
        });
    }

    for (index, view) in state.views.iter().enumerate() {
        if has_filter
            && !contains_case_insensitive(&view.name, filter)
            && !view
                .detail
                .as_deref()
                .is_some_and(|detail| contains_case_insensitive(detail, filter))
        {
            continue;
        }
        rows.push(TreeRow::ServerObject {
            kind: ExplorerRowKind::VirtualItem,
            group_index: index,
            label: view.name.clone(),
            detail: view.detail.clone(),
            count: 0,
            is_expanded: false,
        });
    }
    if !state.loading && state.error.is_none() && state.views.is_empty() {
        rows.push(TreeRow::SchemaPlaceholder {
            text: "（无会话）".into(),
            is_error: false,
        });
    }
}

/// 渲染 Virtual views 节点；子节点只打开只读查询，不进入表编辑路径。
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
        unreachable!("virtual view renderer received another row kind");
    };
    let indent = if matches!(kind, ExplorerRowKind::VirtualRoot) {
        8.0
    } else {
        28.0
    };
    let chevron = if *is_expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    let icon = if matches!(kind, ExplorerRowKind::VirtualRoot) {
        IconName::Network
    } else {
        IconName::Eye
    };
    let label = if *count == 0 {
        label.clone()
    } else {
        format!("{label} {count}")
    };
    let fallback_label = label.clone();
    let muted_bg = cx.theme().muted;
    let row_id = format!("virtual-view-{:?}-{}-{}", kind, group_index, label);
    let detail = detail.clone();
    let mut view = h_flex()
        .id(row_id)
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
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_xs()
                        .text_color(fg)
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(label.clone()),
                )
                .children(detail.map(|value| {
                    div()
                        .text_xs()
                        .text_color(muted_fg)
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(value)
                })),
        );

    match kind {
        ExplorerRowKind::VirtualRoot => {
            view = view.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.toggle_virtual_views(cx);
            }));
        }
        ExplorerRowKind::VirtualItem => {
            let name = panel
                .virtual_views
                .views
                .get(*group_index)
                .map(|view| view.name.clone())
                .unwrap_or_else(|| fallback_label.clone());
            view = view.on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                cx.emit(TreeEvent::VirtualViewSelected { name: name.clone() });
            }));
        }
        _ => unreachable!("virtual renderer received server object kind"),
    }
    view.into_any_element()
}
