//! ConnectionListPanel 渲染：header（搜索 + 新建按钮）+ body（行列表 / 空状态）

use std::ops::Range;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _, button::ButtonVariants as _, v_flex,
};
use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, ParentElement, Render,
    Styled, Window, div, prelude::FluentBuilder, px, uniform_list,
};

use super::row::{connection_header, connection_row};
use super::{ConnectionListPanel, ListEvent, syncable_target_ids};

impl Render for ConnectionListPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.loading && self.loaded_revision != self.service.revision() {
            self.refresh(cx);
        }
        // 首次显示即聚焦搜索框，进入页面直接可打字过滤
        if !self.focused_search_once {
            self.focused_search_once = true;
            self.search.update(cx, |state, cx| state.focus(window, cx));
        }
        let theme = cx.theme();
        let muted_fg = theme.muted_foreground;
        let fg = theme.foreground;
        let accent = theme.accent;
        let border = theme.border;
        let row_hover = theme.muted;

        // 行密度按面板宽度分档：固定列在 800px 窗口下已挤占近满，窄窗口隐藏次要列。
        // 面板宽 ≈ 窗口宽 - 左侧活动栏(约 52px)；用窗口宽近似，断点留足余量
        let width = f32::from(window.viewport_size().width);
        let density = if width < 900.0 {
            super::row::RowDensity::Narrow
        } else if width < 1120.0 {
            super::row::RowDensity::Medium
        } else {
            super::row::RowDensity::Full
        };

        let total = self.connections.len();
        let loading = self.loading;
        let visible = self.filtered_indices();
        let visible_count = visible.len();
        let connections = self.connections.clone();
        let syncable_targets = syncable_target_ids(&connections);

        let count = if self.query.trim().is_empty() {
            format!("{total} 个数据源")
        } else {
            format!("{visible_count} / {total} 个数据源")
        };
        let header = ramag_ui::pulse_ui::pulse_page_title("数据源管理", Some(count), cx)
            .id("connection-list-page-title")
            .max_w(px(1080.0))
            .debug_selector(|| "connection-list-page-title".into());
        let toolbar = ramag_ui::pulse_ui::pulse_home_toolbar(cx)
            .debug_selector(|| "connection-list-toolbar".into())
            .w_full()
            .flex_none()
            .px(px(14.0))
            .py(px(10.0))
            .border_b_1()
            .border_color(border)
            .child(
                div().flex_1().min_w_0().child(
                    div()
                        .debug_selector(|| "connection-search-field".into())
                        .w_full()
                        .max_w(px(400.0))
                        .child(
                            ramag_ui::cleanable_input(
                                &self.search,
                                "connection-search-clear",
                                false,
                                cx,
                            )
                            .small()
                            .prefix(Icon::new(IconName::Search).small().text_color(muted_fg)),
                        ),
                ),
            )
            .child(
                ramag_ui::clickable_button("add-connection")
                    .debug_selector(|| "add-connection".into())
                    .primary()
                    .small()
                    .flex_none()
                    .icon(IconName::Plus)
                    .label("新建连接")
                    .on_click(cx.listener(|_this, _: &ClickEvent, _, cx| {
                        cx.emit(ListEvent::RequestNew);
                    })),
            );

        let body: AnyElement = if loading {
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .p(px(16.0))
                .child(
                    div()
                        .id("connection-list-loading-notice")
                        .debug_selector(|| "connection-list-loading-notice".into())
                        .w_full()
                        .max_w(px(1080.0))
                        .child(ramag_ui::pulse_ui::pulse_status_notice(
                            ramag_ui::pulse_ui::PulseStatus::Warming,
                            "正在读取本地连接列表…",
                            cx,
                        )),
                )
                .into_any_element()
        } else if total == 0 {
            // 加载失败不能显示为空状态。
            if let Some(err) = self.load_error.clone() {
                status_with_retry("connection-list-error-notice", err, cx)
            } else {
                empty_state(cx).into_any_element()
            }
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
                        .child(format!("没有匹配「{}」的连接", self.query)),
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
                "connection-list-rows",
                visible_count,
                cx.processor({
                    let connections = connections.clone();
                    let visible = visible.clone();
                    let syncable_targets = syncable_targets.clone();
                    move |this, range: Range<usize>, _window, cx| {
                        range
                            .map(|row_index| {
                                let connection_index = visible[row_index];
                                let conn = connections[connection_index].clone();
                                let is_selected = this.selected.as_ref() == Some(&conn.id);
                                let show_sync = syncable_targets.contains(&conn.id);
                                let version = this.versions.get(&conn.id).cloned();
                                connection_row(
                                    row_index,
                                    conn,
                                    is_selected,
                                    show_sync,
                                    version,
                                    density,
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
            let content = v_flex()
                .size_full()
                .min_h_0()
                .p_0()
                .overflow_hidden()
                .when_some(self.load_error.clone(), |content, error| {
                    content.child(
                        div()
                            .id("connection-list-error-notice")
                            .debug_selector(|| "connection-list-error-notice".into())
                            .w_full()
                            .flex_none()
                            .p(px(10.0))
                            .child(ramag_ui::pulse_ui::pulse_status_notice(
                                ramag_ui::pulse_ui::PulseStatus::Failed,
                                error,
                                cx,
                            ))
                            .child(
                                ramag_ui::clickable_button("conn-list-retry")
                                    .small()
                                    .label("重试")
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.refresh(cx);
                                    })),
                            ),
                    )
                })
                .child(connection_header(density, cx))
                .child(rows);
            content.into_any_element()
        };

        ramag_ui::pulse_ui::pulse_home_frame(cx)
            .child(header)
            .child(
                ramag_ui::pulse_ui::pulse_home_panel(cx)
                    .id("connection-list-panel")
                    .debug_selector(|| "connection-list-panel".into())
                    .flex_1()
                    .max_w(px(1080.0))
                    .min_h_0()
                    .p_0()
                    .overflow_hidden()
                    .child(toolbar)
                    .child(div().flex_1().min_h_0().overflow_hidden().child(body)),
            )
    }
}

/// Keep load failures visible while preserving an already loaded connection list.
fn status_with_retry(
    id: &'static str,
    message: String,
    cx: &mut Context<ConnectionListPanel>,
) -> AnyElement {
    v_flex()
        .id(id)
        .debug_selector(move || id.into())
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .p(px(16.0))
        .child(
            div()
                .w_full()
                .max_w(px(1080.0))
                .child(ramag_ui::pulse_ui::pulse_status_notice(
                    ramag_ui::pulse_ui::PulseStatus::Failed,
                    message,
                    cx,
                )),
        )
        .child(
            ramag_ui::clickable_button("conn-list-retry")
                .debug_selector(|| "conn-list-retry".into())
                .small()
                .label("重试")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.refresh(cx);
                })),
        )
        .into_any_element()
}

/// 空状态：只放一个居中主按钮
fn empty_state(cx: &mut Context<ConnectionListPanel>) -> impl IntoElement {
    v_flex()
        .id("connection-list-empty-state")
        .debug_selector(|| "connection-list-empty-state".into())
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .px(px(24.0))
        .text_center()
        .child(ramag_ui::icons::database().large())
        .child(ramag_ui::pulse_ui::pulse_display_heading(
            "暂无数据源",
            16.0,
            cx,
        ))
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("新建连接后可浏览对象并执行查询。"),
        )
        .child(
            ramag_ui::clickable_button("empty-add")
                .debug_selector(|| "connection-list-empty-add".into())
                .primary()
                .icon(IconName::Plus)
                .label("新建")
                .on_click(cx.listener(|_this, _: &ClickEvent, _, cx| {
                    cx.emit(ListEvent::RequestNew);
                })),
        )
}
