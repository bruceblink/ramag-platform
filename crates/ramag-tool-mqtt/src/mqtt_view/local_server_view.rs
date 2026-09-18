impl MqttView {
    fn render_local_server(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let running = self.local_server_running();
        let busy = self.local_server_loading
            || self.local_server_starting
            || self.local_server_stopping
            || self.local_server_publishing;
        let status_text = if self.local_server_starting {
            "正在启动本地 MQTT Broker…".to_string()
        } else if self.local_server_stopping {
            "正在停止本地 MQTT Broker…".to_string()
        } else if let Some(status) = &self.local_server_status {
            if status.running {
                format!("运行中：{}", local_server_endpoint(status))
            } else {
                "已停止".to_string()
            }
        } else {
            "尚未读取本地 Broker 状态".to_string()
        };
        let status_color = if self.local_server_starting || self.local_server_stopping {
            theme.warning
        } else if running {
            theme.accent
        } else {
            theme.muted_foreground
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
