//! 在窄窗口复用现有请求集合、搜索和执行历史。

use super::*;
use gpui_kit::component::{
    IconName, WindowExt as _, button::ButtonVariants as _, dialog::DialogEntrance,
};

pub(in crate::view) fn trigger(
    view: &ApiView,
    cx: &mut Context<ApiView>,
) -> impl IntoElement + use<> {
    ramag_ui::clickable_button("api-open-collection")
        .debug_selector(|| "api-open-collection".into())
        .ghost()
        .xsmall()
        .icon(IconName::FolderClosed)
        .label("请求集合")
        .track_focus(&view.collection_button_focus)
        .on_click(
            cx.listener(|view, _: &ClickEvent, window, cx| view.open_collection_dialog(window, cx)),
        )
}

impl ApiView {
    fn open_collection_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.collection_dialog_open {
            return;
        }

        window.focus(&self.collection_button_focus, cx);
        self.collection_dialog_open = true;
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, _| {
            let for_button = view.clone();
            let for_dismiss = view.clone();
            let for_render = view.clone();
            let limit = ramag_ui::responsive_dialog_max_height(window);
            dialog
                .title(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child("请求集合")
                        .child(
                            ramag_ui::clickable_button("api-close-collection")
                                .debug_selector(|| "api-close-collection".into())
                                .ghost()
                                .small()
                                .icon(IconName::Close)
                                .accessibility_label("关闭请求集合")
                                .on_click(move |_: &ClickEvent, window, app| {
                                    let _ = for_button.update(app, |view, cx| {
                                        view.close_collection_dialog(window, cx)
                                    });
                                }),
                        ),
                )
                .centered()
                .close_button(false)
                .entrance(DialogEntrance::None)
                .width(ramag_ui::responsive_dialog_width(window, 540.0))
                .h(px(560.0).min(limit))
                .max_h(limit)
                .on_close(move |_, window, app| {
                    let _ =
                        for_dismiss.update(app, |view, cx| view.collection_dismissed(window, cx));
                })
                .content(move |content, _, app| {
                    content.min_h_0().px_0().children(
                        for_render
                            .update(app, |view, cx| {
                                let theme = cx.theme().clone();
                                super::render_sidebar(view, cx, &theme, true)
                            })
                            .ok(),
                    )
                })
        });
        cx.notify();
    }

    pub(in crate::view) fn close_collection_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.collection_dialog_open {
            window.close_dialog(cx);
            self.collection_dismissed(window, cx);
        }
    }

    fn collection_dismissed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.collection_dialog_open = false;
        if !Self::is_compact(window) {
            cx.on_next_frame(window, |view, window, cx| {
                if !view.collection_dialog_open && !window.has_active_dialog(cx) {
                    window.focus(&view.focus_handle, cx);
                }
            });
        }
        cx.notify();
    }
}
