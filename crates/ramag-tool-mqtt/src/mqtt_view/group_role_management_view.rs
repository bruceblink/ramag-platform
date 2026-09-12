impl MqttView {
    fn render_groups(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme().clone();
        let mut list = v_flex().gap(px(4.0));
        if let Some(snapshot) = &self.management_snapshot {
            for group in &snapshot.groups {
                let group_name = group.group_name.clone();
                let selected = self.selected_group_name.as_ref() == Some(&group_name);
                list = list.child(
                    h_flex()
                        .id(SharedString::from(format!("mqtt-group-{group_name}")))
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
                            this.select_group(group_name.clone(), window, cx)
                        }))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_xs().truncate().child(group.group_name.clone()))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(format!("{} 个 Role", group.roles.len())),
                                ),
                        ),
                );
            }
        }
        let has_groups = self
            .management_snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.groups.is_empty());
        v_flex()
            .w_full()
            .gap(px(12.0))
            .child(section_heading(
                "Group 列表",
                "Group 只保存 Role 绑定，用户绑定由用户编辑器维护",
                &theme,
            ))
            .child(if !has_groups {
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("暂无 Group 或尚未读取列表")
                    .into_any_element()
            } else {
                list.into_any_element()
            })
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(
                        h_flex()
                            .justify_between()
                            .child(section_heading(
                                "Group 编辑",
                                "Role 名称使用逗号分隔",
                                &theme,
                            ))
                            .child(
                                ramag_ui::clickable_button("mqtt-group-new")
                                    .ghost()
                                    .xsmall()
                                    .label("新建 Group")
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.new_group(window, cx)
                                    })),
                            ),
                    )
                    .child(
                        row()
                            .child(field(
                                "Group 名称",
                                Input::new(&self.group_name_editor).small(),
                            ))
                            .child(field("显示名称", Input::new(&self.group_text_name).small()))
                            .child(field("Role", Input::new(&self.group_roles).small())),
                    )
                    .child(field(
                        "描述",
                        Input::new(&self.group_text_description).small(),
                    ))
                    .child(
                        h_flex()
                            .gap(px(6.0))
                            .child(
                                ramag_ui::clickable_button("mqtt-group-save")
                                    .primary()
                                    .small()
                                    .label("保存 Group")
                                    .loading(self.saving_management)
                                    .disabled(self.saving_management || self.deleting_management)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.save_group(cx)
                                    })),
                            )
                            .child(
                                ramag_ui::clickable_button("mqtt-group-delete")
                                    .danger()
                                    .small()
                                    .label("删除 Group")
                                    .disabled(
                                        self.selected_group_name.is_none()
                                            || self.saving_management
                                            || self.deleting_management,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.confirm_delete_group(window, cx)
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_roles(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme().clone();
        let mut list = v_flex().gap(px(4.0));
        if let Some(snapshot) = &self.management_snapshot {
            for role in &snapshot.roles {
                let role_name = role.role_name.clone();
                let selected = self.selected_role_name.as_ref() == Some(&role_name);
                list = list.child(
                    h_flex()
                        .id(SharedString::from(format!("mqtt-role-{role_name}")))
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
                            this.select_role(role_name.clone(), window, cx)
                        }))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_xs().truncate().child(role.role_name.clone()))
                                .child(div().text_xs().text_color(theme.muted_foreground).child(
                                    format!(
                                        "{} 条 ACL · 通配符订阅{}",
                                        role.acls.len(),
                                        if role.allow_wildcards_subscriptions {
                                            "允许"
                                        } else {
                                            "禁止"
                                        }
                                    ),
                                )),
                        ),
                );
            }
        }
        let has_roles = self
            .management_snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.roles.is_empty());
        v_flex()
            .w_full()
            .gap(px(12.0))
            .child(section_heading("Role 列表", "ACL 来自 Dynamic Security；每行格式为 acltype|topic|allow/deny|priority", &theme))
            .child(if !has_roles {
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("暂无 Role 或尚未读取列表")
                    .into_any_element()
            } else {
                list.into_any_element()
            })
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(
                        h_flex()
                            .justify_between()
                            .child(section_heading(
                                "Role 与 ACL 编辑",
                                "支持 publishClientSend、publishClientReceive、subscribeLiteral、subscribePattern、unsubscribeLiteral、unsubscribePattern",
                                &theme,
                            ))
                            .child(
                                ramag_ui::clickable_button("mqtt-role-new")
                                    .ghost()
                                    .xsmall()
                                    .label("新建 Role")
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.new_role(window, cx)
                                    })),
                            ),
                    )
                    .child(
                        row()
                            .child(field("Role 名称", Input::new(&self.role_name_editor).small()))
                            .child(field("显示名称", Input::new(&self.role_text_name).small())),
                    )
                    .child(field(
                        "描述",
                        Input::new(&self.role_text_description).small(),
                    ))
                    .child(field(
                        "ACL",
                        Input::new(&self.role_acls).h(px(180.0)).small(),
                    ))
                    .child(toggle_button(
                        "mqtt-role-wildcards",
                        "通配符订阅",
                        self.role_allow_wildcards,
                        self.saving_management || self.deleting_management,
                        cx,
                        |this| this.role_allow_wildcards = !this.role_allow_wildcards,
                    ))
                    .child(
                        h_flex()
                            .gap(px(6.0))
                            .child(
                                ramag_ui::clickable_button("mqtt-role-save")
                                    .primary()
                                    .small()
                                    .label("保存 Role")
                                    .loading(self.saving_management)
                                    .disabled(self.saving_management || self.deleting_management)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.save_role(cx)
                                    })),
                            )
                            .child(
                                ramag_ui::clickable_button("mqtt-role-delete")
                                    .danger()
                                    .small()
                                    .label("删除 Role")
                                    .disabled(
                                        self.selected_role_name.is_none()
                                            || self.saving_management
                                            || self.deleting_management,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.confirm_delete_role(window, cx)
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

}
