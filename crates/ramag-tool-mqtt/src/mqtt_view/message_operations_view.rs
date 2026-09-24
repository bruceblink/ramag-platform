use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu};

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
                                    .child(mqtt_topic_source_label(topic.source)),
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

    fn render_publish(&self, _window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .w_full()
                            .min_w_0(),
                    ),
                )
                .w_full(),
            )
            .child(
                field(
                    "Payload",
                    input_frame(
                        "mqtt-publish-payload-input",
                        Textarea::new(&self.publish_payload)
                            .h(px(140.0))
                            .w_full()
                            .min_w_0(),
                    ),
                )
                .w_full(),
            )
            .child(
                row()
                    .debug_selector(|| "mqtt-publish-options".into())
                    .child(payload_format_selector(
                        "mqtt-publish-payload-format",
                        self.publish_payload_format,
                        self.publishing || self.subscription_running,
                        cx,
                        |this, format| this.publish_payload_format = format,
                    ))
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
                .self_start()
                .child(
                    ramag_ui::clickable_button("mqtt-publish")
                        .debug_selector(|| "mqtt-publish".into())
                        .primary()
                        .small()
                        .flex_none()
                        .self_start()
                        .label("发布消息")
                        .loading(self.publishing)
                        .disabled(self.publishing || self.subscription_running)
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.publish(window, cx)
                            }),
                        ),
                ),
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

    fn render_subscribe(&self, _window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut topic_rows = v_flex().w_full().gap(px(5.0));
        for (index, subscription) in self.subscription_topics.iter().enumerate() {
            let row_selector = SharedString::from(format!("mqtt-subscription-row-{index}"));
            let remove_selector = SharedString::from(format!(
                "mqtt-remove-subscription-{index}"
            ));
            let filter = subscription.filter.clone();
            let no_local = subscription.no_local;
            let status = self.subscription_status(&subscription.filter);
            let status_selector = SharedString::from(format!("mqtt-subscription-status-{index}"));
            let status_color = match status.state {
                MqttSubscriptionState::Pending => theme.muted_foreground,
                MqttSubscriptionState::Subscribing => theme.warning,
                MqttSubscriptionState::Subscribed => theme.accent,
                MqttSubscriptionState::Unsubscribing => theme.warning,
                MqttSubscriptionState::Rejected => theme.danger,
            };
            let topic_action_selector =
                SharedString::from(format!("mqtt-subscription-action-{index}"));
            let topic_action = match status.state {
                MqttSubscriptionState::Pending | MqttSubscriptionState::Rejected => {
                    ramag_ui::clickable_button(topic_action_selector.clone())
                        .debug_selector({
                            let selector = topic_action_selector.clone();
                            move || selector.to_string()
                        })
                        .ghost()
                        .xsmall()
                        .icon(IconName::Play)
                        .label("订阅")
                        .tooltip("订阅此 Topic")
                        .disabled(self.subscription_stopping)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.toggle_subscription_topic(index, window, cx);
                        }))
                }
                MqttSubscriptionState::Subscribed => {
                    ramag_ui::clickable_button(topic_action_selector.clone())
                        .debug_selector({
                            let selector = topic_action_selector.clone();
                            move || selector.to_string()
                        })
                        .ghost()
                        .xsmall()
                        .icon(IconName::CircleX)
                        .label("取消")
                        .tooltip("取消此 Topic 的订阅")
                        .disabled(self.subscription_stopping)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.toggle_subscription_topic(index, window, cx);
                        }))
                }
                MqttSubscriptionState::Subscribing => {
                    ramag_ui::clickable_button(topic_action_selector.clone())
                        .debug_selector({
                            let selector = topic_action_selector.clone();
                            move || selector.to_string()
                        })
                        .ghost()
                        .xsmall()
                        .label("订阅中")
                        .disabled(true)
                }
                MqttSubscriptionState::Unsubscribing => {
                    ramag_ui::clickable_button(topic_action_selector.clone())
                        .debug_selector({
                            let selector = topic_action_selector.clone();
                            move || selector.to_string()
                        })
                        .ghost()
                        .xsmall()
                        .label("取消中")
                        .disabled(true)
                }
            };
            topic_rows = topic_rows.child(
                h_flex()
                    .id(row_selector.clone())
                    .debug_selector({
                        let selector = row_selector.clone();
                        move || selector.to_string()
                    })
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(10.0))
                    .py(px(7.0))
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(4.0))
                    .child(
                        div()
                            .text_sm()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(filter),
                    )
                    .child(
                        div()
                            .debug_selector({
                                let selector = status_selector.clone();
                                move || selector.to_string()
                            })
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(status_color)
                            .whitespace_normal()
                            .child(subscription_status_text(&status)),
                    )
                    .child(topic_action)
                    .child(subscription_qos_selector(
                        index,
                        subscription.qos,
                        self.is_busy() || self.subscription_running,
                        cx,
                    ))
                    .child(subscription_no_local_toggle(
                        index,
                        no_local,
                        self.is_busy()
                            || self.subscription_running
                            || self.protocol == MqttProtocolVersion::V311,
                        cx,
                    ))
                    .child(
                        ramag_ui::clickable_button(remove_selector.clone())
                            .debug_selector(move || remove_selector.to_string())
                            .ghost()
                            .xsmall()
                            .icon(IconName::Delete)
                            .tooltip("移除订阅 Topic")
                            .disabled(self.is_busy() || self.subscription_running)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.remove_subscription_topic(index, cx);
                            })),
                    ),
            );
        }
        if self.subscription_topics.is_empty() {
            topic_rows = topic_rows.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("尚未添加订阅 Topic Filter。"),
            );
        }
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
            .child(section_heading(
                "订阅主题",
                "每条 Topic Filter 单独配置 QoS；No Local 仅由 MQTT 5 支持",
                &theme,
            ))
            .child(topic_rows)
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
                )
                .w_full(),
            )
            .child(
            row()
                .debug_selector(|| "mqtt-subscribe-options".into())
                .child(payload_format_selector(
                    "mqtt-receive-payload-format",
                    self.receive_payload_format,
                    self.is_busy() || self.subscription_running,
                    cx,
                    |this, format| this.receive_payload_format = format,
                ))
                .child(qos_selector(
                    "mqtt-subscribe-qos",
                    self.subscribe_qos,
                    self.is_busy() || self.subscription_running,
                    cx,
                    |this, qos| this.subscribe_qos = qos,
                ))
                .child(toggle_button(
                    "mqtt-subscribe-no-local",
                    "No Local",
                    self.subscribe_no_local,
                    self.is_busy()
                        || self.subscription_running
                        || self.protocol == MqttProtocolVersion::V311,
                    cx,
                    |this| this.subscribe_no_local = !this.subscribe_no_local,
                ))
                .child(
                    ramag_ui::clickable_button("mqtt-add-subscription")
                        .debug_selector(|| "mqtt-add-subscription".into())
                        .primary()
                        .small()
                        .flex_none()
                        .icon(IconName::Plus)
                        .label("添加主题")
                        .disabled(self.is_busy() || self.subscription_running)
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.add_subscription_topic(window, cx)
                        })),
                ),
        );
        let action = if self.subscription_running {
            ramag_ui::clickable_button("mqtt-stop-subscription")
                .debug_selector(|| "mqtt-stop-subscription".into())
                .danger()
                .small()
                .flex_none()
                .self_start()
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
                .flex_none()
                .self_start()
                .label("开始订阅")
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.start_subscription(window, cx)
                }))
        };
        body = body.child(
            div()
                .debug_selector(|| "mqtt-subscribe-actions".into())
                .self_start()
                .child(action),
        );
        let timeline_status = if self.message_timeline_paused {
            "已暂停展示；Broker 连接继续接收"
        } else if self.subscription_running {
            "正在接收消息"
        } else {
            "未运行订阅"
        };
        body = body.child(
            ramag_ui::responsive_toolbar()
                .id("mqtt-message-timeline-actions")
                .debug_selector(|| "mqtt-message-timeline-actions".into())
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(if self.message_timeline_paused {
                            theme.warning
                        } else {
                            theme.muted_foreground
                        })
                        .child(timeline_status),
                )
                .when(self.subscription_running, |toolbar| {
                    toolbar.child(
                        ramag_ui::clickable_button("mqtt-message-timeline-pause")
                            .debug_selector(|| "mqtt-message-timeline-pause".into())
                            .outline()
                            .small()
                            .flex_none()
                            .icon(if self.message_timeline_paused {
                                IconName::Play
                            } else {
                                IconName::Pause
                            })
                            .label(if self.message_timeline_paused {
                                "恢复展示"
                            } else {
                                "暂停展示"
                            })
                            .disabled(self.subscription_stopping)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.toggle_message_timeline_pause();
                                cx.notify();
                            })),
                    )
                })
                .child(
                    ramag_ui::clickable_button("mqtt-message-timeline-clear")
                        .debug_selector(|| "mqtt-message-timeline-clear".into())
                        .ghost()
                        .small()
                        .flex_none()
                        .icon(IconName::Delete)
                        .label("清空时间线")
                        .disabled(self.messages.is_empty())
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.clear_message_timeline();
                            cx.notify();
                        })),
                ),
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
            let view_entity = cx.entity();
            for message in self.messages.iter().rev() {
                let payload = format_received_payload(self.receive_payload_format, &message.payload);
                let message_for_menu = message.clone();
                let message_for_copy = message.clone();
                let format_for_copy = self.receive_payload_format;
                let viewer_entity = view_entity.clone();
                let mut metadata = h_flex()
                    .debug_selector(|| "mqtt-subscribe-message-meta".into())
                    .flex_wrap()
                    .min_w_0()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
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
                let message_card = v_flex()
                        .debug_selector(|| "mqtt-subscribe-message-card".into())
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
                                .child(payload),
                        )
                        .context_menu(move |menu: PopupMenu, _, _| {
                            let message_for_viewer = message_for_menu.clone();
                            let message_for_topic = message_for_copy.clone();
                            let message_for_payload = message_for_copy.clone();
                            let viewer_entity = viewer_entity.clone();
                            menu.item(
                                ramag_ui::menu_item("查看消息")
                                    .icon(IconName::Eye)
                                    .on_click(move |_, window, app| {
                                        viewer_entity.update(app, |this, cx| {
                                            this.open_message_viewer(
                                                message_for_viewer.clone(),
                                                window,
                                                cx,
                                            );
                                        });
                                    }),
                            )
                            .item(
                                ramag_ui::menu_item("复制 Topic")
                                    .icon(IconName::Copy)
                                    .on_click(move |_, window, app| {
                                        ramag_ui::copy_text_with_notification(
                                            message_for_topic.topic.clone(),
                                            window,
                                            app,
                                        );
                                    }),
                            )
                            .item(
                                ramag_ui::menu_item("复制当前格式")
                                    .icon(IconName::Copy)
                                    .on_click(move |_, window, app| {
                                        let (text, _) = bounded_message_view_text(
                                            format_for_copy,
                                            &message_for_payload.payload,
                                        );
                                        ramag_ui::copy_text_with_notification(text, window, app);
                                    }),
                            )
                        });
                messages = messages.child(message_card);
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
