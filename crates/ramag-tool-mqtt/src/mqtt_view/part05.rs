impl MqttView {
    fn render_sidebar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let narrow = Self::sidebar_is_narrow(window);
        let search = value(&self.search, cx);
        let mut rows = v_flex().w_full().min_h_0();
        for profile in self.profiles.iter().filter(|profile| {
            search.is_empty() || profile.name.contains(&search) || profile.host.contains(&search)
        }) {
            let selected = self.selected_profile_id.as_ref() == Some(&profile.id);
            let id = profile.id.clone();
            rows = rows.child(
                h_flex()
                    .id(SharedString::from(format!("mqtt-profile-{}", profile.id)))
                    .w_full()
                    .min_w_0()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(10.0))
                    .when(selected, |row| row.bg(theme.accent.opacity(0.12)))
                    .when(!selected, |row| {
                        row.hover(|row| row.bg(theme.muted.opacity(0.45)))
                    })
                    .when(!self.is_busy(), |row| {
                        row.cursor_pointer().on_click(cx.listener(
                            move |this, _: &ClickEvent, window, cx| {
                                this.select_profile(id.clone(), window, cx);
                            },
                        ))
                    })
                    .child(div().size(px(8.0)).rounded_full().bg(if selected {
                        theme.accent
                    } else {
                        theme.muted_foreground
                    }))
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(div().text_sm().truncate().child(profile.name.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(format!("{}:{}", profile.host, profile.port)),
                            ),
                    ),
            );
        }
        v_flex()
            .id("mqtt-sidebar")
            .debug_selector(|| "mqtt-sidebar".into())
            .w(px(MQTT_SIDEBAR_WIDTH))
            .min_w(px(210.0))
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.secondary.opacity(0.45))
            .child(
                h_flex()
                    .w_full()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .px(px(14.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(Icon::new(IconName::Network).text_color(theme.accent))
                            .child(
                                v_flex()
                                    .gap(px(1.0))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child("MQTT"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child("Broker 配置"),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap(px(4.0))
                            .when(narrow, |actions| {
                                actions.child(
                                    ramag_ui::clickable_button("mqtt-hide-sidebar")
                                        .debug_selector(|| "mqtt-hide-sidebar".into())
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::PanelLeft)
                                        .tooltip("隐藏配置栏")
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.sidebar_visible = false;
                                            cx.notify();
                                        })),
                                )
                            })
                            .child(
                                ramag_ui::clickable_button("mqtt-add-profile")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Plus)
                                    .tooltip("新建配置")
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.new_profile(window, cx)
                                    })),
                            ),
                    ),
            )
            .child(
                div().w_full().flex_none().px(px(12.0)).py(px(10.0)).child(
                    ramag_ui::cleanable_input(&self.search, "mqtt-search-clear", false, cx)
                        .small()
                        .prefix(
                            Icon::new(IconName::Search)
                                .small()
                                .text_color(theme.muted_foreground),
                        ),
                ),
            )
            .child(
                div()
                    .id("mqtt-profile-list-scroll")
                    .w_full()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(rows),
            )
            .child(
                div()
                    .w_full()
                    .flex_none()
                    .px(px(12.0))
                    .py(px(12.0))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(div().text_xs().text_color(theme.muted_foreground).child(
                        if self.loading_profiles {
                            "正在加载本地配置…".to_string()
                        } else {
                            format!("{} 个本地配置", self.profiles.len())
                        },
                    )),
            )
            .when(narrow, |sidebar| {
                sidebar
                    .w_full()
                    .min_w_0()
                    .h(px(220.0))
                    .border_r_0()
                    .border_b_1()
            })
            .when(
                !narrow && window.viewport_size().width < px(760.0),
                |sidebar| sidebar.w(px(210.0)),
            )
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tabs = h_flex().flex_wrap().gap(px(4.0));
        for section in MqttSection::ALL {
            let mut button =
                ramag_ui::clickable_button(SharedString::from(format!("mqtt-tab-{:?}", section)))
                    .xsmall()
                    .label(section.label());
            button = if self.section == section {
                button.primary()
            } else {
                button.ghost()
            };
            tabs = tabs.child(button.on_click(
                cx.listener(move |this, _: &ClickEvent, _, cx| this.select_section(section, cx)),
            ));
        }
        tabs
    }

    fn render_header(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let narrow = Self::sidebar_is_narrow(window);
        let selected_name = self.selected_profile().map_or_else(
            || "新建 MQTT 配置".to_string(),
            |profile| profile.name.clone(),
        );
        let mut actions = h_flex()
            .flex_wrap()
            .items_center()
            .justify_end()
            .gap(px(6.0));
        if self.selected_profile_id.is_some() {
            actions = actions.child(
                ramag_ui::clickable_button("mqtt-delete-profile")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Delete)
                    .tooltip("删除配置")
                    .disabled(self.is_busy() || self.subscription_running)
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.delete_profile(window, cx)
                    })),
            );
        }
        actions = actions
            .child(
                ramag_ui::clickable_button("mqtt-test-connection")
                    .ghost()
                    .small()
                    .label("测试连接")
                    .loading(self.testing)
                    .disabled(self.is_busy() || self.subscription_running)
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.test_connection(window, cx)
                    })),
            )
            .child(
                ramag_ui::clickable_button("mqtt-save-profile")
                    .primary()
                    .small()
                    .label("保存")
                    .loading(self.saving)
                    .disabled(self.is_busy() || self.subscription_running)
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.save_profile(window, cx)
                    })),
            )
            .when(narrow && !self.sidebar_visible, |actions| {
                actions.child(
                    ramag_ui::clickable_button("mqtt-show-sidebar")
                        .debug_selector(|| "mqtt-show-sidebar".into())
                        .ghost()
                        .small()
                        .icon(IconName::PanelLeft)
                        .tooltip("显示配置栏")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.sidebar_visible = true;
                            cx.notify();
                        })),
                )
            });
        v_flex()
            .w_full()
            .flex_none()
            .gap(px(10.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.secondary.opacity(0.35))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .when(window.viewport_size().width < px(900.0), |row| {
                        row.flex_col().items_stretch()
                    })
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(selected_name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("真实 Broker 数据由 Native MQTT 驱动返回"),
                            ),
                    )
                    .child(actions),
            )
            .child(self.render_tabs(cx))
    }

    fn render_config(&self, _window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut protocol_buttons = h_flex()
            .debug_selector(|| "mqtt-protocol-buttons".into())
            .gap(px(4.0));
        for (protocol, label) in [
            (MqttProtocolVersion::V5, "MQTT 5.0"),
            (MqttProtocolVersion::V311, "MQTT 3.1.1"),
        ] {
            let mut button =
                ramag_ui::clickable_button(SharedString::from(format!("mqtt-protocol-{label}")))
                    .xsmall()
                    .label(label);
            button = if self.protocol == protocol {
                button.primary()
            } else {
                button.ghost()
            };
            protocol_buttons = protocol_buttons.child(button.on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.protocol = protocol;
                    cx.notify();
                },
            )));
        }
        let mut transport_buttons = h_flex().gap(px(4.0));
        for (transport, label) in [(TransportKind::Tcp, "TCP"), (TransportKind::Tls, "TLS")] {
            let mut button =
                ramag_ui::clickable_button(SharedString::from(format!("mqtt-transport-{label}")))
                    .xsmall()
                    .label(label);
            button = if self.transport == transport {
                button.primary()
            } else {
                button.ghost()
            };
            transport_buttons = transport_buttons.child(button.on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.transport = transport;
                    cx.notify();
                },
            )));
        }
        let content = v_flex()
            .w_full()
            .max_w(px(920.0))
            .gap(px(14.0))
            .child(section_heading(
                "连接配置",
                "凭据和证书只通过本机加密存储保存",
                &theme,
            ))
            .child(
                row()
                    .child(field("名称", Input::new(&self.name).small()))
                    .child(field("Broker 地址", Input::new(&self.host).small()))
                    .child(field("端口", Input::new(&self.port).small())),
            )
            .child(
                row()
                    .child(
                        field("协议版本", protocol_buttons)
                            .debug_selector(|| "mqtt-protocol-field".into()),
                    )
                    .child(field("传输", transport_buttons))
                    .child(field("Keep Alive", Input::new(&self.keep_alive).small())),
            )
            .child(
                row()
                    .child(field("Client ID", Input::new(&self.client_id).small()))
                    .child(field("用户名", Input::new(&self.username).small()))
                    .child(field(
                        "密码",
                        Input::new(&self.password).small().mask_toggle(),
                    )),
            )
            .child(section_heading(
                "TLS 文件",
                "只有 TLS 传输会读取这些路径；客户端证书和密钥必须成对配置",
                &theme,
            ))
            .child(
                row()
                    .child(field("CA 证书", Input::new(&self.ca_cert_path).small()))
                    .child(field(
                        "客户端证书",
                        Input::new(&self.client_cert_path).small(),
                    ))
                    .child(field(
                        "客户端密钥",
                        Input::new(&self.client_key_path).small(),
                    )),
            )
            .when(self.management_enabled, |content| {
                content
                    .child(section_heading(
                        "Mosquitto 管理凭据",
                        "Dynamic Security 请求优先使用这里的账号；密码不会从 Broker 读取或回显",
                        &theme,
                    ))
                    .child(
                        row()
                            .child(field(
                                "管理用户名",
                                Input::new(&self.management_admin_username).small(),
                            ))
                            .child(field(
                                "管理密码",
                                Input::new(&self.management_admin_password)
                                    .small()
                                    .mask_toggle(),
                            )),
                    )
                    .child(section_heading(
                        "静态配置文件",
                        "配置绝对路径后，Mosquitto 页面才能读取或保存本机文件",
                        &theme,
                    ))
                    .child(
                        row()
                            .child(field(
                                "password_file",
                                Input::new(&self.password_file_path).small(),
                            ))
                            .child(field("acl_file", Input::new(&self.acl_file_path).small())),
                    )
            })
            .child(
                row()
                    .child(toggle_button(
                        "mqtt-clean-start",
                        "Clean Start",
                        self.clean_start,
                        self.is_busy(),
                        cx,
                        |this| this.clean_start = !this.clean_start,
                    ))
                    .child(toggle_button(
                        "mqtt-management",
                        "启用 Mosquitto 管理",
                        self.management_enabled,
                        self.is_busy(),
                        cx,
                        |this| this.management_enabled = !this.management_enabled,
                    ))
                    .child(div().flex_1().min_w_0()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("保存后才能在状态、发布、订阅和 Mosquitto 页面使用此配置。"),
            );
        div()
            .id("mqtt-config-scroll")
            .w_full()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p(px(16.0))
            .child(content)
    }

}
