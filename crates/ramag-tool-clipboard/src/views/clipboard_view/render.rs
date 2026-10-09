use gpui_kit::component::{
    ActiveTheme, Selectable as _, Sizable as _, button::ButtonVariants as _, h_flex, input::Input,
    scroll::ScrollableElement as _, v_flex,
};
use gpui_kit::{
    ClickEvent, Context, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::*, px, uniform_list,
};
use ramag_domain::entities::{ClipKind, format_bytes};

use super::ClipboardView;
use crate::actions::{SelectNextClip, SelectPrevClip};

impl Render for ClipboardView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_search_once {
            self.focused_search_once = true;
            self.search.update(cx, |state, cx| state.focus(window, cx));
        }
        if let Some(n) = self.pending_notification.take() {
            ramag_ui::push_responsive_notification(window, n, cx);
        }

        let theme = cx.theme();
        let border = theme.border;
        let muted = theme.muted_foreground;
        let visible = self.visible_items(cx);
        self.reconcile_selection(&visible);
        let count = visible.len();
        let total_bytes = visible
            .iter()
            .fold(0_u64, |total, item| total.saturating_add(item.byte_size));
        let query_active = !self.search.read(cx).value().trim().is_empty();
        let count_label =
            clipboard_status_label(count, total_bytes, query_active && self.search_truncated);
        let focus = self.focus_handle.clone();
        let compact = f32::from(window.viewport_size().width) < 900.0;

        let list_pane = v_flex()
            .bg(ramag_ui::pulse_ui::pulse_palette(cx).surface)
            .debug_selector(|| "clipboard-view-list-pane".into())
            .min_w_0()
            .when(compact, |pane| {
                pane.w_full()
                    .flex_1()
                    .min_h(px(240.0))
                    .border_b_1()
                    .border_color(border)
            })
            .when(!compact, |pane| {
                pane.w(px(360.0))
                    .h_full()
                    .flex_none()
                    .border_r_1()
                    .border_color(border)
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .vertical_scrollbar(&self.list_scroll)
                    .child(self.render_list(visible, cx)),
            )
            .child(
                div()
                    .flex_none()
                    .w_full()
                    .px(px(12.0))
                    .py(px(6.0))
                    .border_t_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(muted)
                    .child(count_label),
            );
        let detail_pane = div()
            .debug_selector(|| "clipboard-view-detail-pane".into())
            .min_w_0()
            .when(compact, |pane| pane.w_full().flex_1().min_h(px(240.0)))
            .when(!compact, |pane| pane.flex_1().h_full())
            .child(self.render_detail(cx));

        v_flex()
            .key_context("ClipboardView")
            .track_focus(&focus)
            .on_action(cx.listener(Self::on_select_next))
            .on_action(cx.listener(Self::on_select_prev))
            .size_full()
            .min_w_0()
            .child(
                ramag_ui::pulse_ui::pulse_entry_header(cx)
                    .id("clipboard-view-page-header")
                    .debug_selector(|| "clipboard-view-page-header".into())
                    .w_full()
                    .min_w_0()
                    .flex_none()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        ramag_ui::pulse_ui::pulse_page_title("剪贴板", Some("本地历史和搜索"), cx)
                            .id("clipboard-view-page-title")
                            .debug_selector(|| "clipboard-view-page-title".into()),
                    ),
            )
            .child(self.render_toolbar(cx))
            .child(
                h_flex()
                    .id("clipboard-view-content")
                    .debug_selector(|| "clipboard-view-content".into())
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .when(compact, |content| {
                        content
                            .flex_col()
                            .items_stretch()
                            .overflow_y_scroll()
                            .track_scroll(&self.content_scroll)
                            .vertical_scrollbar(&self.content_scroll)
                    })
                    .child(list_pane)
                    .child(detail_pane),
            )
    }
}

fn clipboard_status_label(count: usize, total_bytes: u64, search_truncated: bool) -> String {
    let usage = format_bytes(total_bytes);
    if search_truncated {
        format!("显示 {count} 条 · {usage} · 历史至少 500 条（仅加载前 500 条）")
    } else {
        format!("{count} 条 · 占用 {usage}")
    }
}

impl ClipboardView {
    /// 渲染剪贴板历史的搜索和类型筛选工具栏；两个区域在窄窗口中换行，保持控件可见。
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;

        ramag_ui::responsive_toolbar()
            .debug_selector(|| "clipboard-view-toolbar".into())
            .flex_none()
            .items_start()
            .border_b_1()
            .border_color(border)
            .child(
                h_flex()
                    .debug_selector(|| "clipboard-view-search-pane".into())
                    .flex_1()
                    .min_w(px(128.0))
                    .max_w(px(360.0))
                    .items_center()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .border_r_1()
                    .border_color(border)
                    .child(
                        div()
                            .debug_selector(|| "clipboard-view-search".into())
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.search).small()),
                    ),
            )
            .child(
                h_flex()
                    .debug_selector(|| "clipboard-view-filters".into())
                    .flex_1()
                    .min_w(px(128.0))
                    .items_center()
                    .px(px(12.0))
                    .py(px(8.0))
                    .child(self.render_filter_chips(cx)),
            )
    }

    fn render_filter_chips(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut row = ramag_ui::responsive_toolbar()
            .debug_selector(|| "clipboard-filter-chips".into())
            .gap(px(4.0));

        row = row.child(
            ramag_ui::clickable_button("filter-all")
                .debug_selector(|| "clipboard-filter-all".into())
                .ghost()
                .xsmall()
                .label("全部")
                .selected(self.filter.is_none())
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.filter = None;
                    cx.notify();
                })),
        );
        for &kind in ClipKind::all() {
            let active = self.filter == Some(kind);
            let selector = format!("clipboard-filter-{}", kind.label());
            row = row.child(
                ramag_ui::clickable_button(SharedString::from(format!("filter-{}", kind.label())))
                    .debug_selector(move || selector.clone())
                    .ghost()
                    .xsmall()
                    .label(kind.label())
                    .selected(active)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.filter = Some(kind);
                        cx.notify();
                    })),
            );
        }
        row
    }

    fn render_list(
        &self,
        visible: Vec<std::sync::Arc<ramag_domain::entities::ClipItem>>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if visible.is_empty() {
            let muted = cx.theme().muted_foreground;
            let query = self.search.read(cx).value().trim().to_string();
            let hint = if !query.is_empty() {
                format!("没有匹配「{query}」的条目")
            } else if self.filter.is_some() {
                "该类型下暂无条目".to_string()
            } else if !self.settings.enabled {
                "采集已关闭：请在“设置 > 剪贴板”中开启后再使用".to_string()
            } else {
                "暂无剪贴历史；复制任意内容后会出现在这里".to_string()
            };
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted)
                .child(hint)
                .into_any_element();
        }

        let count = visible.len();
        let entity = cx.entity().clone();
        uniform_list("clip-list", count, move |range, _window, cx| {
            range
                .map(|ix| {
                    entity.update(cx, |this, cx| {
                        this.render_card(visible[ix].clone(), cx).into_any_element()
                    })
                })
                .collect::<Vec<_>>()
        })
        .track_scroll(&self.list_scroll)
        .size_full()
        .into_any_element()
    }
}

impl ClipboardView {
    fn on_select_next(&mut self, _: &SelectNextClip, _: &mut Window, cx: &mut Context<Self>) {
        self.move_selection(1, cx);
    }

    fn on_select_prev(&mut self, _: &SelectPrevClip, _: &mut Window, cx: &mut Context<Self>) {
        self.move_selection(-1, cx);
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
