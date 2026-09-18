use gpui_component::WindowExt as _;

const MAX_MESSAGE_VIEW_BYTES: usize = 2 * 1024 * 1024;
const MESSAGE_VIEW_MIN_HEIGHT: f32 = 160.0;
const MESSAGE_VIEW_MAX_HEIGHT: f32 = 520.0;

struct MqttMessageViewer {
    message: MqttMessage,
    format: MqttPayloadFormat,
}

impl MqttView {
    fn open_message_viewer(
        &mut self,
        message: MqttMessage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let topic = message.topic.clone();
        let viewer = cx.new(|_| MqttMessageViewer {
            message,
            format: self.receive_payload_format,
        });
        window.open_dialog(cx, move |dialog, window, _| {
            let viewer_for_content = viewer.clone();
            dialog
                .title(ramag_ui::closable_dialog_title(
                    "mqtt-message-viewer-close",
                    format!("查看消息 · {topic}"),
                    |_, _| {},
                ))
                .close_button(false)
                .width(ramag_ui::responsive_dialog_width(window, 960.0))
                .max_h(ramag_ui::responsive_dialog_max_height(window))
                .margin_top(ramag_ui::responsive_dialog_top(window))
                .content(move |content, _, _| content.child(viewer_for_content.clone()))
        });
    }
}

impl MqttMessageViewer {
    fn set_format(&mut self, format: MqttPayloadFormat) {
        self.format = format;
    }

    fn copy_topic(&self, window: &mut Window, cx: &mut Context<Self>) {
        ramag_ui::copy_text_with_notification(self.message.topic.clone(), window, cx);
    }

    fn copy_payload(&self, window: &mut Window, cx: &mut Context<Self>) {
        let (text, _) = bounded_message_view_text(self.format, &self.message.payload);
        ramag_ui::copy_text_with_notification(text, window, cx);
    }

    fn render_format_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut toolbar = h_flex()
            .id("mqtt-message-viewer-formats")
            .debug_selector(|| "mqtt-message-viewer-formats".into())
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_center()
            .gap(px(5.0))
            .child(div().text_xs().child("显示格式"));
        for format in MqttPayloadFormat::ALL {
            let label = format.label();
            let selector = format!("mqtt-message-viewer-format-{label}");
            let mut button = ramag_ui::clickable_button(selector.clone())
                .debug_selector(move || selector.clone())
                .xsmall()
                .label(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.set_format(format);
                    cx.notify();
                }));
            button = if self.format == format {
                button.primary()
            } else {
                button.ghost()
            };
            toolbar = toolbar.child(button);
        }
        toolbar
    }

    fn render_metadata(&self, theme: &gpui_component::Theme) -> gpui::AnyElement {
        let mut metadata = h_flex()
            .id("mqtt-message-viewer-meta")
            .debug_selector(|| "mqtt-message-viewer-meta".into())
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_center()
            .gap(px(10.0))
            .text_xs()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(self.message.topic.clone()),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child(format!("QoS {} · {}", self.message.qos.as_u8(), self.message.received_at)),
            );
        if self.message.retain {
            metadata = metadata.child(message_badge("mqtt-message-viewer-retain", "Retain", theme));
        }
        if self.message.duplicate {
            metadata = metadata.child(message_badge("mqtt-message-viewer-duplicate", "Dup", theme));
        }
        if !self.message.user_properties.is_empty() {
            metadata = metadata.child(message_badge(
                "mqtt-message-viewer-properties",
                format!("属性 {}", self.message.user_properties.len()),
                theme,
            ));
        }
        metadata.into_any_element()
    }
}

impl Render for MqttMessageViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (text, truncated) = bounded_message_view_text(self.format, &self.message.payload);
        let viewport = window.viewport_size();
        let content_height = (f32::from(viewport.height) - 260.0)
            .clamp(MESSAGE_VIEW_MIN_HEIGHT, MESSAGE_VIEW_MAX_HEIGHT);
        let status = if truncated {
            format!("仅显示前 {} MiB", MAX_MESSAGE_VIEW_BYTES / 1024 / 1024)
        } else {
            format!("{} 字节", self.message.payload.len())
        };
        let copy_topic = ramag_ui::clickable_button("mqtt-message-viewer-copy-topic")
            .debug_selector(|| "mqtt-message-viewer-copy-topic".into())
            .ghost()
            .small()
            .icon(IconName::Copy)
            .label("复制 Topic")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.copy_topic(window, cx);
            }));
        let copy_payload = ramag_ui::clickable_button("mqtt-message-viewer-copy-payload")
            .debug_selector(|| "mqtt-message-viewer-copy-payload".into())
            .ghost()
            .small()
            .icon(IconName::Copy)
            .label("复制当前格式")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.copy_payload(window, cx);
            }));
        v_flex()
            .id("mqtt-message-viewer")
            .debug_selector(|| "mqtt-message-viewer".into())
            .w_full()
            .min_w_0()
            .gap(px(8.0))
            .child(self.render_metadata(&theme))
            .child(
                div()
                    .text_xs()
                    .text_color(if truncated {
                        theme.warning
                    } else {
                        theme.muted_foreground
                    })
                    .child(status),
            )
            .child(self.render_format_toolbar(cx))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .gap(px(6.0))
                    .child(copy_topic)
                    .child(copy_payload),
            )
            .child(
                div()
                    .id("mqtt-message-viewer-content-frame")
                    .debug_selector(|| "mqtt-message-viewer-content-frame".into())
                    .w_full()
                    .min_w_0()
                    .h(px(content_height))
                    .overflow_y_scroll()
                    .p(px(12.0))
                    .bg(theme.background)
                    .child(
                        ramag_ui::SelectableText::new("mqtt-message-viewer-content", text)
                            .w_full()
                            .min_w_0()
                            .text_color(theme.foreground),
                    ),
            )
    }
}

fn bounded_message_view_text(format: MqttPayloadFormat, payload: &[u8]) -> (String, bool) {
    let rendered = format_received_payload(format, payload);
    if rendered.len() <= MAX_MESSAGE_VIEW_BYTES {
        return (rendered, false);
    }
    const TRUNCATION_PREFIX: &str = "\n\n[消息内容已截断，原始显示结果字节数：";
    let suffix = format!("{TRUNCATION_PREFIX}{}]", rendered.len());
    let mut end = MAX_MESSAGE_VIEW_BYTES.saturating_sub(suffix.len());
    while end > 0 && !rendered.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}{}", &rendered[..end], suffix), true)
}
