impl MqttView {
    fn render_management_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tabs = h_flex().flex_wrap().gap(px(4.0));
        for section in MosquittoManagementSection::ALL {
            let selector = SharedString::from(format!("mqtt-management-tab-{section:?}"));
            let mut button = ramag_ui::clickable_button(selector.clone())
            .debug_selector({
                let selector = selector.clone();
                move || selector.to_string()
            })
            .xsmall()
            .label(section.label());
            button = if self.management_section == section {
                button.primary()
            } else {
                button.ghost()
            };
            tabs = tabs.child(
                button.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.select_management_section(section, cx)
                })),
            );
        }
        tabs
    }

    fn render_clients(&self, cx: &mut Context<Self>, narrow: bool) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let mut list = v_flex().gap(px(4.0));
        if let Some(snapshot) = &self.management_snapshot {
            for client in &snapshot.clients {
                let username = client.username.clone();
                let selected = self.selected_client_username.as_ref() == Some(&username);
                list = list.child(
                    h_flex()
                        .id(SharedString::from(format!("mqtt-client-{username}")))
                        .w_full()
                        .gap(px(8.0))
                        .p(px(8.0))
                        .rounded(px(4.0))
                        .when(selected, |row| row.bg(theme.accent.opacity(0.12)))
                        .when(!selected, |row| {
                            row.hover(|row| row.bg(theme.muted.opacity(0.45)))
                        })
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.select_client(username.clone(), window, cx)
                        }))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_xs().truncate().child(client.username.clone()))
                                .child(div().text_xs().text_color(theme.muted_foreground).child(
                                    format!(
                                        "{} 个 Group · {} 个 Role · 密码{}",
                                        client.groups.len(),
                                        client.roles.len(),
                                        if client.password_configured {
                                            "已配置"
                                        } else {
                                            "未配置"
                                        }
                                    ),
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(if client.disabled {
                                    theme.danger
                                } else {
                                    theme.muted_foreground
                                })
                                .child(if client.disabled {
                                    "已禁用"
                                } else {
                                    "启用"
                                }),
                        ),
                );
            }
        }
        let editor = v_flex()
            .id("mqtt-client-editor")
            .debug_selector(|| "mqtt-client-editor".into())
            .w_full()
            .gap(px(8.0))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .items_end()
                    .gap(px(8.0))
                    .justify_between()
                    .child(div().flex_1().min_w_0().child(section_heading(
                        "用户编辑",
                        "Group 和 Role 使用逗号分隔；密码只在提交时发送",
                        &theme,
                    )))
                    .child(
                        ramag_ui::clickable_button("mqtt-client-new")
                            .ghost()
                            .xsmall()
                            .label("新建用户")
                            .disabled(self.saving_management || self.deleting_management)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.new_client(window, cx)
                            })),
                    ),
            )
            .child(
                row()
                    .id("mqtt-client-identity-fields")
                    .debug_selector(|| "mqtt-client-identity-fields".into())
                    .when(narrow, |row| row.flex_col().items_stretch())
                    .child(field("用户名", Input::new(&self.client_username).small()))
                    .child(field(
                        "Client ID",
                        Input::new(&self.client_id_editor).small(),
                    ))
                    .child(field(
                        "新密码",
                        Input::new(&self.client_password).small().mask_toggle(),
                    )),
            )
            .child(
                row()
                    .id("mqtt-client-display-fields")
                    .debug_selector(|| "mqtt-client-display-fields".into())
                    .when(narrow, |row| row.flex_col().items_stretch())
                    .child(field(
                        "显示名称",
                        Input::new(&self.client_text_name).small(),
                    ))
                    .child(field(
                        "描述",
                        Input::new(&self.client_text_description).small(),
                    )),
            )
            .child(
                row()
                    .id("mqtt-client-membership-fields")
                    .debug_selector(|| "mqtt-client-membership-fields".into())
                    .when(narrow, |row| row.flex_col().items_stretch())
                    .child(field("Group", Input::new(&self.client_groups).small()))
                    .child(field("Role", Input::new(&self.client_roles).small())),
            )
            .child(toggle_button(
                "mqtt-client-disabled",
                "用户状态",
                self.client_disabled,
                self.saving_management || self.deleting_management,
                cx,
                |this| this.client_disabled = !this.client_disabled,
            ))
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .gap(px(6.0))
                    .child(
                        ramag_ui::clickable_button("mqtt-client-save")
                            .debug_selector(|| "mqtt-client-save".into())
                            .primary()
                            .small()
                            .label("保存用户")
                            .loading(self.saving_management)
                            .disabled(self.saving_management || self.deleting_management)
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| this.save_client(cx)),
                            ),
                    )
                    .child(
                        ramag_ui::clickable_button("mqtt-client-delete")
                            .danger()
                            .small()
                            .label("删除用户")
                            .disabled(
                                self.selected_client_username.is_none()
                                    || self.saving_management
                                    || self.deleting_management,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.confirm_delete_client(window, cx)
                            })),
                    ),
            );
        let has_clients = self
            .management_snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.clients.is_empty());
        v_flex()
            .w_full()
            .gap(px(12.0))
            .child(section_heading(
                "用户列表",
                "来自 Dynamic Security 的真实用户和绑定关系",
                &theme,
            ))
            .child(if !has_clients {
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("暂无用户或尚未读取列表")
                    .into_any_element()
            } else {
                list.into_any_element()
            })
            .child(editor)
            .child(self.render_client_permissions(cx, narrow))
            .into_any_element()
    }

    fn render_client_permissions(
        &self,
        cx: &mut Context<Self>,
        narrow: bool,
    ) -> gpui_kit::AnyElement {
        let theme = cx.theme().clone();
        let Some(snapshot) = self.management_snapshot.as_ref() else {
            return div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("读取 Dynamic Security 后才能展开用户权限")
                .into_any_element();
        };
        let Some(username) = self.selected_client_username.as_deref() else {
            return div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("选择用户后显示直接 Role 和 Group 继承的 ACL")
                .into_any_element();
        };
        let Some(client) = snapshot
            .clients
            .iter()
            .find(|client| client.username == username)
        else {
            return div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("当前用户已不在最近一次 Broker 返回的列表中")
                .into_any_element();
        };

        let (rows, missing_roles) = client_permission_rows(snapshot, client);
        let mut body = v_flex().w_full().gap(px(4.0));
        if rows.is_empty() {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("该用户当前没有可展开的 ACL"),
            );
        } else {
            if !narrow {
                body = body.child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .gap(px(8.0))
                        .child(div().w(px(130.0)).text_xs().child("来源"))
                        .child(div().w(px(130.0)).text_xs().child("Role"))
                        .child(div().flex_1().min_w_0().text_xs().child("Topic"))
                        .child(div().w(px(180.0)).text_xs().child("权限 / 优先级")),
                );
            }
            for (index, row) in rows.into_iter().enumerate() {
                let decision = match row.acl.decision {
                    MosquittoAclDecision::Allow => "允许",
                    MosquittoAclDecision::Deny => "拒绝",
                };
                let row_id = SharedString::from(format!("mqtt-client-permission-row-{index}"));
                let permission = format!(
                    "{} · {} · ACL {} / 绑定 {}",
                    row.acl.acl_type.as_str(),
                    decision,
                    row.acl.priority,
                    row.binding_priority
                );
                if narrow {
                    body = body.child(
                        v_flex()
                            .id(row_id.clone())
                            .debug_selector(move || row_id.to_string())
                            .w_full()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .text_xs()
                                    .truncate()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("来源：{} · Role：{}", row.source, row.role_name)),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .text_xs()
                                    .truncate()
                                    .child(format!("Topic：{}", row.acl.topic)),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .text_xs()
                                    .truncate()
                                    .text_color(
                                        if matches!(row.acl.decision, MosquittoAclDecision::Allow)
                                        {
                                            theme.accent
                                        } else {
                                            theme.danger
                                        },
                                    )
                                    .child(permission),
                            ),
                    );
                } else {
                    body = body.child(
                        h_flex()
                            .id(row_id.clone())
                            .debug_selector(move || row_id.to_string())
                            .w_full()
                            .min_w_0()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .w(px(130.0))
                                    .text_xs()
                                    .truncate()
                                    .text_color(theme.muted_foreground)
                                    .child(row.source),
                            )
                            .child(div().w(px(130.0)).text_xs().truncate().child(row.role_name))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .truncate()
                                    .child(row.acl.topic),
                            )
                            .child(
                                div()
                                    .w(px(180.0))
                                    .text_xs()
                                    .truncate()
                                    .text_color(
                                        if matches!(row.acl.decision, MosquittoAclDecision::Allow) {
                                            theme.accent
                                        } else {
                                            theme.danger
                                        },
                                    )
                                    .child(permission),
                            ),
                    );
                }
            }
        }
        if !missing_roles.is_empty() {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(theme.danger)
                    .child(format!("未找到绑定的 Role：{}", missing_roles.join("、"))),
            );
        }
        v_flex()
            .w_full()
            .gap(px(8.0))
            .child(section_heading(
                "用户权限预览",
                "按用户直接绑定和 Group 继承的 Role 展开；最终判定仍由 Mosquitto Broker 执行",
                &theme,
            ))
            .child(body)
            .into_any_element()
    }

}
