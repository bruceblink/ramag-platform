impl MqttView {
    fn render_static_files(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme().clone();
        let mut kind_buttons = h_flex().gap(px(4.0));
        for kind in [
            MosquittoStaticFileKind::Password,
            MosquittoStaticFileKind::Acl,
        ] {
            let mut button = ramag_ui::clickable_button(SharedString::from(format!(
                "mqtt-static-kind-{:?}",
                kind
            )))
            .xsmall()
            .label(kind.label());
            button = if self.static_file_kind == kind {
                button.primary()
            } else {
                button.ghost()
            };
            kind_buttons = kind_buttons.child(button.on_click(cx.listener(
                move |this, _: &ClickEvent, window, cx| {
                    this.static_file_kind = kind;
                    this.static_file = None;
                    set_value(&this.static_content, "", window, cx);
                    cx.notify();
                },
            )));
        }
        let configured_path = self
            .selected_profile()
            .and_then(|profile| self.static_file_kind.configured_path(profile))
            .unwrap_or("未配置");
        let ssh_target = self.selected_profile().is_some_and(|profile| {
            matches!(
                profile
                    .management
                    .static_config
                    .as_ref()
                    .map(|config| &config.target),
                Some(MosquittoConfigTarget::Ssh { .. })
            )
        });
        v_flex()
            .w_full()
            .gap(px(10.0))
            .child(section_heading(
                "静态配置文件",
                "文件内容来自本机文件驱动；保存成功后仍需 Broker reload 或重启",
                &theme,
            ))
            .child(kind_buttons)
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!(
                        "当前 {} 路径：{}",
                        self.static_file_kind.label(),
                        configured_path
                    )),
            )
            .when(ssh_target, |body| {
                body.child(
                    div()
                        .text_xs()
                        .text_color(theme.danger)
                        .child("当前配置是 SSH 目标；本机文件驱动不会伪装成远程文件管理。"),
                )
            })
            .child(
                h_flex()
                    .gap(px(6.0))
                    .child(
                        ramag_ui::clickable_button("mqtt-static-load")
                            .ghost()
                            .small()
                            .label("读取文件")
                            .loading(self.loading_static_file)
                            .disabled(self.loading_static_file || self.saving_static_file)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.load_static_file(window, cx)
                            })),
                    )
                    .child(
                        ramag_ui::clickable_button("mqtt-static-save")
                            .primary()
                            .small()
                            .label("保存文件")
                            .loading(self.saving_static_file)
                            .disabled(
                                self.static_file.is_none()
                                    || self.loading_static_file
                                    || self.saving_static_file,
                            )
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.save_static_file(cx)
                                }),
                            ),
                    ),
            )
            .child(field(
                "文件内容",
                Input::new(&self.static_content).h(px(300.0)).small(),
            ))
            .into_any_element()
    }

    fn render_mosquitto(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut body = v_flex()
            .w_full()
            .max_w(px(980.0))
            .gap(px(12.0))
            .child(section_heading(
                "Mosquitto 管理",
                "Dynamic Security 和静态文件分别通过明确的管理接口读取，不生成演示数据",
                &theme,
            ));
        if !self.management_enabled {
            body = body.child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("当前配置未启用 Mosquitto 管理能力。"),
            );
        } else {
            body = body
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap(px(6.0))
                        .child(
                            ramag_ui::clickable_button("mqtt-load-management")
                                .ghost()
                                .small()
                                .label("读取 Dynamic Security")
                                .loading(self.loading_management)
                                .disabled(
                                    self.loading_management
                                        || self.saving_management
                                        || self.deleting_management
                                        || self.selected_profile_id.is_none(),
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.load_management(cx)
                                })),
                        )
                        .child(self.render_management_tabs(cx)),
                )
                .child(if let Some(error) = &self.management_error {
                    div()
                        .text_sm()
                        .text_color(theme.danger)
                        .child(error.clone())
                        .into_any_element()
                } else if self.loading_management && self.management_snapshot.is_none() {
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("正在读取 Dynamic Security…")
                        .into_any_element()
                } else if let Some(snapshot) = &self.management_snapshot {
                    let metrics = h_flex().flex_wrap().gap(px(8.0)).children([
                        metric("用户", snapshot.clients.len(), &theme),
                        metric("Group", snapshot.groups.len(), &theme),
                        metric("Role", snapshot.roles.len(), &theme),
                    ]);
                    let panel = match self.management_section {
                        MosquittoManagementSection::Clients => self.render_clients(cx),
                        MosquittoManagementSection::Groups => self.render_groups(cx),
                        MosquittoManagementSection::Roles => self.render_roles(cx),
                        MosquittoManagementSection::StaticFiles => self.render_static_files(cx),
                    };
                    v_flex()
                        .w_full()
                        .gap(px(12.0))
                        .child(metrics)
                        .child(panel)
                        .into_any_element()
                } else {
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("尚未读取 Mosquitto 管理数据。")
                        .into_any_element()
                });
        }
        v_flex()
            .id("mqtt-mosquitto-scroll")
            .w_full()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p(px(16.0))
            .child(body)
    }
}
