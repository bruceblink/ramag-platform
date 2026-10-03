//! DbClientView 渲染：顶部连接 Tab Bar + 中心内容（picker / session）

use gpui_kit::component::{
    ActiveTheme, IconName, Sizable as _, button::ButtonVariants as _, h_flex,
    scroll::ScrollableElement as _, v_flex,
};
use gpui_kit::{
    AnyView, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, Styled, Window, div, prelude::*, px,
};

use super::{CenterMode, DbClientView};
use ramag_domain::entities::ConnectionId;

fn session_tab_status_colors(
    stale: bool,
    health: Option<(bool, bool)>,
    warning: gpui_kit::Hsla,
    danger: gpui_kit::Hsla,
    success: gpui_kit::Hsla,
    muted: gpui_kit::Hsla,
) -> (gpui_kit::Hsla, &'static str, gpui_kit::Hsla) {
    if stale {
        return (warning, "需重连", warning);
    }

    match health {
        None => (muted, "未连接", muted),
        Some((true, _)) => (warning, "连接中", warning),
        Some((false, true)) => (danger, "连接失败", danger),
        Some((false, false)) => (success, "已连接", success),
    }
}

/// 根据窗口宽度限制连接标签标题，给类型、状态和关闭按钮保留稳定空间。
///
/// 返回值只影响标题显示区域；标签本身仍可以通过横向滚动容器访问全部内容。
fn session_tab_title_max_width(viewport_width: f32) -> f32 {
    if viewport_width < 720.0 {
        140.0
    } else if viewport_width < 1120.0 {
        180.0
    } else {
        240.0
    }
}

/// 渲染受约束的连接标签标题，并保留完整名称的悬停提示。
fn render_session_tab_title(
    title: String,
    id: SharedString,
    max_width: f32,
    text_color: gpui_kit::Hsla,
) -> impl IntoElement {
    let title_for_tooltip = title.clone();
    let debug_selector = format!("{id}-bounds");
    div()
        .id(id)
        .debug_selector(move || debug_selector.clone())
        .min_w_0()
        .max_w(px(max_width))
        .text_xs()
        .text_color(text_color)
        .overflow_hidden()
        .text_ellipsis()
        .tooltip(move |window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(title_for_tooltip.clone()).build(window, cx)
        })
        .child(title)
}

fn session_pulse_status(
    stale: bool,
    health: Option<(bool, bool)>,
) -> (ramag_ui::pulse_ui::PulseStatus, &'static str) {
    if stale {
        return (ramag_ui::pulse_ui::PulseStatus::Stale, "需重连");
    }

    match health {
        None => (ramag_ui::pulse_ui::PulseStatus::Unavailable, "未连接"),
        Some((true, _)) => (ramag_ui::pulse_ui::PulseStatus::Warming, "连接中"),
        Some((false, true)) => (ramag_ui::pulse_ui::PulseStatus::Failed, "连接失败"),
        Some((false, false)) => (ramag_ui::pulse_ui::PulseStatus::Current, "已连接"),
    }
}

fn render_session_context_header(
    title: String,
    subtitle: String,
    status: ramag_ui::pulse_ui::PulseStatus,
    status_label: &'static str,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    div()
        .id("dbclient-session-header")
        .debug_selector(|| "dbclient-session-header".into())
        .w_full()
        .min_w_0()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_2()
        .px_4()
        .py_2()
        .border_b_1()
        .border_color(cx.theme().border.opacity(0.65))
        .bg(cx.theme().background)
        .child(
            div()
                .id("dbclient-session-header-title")
                .debug_selector(|| "dbclient-session-header-title".into())
                .flex_1()
                .min_w(px(160.0))
                .min_w_0()
                .overflow_hidden()
                .child(ramag_ui::pulse_ui::pulse_page_title(
                    title,
                    Some(subtitle),
                    cx,
                )),
        )
        .child(
            div()
                .id("dbclient-session-header-status")
                .debug_selector(|| "dbclient-session-header-status".into())
                .flex_none()
                .child(ramag_ui::pulse_ui::pulse_status_badge_with_label(
                    status,
                    status_label,
                    cx,
                )),
        )
}

impl Render for DbClientView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 异步失败提示：Render 持 Window 时统一推送（与各面板同款）
        if let Some(n) = self.pending_notification.take() {
            ramag_ui::push_responsive_notification(window, n, cx);
        }
        // 跨重启恢复：首帧只建占位槽（不连库），仅上次激活的那个立即建会话；
        // 其余标签首次点击时才真正连接，避免恢复 N 个标签时全部拉元数据 / SCAN
        if let Some((configs, active_id)) = self.pending_restore.take() {
            let active_index = self
                .restore_allowed
                .then(|| {
                    super::queue_restored_session_slots(
                        &mut self.sessions,
                        configs,
                        active_id.as_ref(),
                    )
                })
                .flatten();
            self.restore_allowed = false;
            if let Some(idx) = active_index {
                self.active_session = Some(idx);
                self.center = CenterMode::Session;
                self.materialize_slot(idx, window, cx);
            }
        }
        // 中央区为激活会话但实体尚未创建（如恢复兜底路径）：此处有 Window，补建
        if matches!(self.center, CenterMode::Session)
            && let Some(idx) = self.active_session
            && self
                .sessions
                .get(idx)
                .is_some_and(|s| s.entity.is_none() && !s.stale)
        {
            self.materialize_slot(idx, window, cx);
        }
        let theme = cx.theme();
        let muted_fg = theme.muted_foreground;
        let fg = theme.foreground;
        let border = theme.border;
        let secondary_bg = theme.secondary;
        let muted_bg = theme.muted;
        let accent = theme.accent;
        let bg = theme.background;
        let warning = theme.warning;
        let danger = theme.danger;
        let success = theme.success;
        let tab_title_max_width =
            session_tab_title_max_width(f32::from(window.viewport_size().width));

        let active = self.active_session;

        /// Tab 条目的展示快照
        struct TabInfo {
            id: ConnectionId,
            title: String,
            kind_label: &'static str,
            is_active: bool,
            /// 实体存在时记录元数据加载快照；未实例化的恢复标签为 None。
            health: Option<(bool, bool)>,
            stale: bool,
            production: bool,
        }
        let session_titles: Vec<TabInfo> = self
            .sessions
            .iter()
            .enumerate()
            .map(|(i, s)| TabInfo {
                id: s.config.id.clone(),
                title: s.config.name.clone(),
                kind_label: super::driver_kind_label(s.config.driver),
                is_active: Some(i) == active,
                health: s.entity.as_ref().map(|entity| entity.health(cx)),
                stale: s.stale,
                production: s.config.production,
            })
            .collect::<Vec<_>>();

        let on_picker_active = matches!(self.center, CenterMode::ConnectionPicker);

        let mut tab_bar = h_flex()
            .w_full()
            .flex_none()
            .border_b_1()
            .border_color(border)
            .bg(secondary_bg);

        let picker_btn_active = on_picker_active;
        let mut picker_tab = h_flex()
            .id("picker-tab")
            .items_center()
            .gap_2()
            .px_3()
            .py(px(7.0))
            .border_r_1()
            .border_color(border)
            .cursor_pointer()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.show_picker(cx);
            }))
            .child(
                ramag_ui::icons::database()
                    .small()
                    .text_color(if picker_btn_active { fg } else { muted_fg }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(if picker_btn_active { fg } else { muted_fg })
                    .child("数据源管理"),
            );

        if picker_btn_active {
            let mut active_bg = accent;
            active_bg.a = 0.15;
            picker_tab = picker_tab.bg(active_bg);
        } else {
            picker_tab = picker_tab.hover(move |this| this.bg(muted_bg));
        }
        tab_bar = tab_bar.child(picker_tab);

        // 右侧 session tabs 横向滚动，不挤压 picker tab
        let mut session_strip = h_flex()
            .id("conn-tabs-scroll")
            .flex_1()
            .min_w_0()
            .overflow_x_scroll()
            .track_scroll(&self.sessions_scroll)
            .horizontal_scrollbar(&self.sessions_scroll);

        for info in session_titles {
            let TabInfo {
                id: session_id,
                title,
                kind_label,
                is_active,
                health,
                stale,
                production,
            } = info;
            let tab_id = SharedString::from(format!("conn-tab-{session_id}"));
            let close_id = SharedString::from(format!("conn-tab-close-{session_id}"));
            let title_id = SharedString::from(format!("conn-tab-title-{session_id}"));
            let close_session_id = session_id.clone();
            let select_session_id = session_id.clone();

            // 标签状态与会话实体绑定：已完成首次连接显示绿色，未实例化的恢复标签明确显示未连接。
            let (dot_color, status_label, status_color) =
                session_tab_status_colors(stale, health, warning, danger, success, muted_fg);
            let title_element = render_session_tab_title(
                title,
                title_id,
                tab_title_max_width,
                if is_active { fg } else { muted_fg },
            );

            let mut tab = h_flex()
                .id(tab_id)
                .flex_none()
                .items_center()
                .gap_2()
                .px_3()
                .py(px(7.0))
                .border_r_1()
                .border_color(border)
                .cursor_pointer()
                .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(dot_color))
                .child(title_element)
                .child(div().text_xs().text_color(muted_fg).child(kind_label))
                .child(div().text_xs().text_color(status_color).child(status_label))
                // 生产徽标持续可见，与 driver 层拦截、写入口禁用保持同一语义。
                .when(production, |tab| {
                    let mut chip_bg = danger;
                    chip_bg.a = 0.15;
                    tab.child(
                        div()
                            .flex_none()
                            .px(px(6.0))
                            .py(px(1.0))
                            .rounded(px(4.0))
                            .bg(chip_bg)
                            .text_xs()
                            .text_color(danger)
                            .child(ramag_ui::PRODUCTION_BADGE_LABEL),
                    )
                })
                .child(
                    ramag_ui::clickable_button(close_id)
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .tooltip("关闭")
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.close_session_by_id(close_session_id.clone(), cx);
                        })),
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.select_session_by_id(select_session_id.clone(), window, cx);
                }));

            if is_active && !on_picker_active {
                let mut active_bg = accent;
                active_bg.a = 0.15;
                tab = tab.bg(active_bg);
            } else {
                tab = tab.hover(move |this| this.bg(muted_bg));
            }

            session_strip = session_strip.child(tab);
        }

        tab_bar = tab_bar.child(session_strip);

        // stale 槽显示"配置已更新"面板（暂停查询与写入，等待用户一键重连）
        let center_view: gpui_kit::AnyElement = match &self.center {
            CenterMode::Session => {
                match active.and_then(|i| self.sessions.get(i).map(|s| (i, s))) {
                    Some((_, slot)) if slot.stale => self
                        .render_stale_panel(slot.config.id.clone(), &slot.config.name, cx)
                        .into_any_element(),
                    Some((_, slot)) => match &slot.entity {
                        Some(entity) => {
                            let view: AnyView = entity.to_any_view();
                            div().size_full().child(view).into_any_element()
                        }
                        // 兜底：实体缺失（本帧顶部已尝试补建），显示占位避免空白
                        None => v_flex()
                            .size_full()
                            .items_center()
                            .justify_center()
                            .p(px(16.0))
                            .child(div().w_full().max_w(px(360.0)).child(
                                ramag_ui::pulse_ui::pulse_status_notice(
                                    ramag_ui::pulse_ui::PulseStatus::Warming,
                                    "正在打开连接…",
                                    cx,
                                ),
                            ))
                            .into_any_element(),
                    },
                    None => {
                        let view: AnyView = self.picker.clone().into();
                        div().size_full().child(view).into_any_element()
                    }
                }
            }
            CenterMode::ConnectionPicker => {
                let view: AnyView = self.picker.clone().into();
                div().size_full().child(view).into_any_element()
            }
        };
        let session_context_header = (!on_picker_active)
            .then(|| {
                active
                    .and_then(|index| self.sessions.get(index))
                    .map(|slot| {
                        let (status, status_label) = session_pulse_status(
                            slot.stale,
                            slot.entity.as_ref().map(|entity| entity.health(cx)),
                        );
                        let subtitle = format!(
                            "{} · {}:{}",
                            super::driver_kind_label(slot.config.driver),
                            slot.config.host,
                            slot.config.port
                        );
                        render_session_context_header(
                            slot.config.name.clone(),
                            subtitle,
                            status,
                            status_label,
                            cx,
                        )
                    })
            })
            .flatten();

        v_flex()
            .key_context("DbClientView")
            .track_focus(&self.focus_handle)
            .on_action(
                cx.listener(|this, _: &ramag_ui::OpenRecentItems, window, cx| {
                    if matches!(this.center, CenterMode::Session) && !this.sessions.is_empty() {
                        this.open_connection_picker_dialog(window, cx);
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }),
            )
            .size_full()
            .bg(bg)
            .text_color(fg)
            .child(tab_bar)
            .when_some(session_context_header, |view, header| view.child(header))
            .child(div().flex_1().min_h_0().child(center_view))
    }
}

impl DbClientView {
    /// 配置已更新的暂停面板：说明原因 + 一键重连 / 关闭标签
    fn render_stale_panel(
        &self,
        id: ConnectionId,
        name: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let reconnect_id = id.clone();
        let close_id = id;
        let muted_fg = theme.muted_foreground;
        let fg = theme.foreground;

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .child(ramag_ui::pulse_ui::pulse_status_notice(
                ramag_ui::pulse_ui::PulseStatus::Stale,
                format!("连接「{name}」的配置已更新"),
                cx,
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(muted_fg)
                    .child("查询和写入已暂停，避免按旧配置操作数据库。"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted_fg)
                    .child("SQL / 命令草稿会在重连后恢复。"),
            )
            .child(
                h_flex()
                    .pt_2()
                    .gap_2()
                    .child(
                        ramag_ui::clickable_button("stale-reconnect")
                            .primary()
                            .small()
                            .label("重连")
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.reconnect_slot_by_id(reconnect_id.clone(), window, cx);
                            })),
                    )
                    .child(
                        ramag_ui::clickable_button("stale-close")
                            .ghost()
                            .small()
                            .label("关闭")
                            .text_color(fg)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.close_session_by_id(close_id.clone(), cx);
                            })),
                    ),
            )
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
