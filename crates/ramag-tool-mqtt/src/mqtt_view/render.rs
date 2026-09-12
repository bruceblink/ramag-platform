impl Render for MqttView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some((message, is_error)) = self.notice.take() {
            let notification = if is_error {
                gpui_component::notification::Notification::error(message)
            } else {
                gpui_component::notification::Notification::info(message).autohide(true)
            };
            ramag_ui::push_responsive_notification(window, notification, cx);
        }
        let theme = cx.theme().clone();
        let narrow = Self::sidebar_is_narrow(window);
        let main = v_flex()
            .id("mqtt-main")
            .debug_selector(|| "mqtt-main".into())
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(theme.background)
            .child(self.render_header(window, cx))
            .child(match self.section {
                MqttSection::Config => self.render_config(window, cx).into_any_element(),
                MqttSection::Overview => self.render_overview(window, cx).into_any_element(),
                MqttSection::Publish => self.render_publish(cx).into_any_element(),
                MqttSection::Subscribe => self.render_subscribe(cx).into_any_element(),
                MqttSection::Mosquitto => self.render_mosquitto(cx).into_any_element(),
            });
        h_flex()
            .id("mqtt-root")
            .debug_selector(|| "mqtt-root".into())
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(theme.background)
            .when(narrow && self.sidebar_visible, |root| {
                root.flex_col().items_stretch()
            })
            .when(!narrow || self.sidebar_visible, |root| {
                root.child(self.render_sidebar(window, cx))
            })
            .child(main)
    }
}
