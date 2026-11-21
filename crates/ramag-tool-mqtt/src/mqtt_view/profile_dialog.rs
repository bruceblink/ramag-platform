// 窄窗口复用 Broker 配置列表、搜索和新建入口。

use gpui_kit::component::dialog::DialogEntrance;

impl MqttView {
    fn profile_dialog_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        ramag_ui::clickable_button("mqtt-show-sidebar")
            .debug_selector(|| "mqtt-show-sidebar".into())
            .outline()
            .small()
            .icon(IconName::PanelLeft)
            .label("Broker 配置")
            .track_focus(&self.profile_button_focus)
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.open_profile_dialog(window, cx);
            }))
    }

    fn open_profile_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.profile_dialog_open {
            return;
        }
        window.focus(&self.profile_button_focus, cx);
        self.profile_dialog_open = true;
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
                        .items_center()
                        .justify_between()
                        .child("Broker 配置")
                        .child(
                            ramag_ui::clickable_button("mqtt-hide-sidebar")
                                .debug_selector(|| "mqtt-hide-sidebar".into())
                                .ghost()
                                .small()
                                .icon(IconName::Close)
                                .accessibility_label("关闭 Broker 配置")
                                .on_click(move |_: &ClickEvent, window, app| {
                                    let _ = for_button.update(app, |view, cx| {
                                        view.close_profile_dialog(window, cx)
                                    });
                                }),
                        ),
                )
                .centered()
                .close_button(false)
                .entrance(DialogEntrance::None)
                .width(ramag_ui::responsive_dialog_width(window, 540.0))
                .h(px(540.0).min(limit))
                .max_h(limit)
                .on_close(move |_, window, app| {
                    let _ = for_dismiss
                        .update(app, |view, cx| view.profile_dialog_dismissed(window, cx));
                })
                .content(move |content, _, app| {
                    content.min_h_0().px_0().children(
                        for_render
                            .update(app, |view, cx| {
                                view.render_sidebar(cx, true).into_any_element()
                            })
                            .ok(),
                    )
                })
        });
        cx.notify();
    }

    fn close_profile_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.profile_dialog_open {
            window.close_dialog(cx);
            self.profile_dialog_dismissed(window, cx);
        }
    }

    fn profile_dialog_dismissed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.profile_dialog_open = false;
        if !Self::sidebar_is_narrow(window) {
            cx.on_next_frame(window, |view, window, cx| {
                if !view.profile_dialog_open && !window.has_active_dialog(cx) {
                    window.focus(&view.focus_handle, cx);
                }
            });
        }
        cx.notify();
    }
}
