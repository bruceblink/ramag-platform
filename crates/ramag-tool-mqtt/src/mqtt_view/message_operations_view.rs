impl MqttView {
    fn render_overview(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let capabilities = self.service.transport_capabilities();
        let mut capabilities_view = v_flex().gap(px(5.0));
        for (label, enabled) in capability_items(capabilities) {
            capabilities_view = capabilities_view.child(
                h_flex()
                    .gap(px(8.0))
                    .child(
                        Icon::new(if enabled {
                            IconName::CircleCheck
                        } else {
                            IconName::CircleX
                        })
                        .small()
                        .text_color(if enabled {
                            theme.accent
                        } else {
                            theme.muted_foreground
                        }),
                    )
                    .child(div().text_xs().child(label)),
            );
        }
        let snapshot_body = if self.loading_snapshot {
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("正在读取 Broker 状态…")
                .into_any_element()
        } else if let Some(error) = &self.snapshot_error {
            div()
                .text_sm()
                .text_color(theme.danger)
                .child(error.clone())
                .into_any_element()
        } else if let Some(snapshot) = &self.snapshot {
            v_flex()
                .gap(px(6.0))
                .children([
                    div().text_sm().child(format!(
                        "观察到 {} 个 Topic，{} 个在线客户端",
                        snapshot.topics.len(),
                        snapshot.online_clients.len()
                    )),
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "Topic 数据{}完整；在线客户端数据{}完整。",
                            if snapshot.topics_complete { "" } else { "不" },
                            if snapshot.online_clients_complete {
                                ""
                            } else {
                                "不"
                            }
                        )),
                ])
                .into_any_element()
        } else {
            div().text_sm().text_color(theme.muted_foreground).child("尚未读取 Broker 状态；标准 MQTT 不提供完整 Topic 目录，驱动会明确标记数据是否完整。").into_any_element()
        };
        let mut body = v_flex().w_full().max_w(px(920.0)).gap(px(14.0));
        body = body
            .child(section_heading(
                "传输能力",
                "能力来自当前编译的 Native 驱动，不代表远端 Broker 已连接",
                &theme,
            ))
            .child(capabilities_view);
        body = body
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(section_heading(
                        "Broker 状态",
                        "只展示本次真实请求返回的观察结果",
                        &theme,
                    ))
                    .child(
                        ramag_ui::clickable_button("mqtt-refresh-snapshot")
                            .ghost()
                            .small()
                            .icon(ramag_ui::icons::refresh_cw())
                            .tooltip("读取 Broker 状态")
                            .disabled(self.loading_snapshot || self.selected_profile_id.is_none())
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.load_snapshot(window, cx)
                            })),
                    ),
            )
            .child(snapshot_body);
        if let Some(snapshot) = &self.snapshot {
            body = body.child(section_heading(
                "观察到的 Topic",
                "这些 Topic 来自驱动返回，不是 Broker 的完整目录",
                &theme,
            ));
            if snapshot.topics.is_empty() {
                body = body.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("本次没有观察到 Topic。"),
                );
            } else {
                let mut topics = v_flex().gap(px(3.0));
                for topic in &snapshot.topics {
                    topics = topics.child(
                        h_flex()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_xs()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(topic.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{:?}", topic.source)),
                            ),
                    );
                }
                body = body.child(topics);
            }
        }
        div()
            .id("mqtt-overview-scroll")
            .w_full()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p(px(16.0))
            .child(body)
            .when(window.viewport_size().width < px(760.0), |view| {
                view.p(px(10.0))
            })
    }

    fn render_publish(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let body = v_flex()
            .w_full()
            .min_w_0()
            .max_w(px(920.0))
            .gap(px(12.0))
            .child(section_heading(
                "发布消息",
                "消息通过当前配置连接远端 Broker；没有成功返回就不会显示为已发布",
                &theme,
            ))
            .child(
                field(
                    "Topic",
                    input_frame(
                        "mqtt-publish-topic-input",
                        Input::new(&self.publish_topic)
                            .small()
                            .w_full()
                            .min_w_0(),
                    ),
                ),
            )
            .child(field(
                "Payload（UTF-8）",
                input_frame(
                    "mqtt-publish-payload-input",
                        Input::new(&self.publish_payload)
                            .h(px(140.0))
                            .small()
                            .w_full()
                            .min_w_0(),
                ),
            ))
            .child(
                row()
                    .debug_selector(|| "mqtt-publish-options".into())
                    .child(qos_selector(
                        "mqtt-publish-qos",
                        self.publish_qos,
                        self.is_busy() || self.subscription_running,
                        cx,
                        |this, qos| this.publish_qos = qos,
                    ))
                    .child(toggle_button(
                        "mqtt-publish-retain",
                        "Retain",
                        self.publish_retain,
                        self.is_busy() || self.subscription_running,
                        cx,
                        |this| this.publish_retain = !this.publish_retain,
                    )),
            )
            .child(
                div()
                    .debug_selector(|| "mqtt-publish-actions".into())
                    .when(window.viewport_size().width < px(760.0), |actions| {
                        actions.w_full()
                    })
                    .child(ramag_ui::clickable_button("mqtt-publish")
                    .debug_selector(|| "mqtt-publish".into())
                    .primary()
                    .small()
                    .label("发布消息")
                    .loading(self.publishing)
                    .disabled(self.publishing || self.subscription_running)
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| this.publish(window, cx)),
                    )),
            );
        v_flex()
            .id("mqtt-publish-scroll")
            .w_full()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p(px(16.0))
            .child(body)
    }

    fn render_subscribe(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut body = v_flex()
            .w_full()
            .min_w_0()
            .max_w(px(920.0))
            .gap(px(12.0))
            .child(section_heading(
                "订阅消息",
                "订阅使用有界缓冲；缓冲满时驱动会报告背压，不会无限堆积内存",
                &theme,
            ))
            .child(
                field(
                    "Topic Filter",
                    input_frame(
                        "mqtt-subscribe-filter-input",
                        div()
                            .w_full()
                            .min_w_0()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                                    this.subscribe_filter
                                        .update(cx, |input, cx| input.focus(window, cx));
                                }),
                            )
                            .child(
                                Input::new(&self.subscribe_filter)
                                    .small()
                                    .w_full()
                                    .min_w_0(),
                            ),
                    ),
                ),
            );
        body = body.child(
            row()
                .debug_selector(|| "mqtt-subscribe-options".into())
                .child(qos_selector(
                    "mqtt-subscribe-qos",
                    self.subscribe_qos,
                    self.is_busy() || self.subscription_running,
                    cx,
                    |this, qos| this.subscribe_qos = qos,
                )),
        );
        let action = if self.subscription_running {
            ramag_ui::clickable_button("mqtt-stop-subscription")
                .debug_selector(|| "mqtt-stop-subscription".into())
                .danger()
                .small()
                .label(if self.subscription_stopping {
                    "正在停止订阅…"
                } else {
                    "停止订阅"
                })
                .disabled(self.subscription_stopping)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.stop_subscription();
                    cx.notify();
                }))
        } else {
            ramag_ui::clickable_button("mqtt-start-subscription")
                .debug_selector(|| "mqtt-start-subscription".into())
                .primary()
                .small()
                .label("开始订阅")
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.start_subscription(window, cx)
                }))
        };
        body = body.child(
            div()
                .debug_selector(|| "mqtt-subscribe-actions".into())
                .when(window.viewport_size().width < px(760.0), |actions| {
                    actions.w_full()
                })
                .child(action),
        );
        if self.messages.is_empty() {
            body = body.child(div().text_sm().text_color(theme.muted_foreground).child(
                if self.subscription_running {
                    "等待消息…"
                } else {
                    "尚未收到消息。"
                },
            ));
        } else {
            let mut messages = v_flex().gap(px(6.0));
            for message in self.messages.iter().rev() {
                let payload = String::from_utf8_lossy(&message.payload);
                let mut metadata = h_flex()
                    .debug_selector(|| "mqtt-subscribe-message-meta".into())
                    .flex_wrap()
                    .min_w_0()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(message.topic.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!(
                                "QoS {} · {}",
                                message.qos.as_u8(),
                                message.received_at
                            )),
                    );
                if message.retain {
                    metadata = metadata.child(message_badge(
                        "mqtt-subscribe-message-retained",
                        "Retain",
                        &theme,
                    ));
                }
                if message.duplicate {
                    metadata = metadata.child(message_badge(
                        "mqtt-subscribe-message-duplicate",
                        "Dup",
                        &theme,
                    ));
                }
                if !message.user_properties.is_empty() {
                    metadata = metadata.child(message_badge(
                        "mqtt-subscribe-message-properties",
                        format!("属性 {}", message.user_properties.len()),
                        &theme,
                    ));
                }
                messages = messages.child(
                    v_flex()
                        .gap(px(3.0))
                        .p(px(10.0))
                        .border_1()
                        .border_color(theme.border)
                        .rounded(px(5.0))
                        .child(metadata)
                        .child(
                            div()
                                .text_xs()
                                .whitespace_normal()
                                .child(payload.to_string()),
                        ),
                );
            }
            body = body.child(messages);
        }
        v_flex()
            .id("mqtt-subscribe-scroll")
            .w_full()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p(px(16.0))
            .child(body)
    }

}
