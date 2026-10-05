//! 对象存储工作区的 Bucket 导航。

use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Selectable as _, Sizable as _, StyledExt as _,
    button::ButtonVariants as _, h_flex, scroll::ScrollableElement as _, v_flex,
};
use gpui_kit::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, Role, SharedString,
    StatefulInteractiveElement as _, Styled, div, prelude::*, px,
};

use super::model::ObjectStorageView;
use super::mount_sort::{
    MountSortColumn, mount_sort_description, mount_sort_icon, next_mount_sort, sort_mounts,
};

impl ObjectStorageView {
    pub(super) fn render_mounts(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let selected_bg = cx.theme().muted;
        let muted = cx.theme().muted_foreground;
        let query = self
            .mount_search
            .read(cx)
            .value()
            .to_string()
            .to_lowercase();
        let mut rows = v_flex().w_full();
        let mut mounts: Vec<_> = self
            .mounts
            .iter()
            .filter(|mount| {
                query.is_empty()
                    || mount.bucket.to_lowercase().contains(&query)
                    || mount.region.to_lowercase().contains(&query)
                    || mount
                        .root_prefix
                        .as_deref()
                        .is_some_and(|prefix| prefix.to_lowercase().contains(&query))
            })
            .collect();
        sort_mounts(&mut mounts, self.mount_sort);
        if mounts.is_empty() {
            rows = rows.child(
                div()
                    .w_full()
                    .py(px(48.0))
                    .px(px(16.0))
                    .text_center()
                    .text_xs()
                    .text_color(muted)
                    .child(if query.is_empty() {
                        "暂无 Bucket，请编辑账号并添加 Bucket"
                    } else {
                        "暂无匹配"
                    }),
            );
        }
        let mut last_region: Option<&str> = None;
        for mount in mounts {
            if last_region != Some(mount.region.as_str()) {
                last_region = Some(&mount.region);
                rows = rows.child(mount_section(mount.region.clone(), muted));
            }
            let selected = self
                .selected_mount
                .as_ref()
                .is_some_and(|value| value.id == mount.id);
            let target = mount.clone();
            let row_selector = format!("object-mount-row-{}", mount.bucket);
            rows = rows.child(
                h_flex()
                    .id(SharedString::from(format!("object-mount-{}", mount.id)))
                    .debug_selector(move || row_selector.clone())
                    .w_full()
                    .h(px(36.0))
                    .items_center()
                    .gap(px(8.0))
                    .px(px(10.0))
                    .cursor_pointer()
                    .when(selected, |row| row.bg(selected_bg))
                    .hover(|row| row.bg(selected_bg))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_mount(target.clone(), window, cx);
                    }))
                    .child(Icon::new(IconName::HardDrive).small().text_color(muted))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_sm()
                            .child(mount.bucket.clone()),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .flex_none()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_right()
                            .text_xs()
                            .text_color(muted)
                            .child(mount.root_prefix.as_deref().map_or_else(
                                || "根目录".to_owned(),
                                |prefix| format!("/{prefix}"),
                            )),
                    ),
            );
        }
        let summary = format!(
            "Bucket {} · 收藏 {}",
            self.mounts.len(),
            self.favorites.len()
        );
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .id("object-mount-toolbar")
                    .debug_selector(|| "object-mount-toolbar".into())
                    .w_full()
                    .h(px(40.0))
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .gap(px(4.0))
                    .px(px(6.0))
                    .bg(cx.theme().secondary)
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div().flex_1().min_w_0().child(
                            ramag_ui::cleanable_input(
                                &self.mount_search,
                                "object-mount-search-clear",
                                false,
                                cx,
                            )
                            .small()
                            .prefix(Icon::new(IconName::Search).small().text_color(muted)),
                        ),
                    )
                    .child(
                        ramag_ui::clickable_button("refresh-object-mounts")
                            .ghost()
                            .xsmall()
                            .icon(ramag_ui::icons::refresh_cw())
                            .tooltip("刷新")
                            .disabled(self.selected_account_id.is_none() || self.loading)
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(id) = this.selected_account_id.clone() {
                                    this.load_mounts(id, window, cx);
                                }
                            })),
                    )
                    .child(
                        div()
                            .id("object-transfers")
                            .debug_selector(|| "object-transfers".into())
                            .flex_none()
                            .child(
                                ramag_ui::clickable_button("object-transfers-button")
                                    .ghost()
                                    .xsmall()
                                    .icon(ramag_ui::icons::arrow_up_down())
                                    .tooltip("传输")
                                    .selected(self.transfers_visible)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.transfers_visible = !this.transfers_visible;
                                        if this.transfers_visible {
                                            this.show_detail = false;
                                            this.persist_workspace(cx);
                                        }
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .id("object-mount-columns")
                    .debug_selector(|| "object-mount-columns".into())
                    .w_full()
                    .h(px(28.0))
                    .flex_none()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(10.0))
                    .border_b_1()
                    .border_color(border)
                    .bg(cx.theme().secondary)
                    .text_xs()
                    .text_color(muted)
                    .child(div().w(px(16.0)).flex_none())
                    .child(self.render_mount_sort_header(
                        MountSortColumn::Bucket,
                        "Bucket",
                        None,
                        true,
                        false,
                        cx,
                    ))
                    .child(self.render_mount_sort_header(
                        MountSortColumn::RootPath,
                        "根路径",
                        Some(100.0),
                        false,
                        true,
                        cx,
                    )),
            )
            .child(
                div()
                    .id("object-mounts-scroll")
                    .debug_selector(|| "object-mounts-scroll".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(rows),
            )
            .child(
                h_flex()
                    .id("object-mount-summary")
                    .debug_selector(|| "object-mount-summary".into())
                    .w_full()
                    .h(px(32.0))
                    .flex_none()
                    .items_center()
                    .px(px(10.0))
                    .border_t_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(muted)
                    .child(summary),
            )
    }

    fn render_mount_sort_header(
        &self,
        column: MountSortColumn,
        label: &'static str,
        width: Option<f32>,
        flexible: bool,
        right_aligned: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let description = mount_sort_description(self.mount_sort, column, label);
        let icon = mount_sort_icon(self.mount_sort, column);
        let foreground = cx.theme().foreground;
        let selector = format!("object-mount-sort-{}", column.key());
        let debug_selector = selector.clone();
        let mut header = h_flex()
            .id(SharedString::from(selector))
            .debug_selector(move || debug_selector.clone())
            .role(Role::Button)
            .aria_label(description)
            .items_center()
            .gap(px(4.0))
            .min_w_0()
            .cursor_pointer()
            .hover(move |header| header.text_color(foreground))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.mount_sort = Some(next_mount_sort(this.mount_sort, column));
                cx.notify();
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .when(right_aligned, |label| label.text_right())
                    .text_color(cx.theme().muted_foreground)
                    .child(label),
            )
            .child(
                Icon::new(icon)
                    .xsmall()
                    .text_color(cx.theme().muted_foreground),
            );
        if let Some(width) = width {
            header = header.w(px(width)).flex_none();
        }
        if flexible {
            header = header.flex_1();
        }
        header.into_any_element()
    }
}

fn mount_section(label: impl Into<SharedString>, color: gpui_kit::Hsla) -> AnyElement {
    div()
        .w_full()
        .h(px(28.0))
        .flex()
        .items_center()
        .px(px(10.0))
        .text_xs()
        .font_semibold()
        .text_color(color)
        .child(label.into())
        .into_any_element()
}
