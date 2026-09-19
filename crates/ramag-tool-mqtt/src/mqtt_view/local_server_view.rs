fn local_server_event_text(event: &MqttLocalServerEvent) -> (String, String, String) {
    match event {
        MqttLocalServerEvent::ClientConnected {
            client, occurred_at, ..
        } => (
            "客户端连接".into(),
            format!(
                "Client ID：{}{} · {}",
                client.client_id,
                client
                    .username
                    .as_deref()
                    .map_or(String::new(), |username| format!(" · 用户名：{username}")),
                occurred_at.to_rfc3339()
            ),
            "客户端连接已建立".into(),
        ),
        MqttLocalServerEvent::ClientDisconnected {
            client_id,
            reason,
            occurred_at,
        } => (
            "客户端断开".into(),
            format!(
                "Client ID：{client_id} · 原因：{} · {}",
                reason.as_deref().unwrap_or("未提供"),
                occurred_at.to_rfc3339()
            ),
            "客户端连接已关闭".into(),
        ),
        MqttLocalServerEvent::ClientSubscribed {
            client_id,
            subscription,
            occurred_at,
        } => (
            "客户端订阅".into(),
            format!(
                "Client ID：{client_id} · {}（QoS {}{}） · {}",
                subscription.filter,
                subscription.qos.as_u8(),
                if subscription.no_local { "，No Local" } else { "" },
                occurred_at.to_rfc3339()
            ),
            "订阅已确认".into(),
        ),
        MqttLocalServerEvent::ClientUnsubscribed {
            client_id,
            filter,
            occurred_at,
        } => (
            "客户端取消订阅".into(),
            format!("Client ID：{client_id} · {filter} · {}", occurred_at.to_rfc3339()),
            "订阅已移除".into(),
        ),
        MqttLocalServerEvent::ClientPublished {
            client_id,
            message,
            occurred_at,
        } => (
            "客户端发布".into(),
            format!(
                "Client ID：{client_id} · {} · QoS {} · {} bytes{} · {}",
                message.topic,
                message.qos.as_u8(),
                message.payload.len(),
                if message.retain { " · Retain" } else { "" },
                occurred_at.to_rfc3339()
            ),
            "客户端消息已进入 Broker 路由".into(),
        ),
        MqttLocalServerEvent::BrokerPublished {
            message,
            occurred_at,
        } => (
            "Broker 发布".into(),
            format!(
                "{} · QoS {} · {} bytes{} · {}",
                message.topic,
                message.qos.as_u8(),
                message.payload.len(),
                if message.retain { " · Retain" } else { "" },
                occurred_at.to_rfc3339()
            ),
            "本地 Broker 注入消息已进入路由".into(),
        ),
    }
}

impl MqttView {
    fn render_local_server(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let running = self.local_server_running();
        let busy = self.local_server_loading
            || self.local_server_starting
            || self.local_server_stopping
            || self.local_server_publishing
            || self.local_server_snapshot_loading;
        let status_text = if self.local_server_starting {
            "正在启动本地 MQTT Broker…".to_string()
        } else if self.local_server_stopping {
            "正在停止本地 MQTT Broker…".to_string()
        } else if let Some(status) = &self.local_server_status {
            if status.running {
                format!(
                    "运行中：{} · 连接上限 {}",
                    local_server_endpoint(status),
                    status.max_connections
                )
            } else {
                "已停止".to_string()
            }
        } else {
            "尚未读取本地 Broker 状态".to_string()
        };

        let local_event_body = if self.local_server_events.is_empty() {
            div()
                .debug_selector(|| "mqtt-local-server-events-empty".into())
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(if running {
                    "等待本地 Broker 事件..."
                } else {
                    "本地 Broker 未运行。"
                })
                .into_any_element()
        } else {
            let mut events = v_flex()
                .debug_selector(|| "mqtt-local-server-events-list".into())
                .w_full()
                .min_w_0()
                .gap(px(5.0));
            for (index, event) in self.local_server_events.iter().rev().enumerate() {
                let (title, detail, summary) = local_server_event_text(event);
                events = events.child(
                    v_flex()
                        .id(SharedString::from(format!(
                            "mqtt-local-server-event-{index}"
                        )))
                        .debug_selector({
                            let selector = format!("mqtt-local-server-event-{index}");
                            move || selector.clone()
                        })
                        .w_full()
                        .min_w_0()
                        .gap(px(3.0))
                        .px(px(10.0))
                        .py(px(7.0))
                        .border_1()
                        .border_color(theme.border)
                        .rounded(px(4.0))
                        .child(
                            h_flex()
                                .flex_wrap()
                                .min_w_0()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .flex_1()
                                        .min_w_0()
                                        .child(detail),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(summary),
                        ),
                );
            }
            v_flex()
                .debug_selector(|| "mqtt-local-server-events".into())
                .w_full()
                .max_h(px(280.0))
                .min_w_0()
                .overflow_y_scrollbar()
                .child(events)
                .into_any_element()
        };
        let status_color = if self.local_server_starting || self.local_server_stopping {
            theme.warning
        } else if running {
            theme.accent
        } else {
            theme.muted_foreground
        };

        let client_snapshot_body = if self.local_server_snapshot_loading {
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("正在读取在线客户端…")
                .into_any_element()
        } else if let Some(error) = &self.local_server_snapshot_error {
            div()
                .text_xs()
                .text_color(theme.danger)
                .child(error.clone())
                .into_any_element()
        } else if let Some(snapshot) = &self.local_server_snapshot {
            let metrics = &snapshot.metrics;
            let metric_items = [
                ("当前连接", metrics.current_connections.to_string()),
                ("连接上限", metrics.max_connections.to_string()),
                ("峰值连接", metrics.peak_connections.to_string()),
                ("活跃订阅", metrics.active_subscriptions.to_string()),
                ("已发布消息", metrics.published_messages.to_string()),
                ("保留消息", metrics.retained_messages.to_string()),
                (
                    "事件队列",
                    format!(
                        "{} / {}",
                        metrics.event_queue_depth, metrics.event_queue_capacity
                    ),
                ),
                (
                    "控制队列",
                    format!(
                        "{} / {}",
                        metrics.command_queue_depth, metrics.command_queue_capacity
                    ),
                ),
            ];
            let mut metric_strip = h_flex()
                .debug_selector(|| "mqtt-local-server-metrics".into())
                .w_full()
                .flex_wrap()
                .gap(px(6.0));
            for (index, (label, value)) in metric_items.into_iter().enumerate() {
                metric_strip = metric_strip.child(
                    v_flex()
                        .id(SharedString::from(format!("mqtt-local-server-metric-{index}")))
                        .debug_selector({
                            let selector = format!("mqtt-local-server-metric-{index}");
                            move || selector.clone()
                        })
                        .w(px(132.0))
                        .min_w_0()
                        .gap(px(2.0))
                        .px(px(8.0))
                        .py(px(6.0))
                        .border_1()
                        .border_color(theme.border)
                        .rounded(px(4.0))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(label),
                        )
                        .child(div().text_sm().child(value)),
                );
            }

            let mut topics = v_flex()
                .debug_selector(|| "mqtt-local-server-topics".into())
                .w_full()
                .min_w_0()
                .gap(px(5.0));
            if snapshot.topics.is_empty() {
                topics = topics.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("当前没有已观察到的发布主题。"),
                );
            } else {
                for (index, topic) in snapshot.topics.iter().enumerate() {
                    let observed_at = topic
                        .observed_at
                        .as_ref()
                        .map_or_else(|| "时间未知".to_string(), |value| value.to_rfc3339());
                    topics = topics.child(
                        h_flex()
                            .id(SharedString::from(format!(
                                "mqtt-local-server-topic-{index}"
                            )))
                            .debug_selector({
                                let selector = format!("mqtt-local-server-topic-{index}");
                                move || selector.clone()
                            })
                            .w_full()
                            .min_w_0()
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
                                    .child(topic.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!(
                                        "发布 {} · 订阅 {} · {} bytes · {}{}",
                                        topic.publish_count,
                                        topic.subscriber_count,
                                        topic.last_payload_bytes,
                                        if topic.retained { "Retain · " } else { "" },
                                        observed_at
                                    )),
                            ),
                    );
                }
            }
            let mut clients = v_flex()
                .debug_selector(|| "mqtt-local-server-clients".into())
                .w_full()
                .gap(px(5.0));
            if snapshot.online_clients.is_empty() {
                clients = clients.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("当前没有在线客户端。"),
                );
            } else {
                for (index, client) in snapshot.online_clients.iter().enumerate() {
                    let subscriptions = if client.subscriptions.is_empty() {
                        "无订阅".to_string()
                    } else {
                        client
                            .subscriptions
                            .iter()
                            .map(|subscription| {
                                format!(
                                    "{}（QoS {}{}）",
                                    subscription.filter,
                                    subscription.qos.as_u8(),
                                    if subscription.no_local { "，No Local" } else { "" }
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("；")
                    };
                    let identity = format!(
                        "{}{}",
                        client.client_id,
                        client
                            .username
                            .as_deref()
                            .map_or(String::new(), |username| format!(" · {username}"))
                    );
                    let address = client
                        .remote_address
                        .as_deref()
                        .unwrap_or("地址未知")
                        .to_string();
                    let connected_at = client
                        .connected_at
                        .as_ref()
                        .map_or_else(|| "连接时间未知".to_string(), |value| value.to_rfc3339());
                    clients = clients.child(
                        v_flex()
                            .id(SharedString::from(format!(
                                "mqtt-local-server-client-{index}"
                            )))
                            .debug_selector({
                                let selector = format!("mqtt-local-server-client-{index}");
                                move || selector.clone()
                            })
                            .w_full()
                            .min_w_0()
                            .gap(px(3.0))
                            .px(px(10.0))
                            .py(px(7.0))
                            .border_1()
                            .border_color(theme.border)
                            .rounded(px(4.0))
                            .child(
                                div()
                                    .text_sm()
                                    .min_w_0()
                                    .truncate()
                                    .child(identity),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .min_w_0()
                                    .truncate()
                                    .child(format!(
                                        "{address} · 连接：{connected_at} · 订阅：{subscriptions}"
                                    )),
                            ),
                    );
                }
            }
            v_flex()
                .debug_selector(|| "mqtt-local-server-snapshot".into())
                .w_full()
                .min_w_0()
                .gap(px(10.0))
                .child(metric_strip)
                .child(section_heading(
                    "主题目录",
                    if snapshot.topics_complete {
                        "服务端已枚举当前运行期间观察到的主题和保留消息。"
                    } else {
                        "主题目录达到容量上限，当前列表只包含部分服务端观察记录。"
                    },
                    &theme,
                ))
                .child(topics)
                .child(section_heading(
                    "在线客户端",
                    "真实连接数、Client ID、远端地址和订阅关系来自 Broker 当前会话。",
                    &theme,
                ))
                .child(
                    div()
                        .debug_selector(|| "mqtt-local-server-clients-summary".into())
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "在线客户端 {} 个；客户端数据{}完整；丢弃事件 {} 条。",
                            snapshot.online_clients.len(),
                            if snapshot.online_clients_complete {
                                ""
                            } else {
                                "不"
                            },
                            metrics.dropped_events
                        )),
                )
                .child(clients)
                .into_any_element()
        } else {
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("尚未读取本地 Broker 快照；启动服务后刷新即可查看连接、主题和队列指标。")
                .into_any_element()
        };

        let server_action = if running {
            ramag_ui::clickable_button("mqtt-local-server-stop")
                .debug_selector(|| "mqtt-local-server-stop".into())
                .danger()
                .small()
                .label("停止服务")
                .loading(self.local_server_stopping)
                .disabled(busy)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.stop_local_server(window, cx)
                }))
        } else {
            ramag_ui::clickable_button("mqtt-local-server-start")
                .debug_selector(|| "mqtt-local-server-start".into())
                .primary()
                .small()
                .icon(IconName::Play)
                .label("启动服务")
                .loading(self.local_server_starting)
                .disabled(busy)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.start_local_server(window, cx)
                }))
        };

        let mut users = v_flex().w_full().gap(px(5.0));
        if self.local_server_users.is_empty() {
            users = users.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("未配置固定账号；关闭匿名连接后，客户端必须使用下方账号。"),
            );
        } else {
            for (index, user) in self.local_server_users.iter().enumerate() {
                let selector = SharedString::from(format!("mqtt-local-server-user-{index}"));
                let username = user.username.clone();
                users = users.child(
                    h_flex()
                        .id(selector.clone())
                        .debug_selector({
                            let selector = selector.clone();
                            move || selector.to_string()
                        })
                        .w_full()
                        .min_w_0()
                        .gap(px(8.0))
                        .px(px(10.0))
                        .py(px(7.0))
                        .border_1()
                        .border_color(theme.border)
                        .rounded(px(4.0))
                        .child(Icon::new(IconName::User).small().text_color(theme.accent))
                        .child(
                            div()
                                .text_sm()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(username),
                        )
                        .child(
                            ramag_ui::clickable_button(SharedString::from(format!(
                                "mqtt-local-server-remove-user-{index}"
                            )))
                            .debug_selector({
                                let selector = format!("mqtt-local-server-remove-user-{index}");
                                move || selector.clone()
                            })
                            .ghost()
                            .xsmall()
                            .icon(IconName::Delete)
                            .tooltip("移除账号")
                            .disabled(busy || running)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.remove_local_server_user(index, cx)
                            })),
                        ),
                );
            }
        }

        let mut body = v_flex()
            .w_full()
            .min_w_0()
            .max_w(px(920.0))
            .gap(px(14.0))
            .child(section_heading(
                "本地 MQTT Broker",
                "参考 Wu.CommTool 的本地服务能力；监听地址、端口和账号调整在下次启动时生效，绑定 0.0.0.0 会暴露到所有网卡。",
                &theme,
            ))
            .child(
                row()
                    .debug_selector(|| "mqtt-local-server-config".into())
                    .child(field(
                        "监听地址",
                        input_frame(
                            "mqtt-local-server-bind-host-input",
                            Input::new(&self.local_server_bind_host)
                                .small()
                                .disabled(running || busy)
                                .w_full()
                                .min_w_0(),
                        ),
                    ))
                    .child(field(
                        "端口",
                        input_frame(
                            "mqtt-local-server-port-input",
                            Input::new(&self.local_server_port)
                                .small()
                                .disabled(running || busy)
                                .w_full()
                                .min_w_0(),
                        ),
                    ))
                    .child(field(
                        "连接上限",
                        input_frame(
                            "mqtt-local-server-max-connections-input",
                            Input::new(&self.local_server_max_connections)
                                .small()
                                .disabled(running || busy)
                                .w_full()
                                .min_w_0(),
                        ),
                    )),
            )
            .child(
                row()
                    .debug_selector(|| "mqtt-local-server-actions".into())
                    .child(toggle_button(
                        "mqtt-local-server-anonymous",
                        "匿名连接",
                        self.local_server_allow_anonymous,
                        busy || running,
                        cx,
                        |this| this.toggle_local_server_anonymous(),
                    ))
                    .child(server_action)
                    .child(
                        ramag_ui::clickable_button("mqtt-local-server-refresh")
                            .debug_selector(|| "mqtt-local-server-refresh".into())
                            .ghost()
                            .small()
                            .icon(ramag_ui::icons::refresh_cw())
                            .tooltip("读取服务状态")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.load_local_server_status(window, cx)
                            })),
                    )
                    .child(
                        ramag_ui::clickable_button("mqtt-local-server-use-client")
                            .debug_selector(|| "mqtt-local-server-use-client".into())
                            .ghost()
                            .small()
                            .label("填入客户端配置")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.use_local_server_for_client(window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "mqtt-local-server-status".into())
                    .text_sm()
                    .text_color(status_color)
                    .child(status_text),
            )
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(section_heading(
                        "在线客户端",
                        "读取本地 Broker 的连接、主题、订阅和队列指标；所有数值来自服务端快照。",
                        &theme,
                    ))
                    .child(
                        ramag_ui::clickable_button("mqtt-local-server-refresh-clients")
                            .debug_selector(|| "mqtt-local-server-refresh-clients".into())
                            .ghost()
                            .small()
                            .icon(ramag_ui::icons::refresh_cw())
                            .tooltip("读取在线客户端")
                            .disabled(!running || busy)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.load_local_server_snapshot(window, cx)
                            })),
                    ),
            )
            .child(client_snapshot_body)
            .child(section_heading(
                "Broker 注入发布",
                "消息由本地 Broker 直接送入订阅路由，不创建额外 MQTT 客户端连接；载荷格式、QoS 和 Retain 与客户端发布保持一致。",
                &theme,
            ))
            .child(
                field(
                    "Topic",
                    input_frame(
                        "mqtt-local-server-publish-topic-input",
                        Input::new(&self.local_server_publish_topic)
                            .small()
                            .disabled(!running || busy)
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
                        "mqtt-local-server-publish-payload-input",
                        Input::new(&self.local_server_publish_payload)
                            .h(px(140.0))
                            .small()
                            .disabled(!running || busy)
                            .w_full()
                            .min_w_0(),
                    ),
                )
                .w_full(),
            )
            .child(
                row()
                    .debug_selector(|| "mqtt-local-server-publish-options".into())
                    .child(payload_format_selector(
                        "mqtt-local-server-publish-payload-format",
                        self.local_server_publish_payload_format,
                        !running || busy,
                        cx,
                        |this, format| this.local_server_publish_payload_format = format,
                    ))
                    .child(qos_selector(
                        "mqtt-local-server-publish-qos",
                        self.local_server_publish_qos,
                        !running || busy,
                        cx,
                        |this, qos| this.local_server_publish_qos = qos,
                    ))
                    .child(toggle_button(
                        "mqtt-local-server-publish-retain",
                        "Retain",
                        self.local_server_publish_retain,
                        !running || busy,
                        cx,
                        |this| this.local_server_publish_retain = !this.local_server_publish_retain,
                    )),
            )
            .child(
                div()
                    .debug_selector(|| "mqtt-local-server-publish-actions".into())
                    .self_start()
                    .child(
                        ramag_ui::clickable_button("mqtt-local-server-publish")
                            .debug_selector(|| "mqtt-local-server-publish".into())
                            .primary()
                            .small()
                            .flex_none()
                            .self_start()
                            .label("注入发布")
                            .loading(self.local_server_publishing)
                            .disabled(!running || busy)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.publish_local_server(window, cx)
                            })),
                    ),
            )
            .child(section_heading(
                "固定账号",
                "账号密码只在当前视图中保留；使用“填入客户端配置”后会复制到客户端表单，是否保存由你决定。",
                &theme,
            ))
            .child(
                row()
                    .debug_selector(|| "mqtt-local-server-user-editor".into())
                    .child(field(
                        "用户名",
                        input_frame(
                            "mqtt-local-server-username-input",
                            Input::new(&self.local_server_username)
                                .small()
                                .disabled(busy || running)
                                .w_full()
                                .min_w_0(),
                        ),
                    ))
                    .child(field(
                        "密码",
                        input_frame(
                            "mqtt-local-server-password-input",
                            Input::new(&self.local_server_password)
                                .small()
                                .mask_toggle()
                                .disabled(busy || running)
                                .w_full()
                                .min_w_0(),
                        ),
                    ))
                    .child(
                        ramag_ui::clickable_button("mqtt-local-server-add-user")
                            .debug_selector(|| "mqtt-local-server-add-user".into())
                            .primary()
                            .small()
                            .icon(IconName::Plus)
                            .label("添加账号")
                            .disabled(busy || running)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.add_local_server_user(window, cx)
                            })),
                    ),
            )
            .child(users);
        body = body
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(section_heading(
                        "Broker 事件",
                        "来自本地 Broker 的真实连接、订阅、发布和断开事件；事件列表只保留最近记录。",
                        &theme,
                    ))
                    .child(
                        ramag_ui::clickable_button("mqtt-local-server-events-clear")
                            .debug_selector(|| "mqtt-local-server-events-clear".into())
                            .ghost()
                            .small()
                            .icon(IconName::Delete)
                            .tooltip("清空 Broker 事件")
                            .disabled(self.local_server_events.is_empty())
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.clear_local_server_events();
                                cx.notify();
                            })),
                    ),
            )
            .child(local_event_body);
        if let Some((message, is_error)) = self.local_server_notice.as_ref() {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(if *is_error {
                        theme.danger
                    } else {
                        theme.muted_foreground
                    })
                    .child(message.clone()),
            );
        }

        v_flex()
            .id("mqtt-local-server-scroll")
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
}
