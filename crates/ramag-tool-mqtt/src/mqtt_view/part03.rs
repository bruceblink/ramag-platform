impl MqttView {
    fn load_management(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择已启用 Mosquitto 管理的配置".into(), true));
            cx.notify();
            return;
        };
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.loading_management = true;
        self.management_error = None;
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.dynamic_security_snapshot(&profile).await;
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.loading_management = false;
                match result {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.notice = Some(("Mosquitto Dynamic Security 读取完成".into(), false));
                    }
                    Err(error) => {
                        let message = error.user_message();
                        this.management_error = Some(message.clone());
                        this.notice = Some((format!("Mosquitto 管理读取失败：{message}"), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn select_management_section(
        &mut self,
        section: MosquittoManagementSection,
        cx: &mut Context<Self>,
    ) {
        self.management_section = section;
        cx.notify();
    }

    fn select_client(&mut self, username: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(client) = self
            .management_snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .clients
                    .iter()
                    .find(|client| client.username == username)
            })
            .cloned()
        else {
            return;
        };
        self.management_section = MosquittoManagementSection::Clients;
        self.selected_client_username = Some(client.username.clone());
        set_value(&self.client_username, client.username, window, cx);
        set_value(
            &self.client_id_editor,
            client.client_id.unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.client_password, "", window, cx);
        set_value(
            &self.client_text_name,
            client.text_name.unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.client_text_description,
            client.text_description.unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.client_groups,
            join_group_bindings(&client.groups),
            window,
            cx,
        );
        set_value(
            &self.client_roles,
            join_role_bindings(&client.roles),
            window,
            cx,
        );
        self.client_disabled = client.disabled;
        cx.notify();
    }

    fn select_group(&mut self, group_name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(group) = self
            .management_snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .groups
                    .iter()
                    .find(|group| group.group_name == group_name)
            })
            .cloned()
        else {
            return;
        };
        self.management_section = MosquittoManagementSection::Groups;
        self.selected_group_name = Some(group.group_name.clone());
        set_value(&self.group_name_editor, group.group_name, window, cx);
        set_value(
            &self.group_text_name,
            group.text_name.unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.group_text_description,
            group.text_description.unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.group_roles,
            join_role_bindings(&group.roles),
            window,
            cx,
        );
        cx.notify();
    }

    fn select_role(&mut self, role_name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(role) = self
            .management_snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .roles
                    .iter()
                    .find(|role| role.role_name == role_name)
            })
            .cloned()
        else {
            return;
        };
        self.management_section = MosquittoManagementSection::Roles;
        self.selected_role_name = Some(role.role_name.clone());
        set_value(&self.role_name_editor, role.role_name, window, cx);
        set_value(
            &self.role_text_name,
            role.text_name.unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.role_text_description,
            role.text_description.unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.role_acls, serialize_acls(&role.acls), window, cx);
        self.role_allow_wildcards = role.allow_wildcards_subscriptions;
        cx.notify();
    }

    fn client_from_editor(&self, cx: &App) -> std::result::Result<MosquittoClient, String> {
        let client = MosquittoClient {
            username: value(&self.client_username, cx),
            client_id: optional_value(&self.client_id_editor, cx),
            password_configured: self
                .selected_client_username
                .as_ref()
                .and_then(|username| {
                    self.management_snapshot.as_ref().and_then(|snapshot| {
                        snapshot
                            .clients
                            .iter()
                            .find(|client| &client.username == username)
                            .map(|client| client.password_configured)
                    })
                })
                .unwrap_or(false),
            password: optional_value(&self.client_password, cx),
            disabled: self.client_disabled,
            text_name: optional_value(&self.client_text_name, cx),
            text_description: optional_value(&self.client_text_description, cx),
            groups: parse_group_bindings(&value(&self.client_groups, cx))?,
            roles: parse_role_bindings(&value(&self.client_roles, cx))?,
        };
        client.validate()?;
        Ok(client)
    }

    fn group_from_editor(&self, cx: &App) -> std::result::Result<MosquittoGroup, String> {
        let group = MosquittoGroup {
            group_name: value(&self.group_name_editor, cx),
            text_name: optional_value(&self.group_text_name, cx),
            text_description: optional_value(&self.group_text_description, cx),
            roles: parse_role_bindings(&value(&self.group_roles, cx))?,
        };
        group.validate()?;
        Ok(group)
    }

    fn role_from_editor(&self, cx: &App) -> std::result::Result<MosquittoRole, String> {
        let role = MosquittoRole {
            role_name: value(&self.role_name_editor, cx),
            text_name: optional_value(&self.role_text_name, cx),
            text_description: optional_value(&self.role_text_description, cx),
            allow_wildcards_subscriptions: self.role_allow_wildcards,
            acls: parse_acls(&value(&self.role_acls, cx))?,
        };
        role.validate()?;
        Ok(role)
    }

    fn save_client(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let client = match self.client_from_editor(cx) {
            Ok(client) => client,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let service = self.service.clone();
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.saving_management = true;
        self.notice = Some(("正在保存 Mosquitto 用户…".into(), false));
        cx.spawn(async move |this, cx| {
            let outcome = match service.save_client(&profile, &client).await {
                Err(error) => Err((false, error)),
                Ok(()) => match service.dynamic_security_snapshot(&profile).await {
                    Ok(snapshot) => Ok(snapshot),
                    Err(error) => Err((true, error)),
                },
            };
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.saving_management = false;
                match outcome {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.notice = Some(("Mosquitto 用户已保存".into(), false));
                    }
                    Err((saved, error)) => {
                        let message = error.user_message().to_string();
                        this.management_error = Some(message.clone());
                        this.notice = Some((
                            if saved {
                                format!("用户已保存，但刷新列表失败：{message}")
                            } else {
                                format!("保存用户失败：{message}")
                            },
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save_group(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let group = match self.group_from_editor(cx) {
            Ok(group) => group,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let service = self.service.clone();
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.saving_management = true;
        self.notice = Some(("正在保存 Mosquitto Group…".into(), false));
        cx.spawn(async move |this, cx| {
            let outcome = match service.save_group(&profile, &group).await {
                Err(error) => Err((false, error)),
                Ok(()) => match service.dynamic_security_snapshot(&profile).await {
                    Ok(snapshot) => Ok(snapshot),
                    Err(error) => Err((true, error)),
                },
            };
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.saving_management = false;
                match outcome {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.notice = Some(("Mosquitto Group 已保存".into(), false));
                    }
                    Err((saved, error)) => {
                        let message = error.user_message().to_string();
                        this.notice = Some((
                            if saved {
                                format!("Group 已保存，但刷新列表失败：{message}")
                            } else {
                                format!("保存 Group 失败：{message}")
                            },
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save_role(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let role = match self.role_from_editor(cx) {
            Ok(role) => role,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let service = self.service.clone();
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.saving_management = true;
        self.notice = Some(("正在保存 Mosquitto Role…".into(), false));
        cx.spawn(async move |this, cx| {
            let outcome = match service.save_role(&profile, &role).await {
                Err(error) => Err((false, error)),
                Ok(()) => match service.dynamic_security_snapshot(&profile).await {
                    Ok(snapshot) => Ok(snapshot),
                    Err(error) => Err((true, error)),
                },
            };
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.saving_management = false;
                match outcome {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.notice = Some(("Mosquitto Role 已保存".into(), false));
                    }
                    Err((saved, error)) => {
                        let message = error.user_message().to_string();
                        this.notice = Some((
                            if saved {
                                format!("Role 已保存，但刷新列表失败：{message}")
                            } else {
                                format!("保存 Role 失败：{message}")
                            },
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn confirm_delete_client(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(username) = self.selected_client_username.clone() else {
            self.notice = Some(("请先选择要删除的用户".into(), true));
            cx.notify();
            return;
        };
        let entity = cx.entity().clone();
        ramag_ui::open_confirm(
            "删除 Mosquitto 用户",
            format!("将从 Dynamic Security 删除用户「{username}」，此操作不可撤销。"),
            "删除用户",
            true,
            move |_, app| {
                entity.update(app, |this, cx| this.delete_client(username, cx));
            },
            window,
            cx,
        );
    }

    fn delete_client(&mut self, username: String, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            return;
        };
        let service = self.service.clone();
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.deleting_management = true;
        self.notice = Some(("正在删除 Mosquitto 用户…".into(), false));
        cx.spawn(async move |this, cx| {
            let outcome = match service.delete_client(&profile, &username).await {
                Err(error) => Err((false, error)),
                Ok(()) => match service.dynamic_security_snapshot(&profile).await {
                    Ok(snapshot) => Ok(snapshot),
                    Err(error) => Err((true, error)),
                },
            };
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.deleting_management = false;
                match outcome {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.selected_client_username = None;
                        this.notice = Some(("Mosquitto 用户已删除".into(), false));
                    }
                    Err((saved, error)) => {
                        let message = error.user_message().to_string();
                        this.notice = Some((
                            if saved {
                                format!("用户已删除，但刷新列表失败：{message}")
                            } else {
                                format!("删除用户失败：{message}")
                            },
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn confirm_delete_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(group_name) = self.selected_group_name.clone() else {
            self.notice = Some(("请先选择要删除的 Group".into(), true));
            cx.notify();
            return;
        };
        let entity = cx.entity().clone();
        ramag_ui::open_confirm(
            "删除 Mosquitto Group",
            format!("将从 Dynamic Security 删除 Group「{group_name}」，此操作不可撤销。"),
            "删除 Group",
            true,
            move |_, app| {
                entity.update(app, |this, cx| this.delete_group(group_name, cx));
            },
            window,
            cx,
        );
    }

    fn delete_group(&mut self, group_name: String, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            return;
        };
        let service = self.service.clone();
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.deleting_management = true;
        self.notice = Some(("正在删除 Mosquitto Group…".into(), false));
        cx.spawn(async move |this, cx| {
            let outcome = match service.delete_group(&profile, &group_name).await {
                Err(error) => Err((false, error)),
                Ok(()) => match service.dynamic_security_snapshot(&profile).await {
                    Ok(snapshot) => Ok(snapshot),
                    Err(error) => Err((true, error)),
                },
            };
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.deleting_management = false;
                match outcome {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.selected_group_name = None;
                        this.notice = Some(("Mosquitto Group 已删除".into(), false));
                    }
                    Err((saved, error)) => {
                        let message = error.user_message().to_string();
                        this.notice = Some((
                            if saved {
                                format!("Group 已删除，但刷新列表失败：{message}")
                            } else {
                                format!("删除 Group 失败：{message}")
                            },
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn confirm_delete_role(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(role_name) = self.selected_role_name.clone() else {
            self.notice = Some(("请先选择要删除的 Role".into(), true));
            cx.notify();
            return;
        };
        let entity = cx.entity().clone();
        ramag_ui::open_confirm(
            "删除 Mosquitto Role",
            format!("将从 Dynamic Security 删除 Role「{role_name}」及其 ACL，此操作不可撤销。"),
            "删除 Role",
            true,
            move |_, app| {
                entity.update(app, |this, cx| this.delete_role(role_name, cx));
            },
            window,
            cx,
        );
    }

    fn delete_role(&mut self, role_name: String, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            return;
        };
        let service = self.service.clone();
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.deleting_management = true;
        self.notice = Some(("正在删除 Mosquitto Role…".into(), false));
        cx.spawn(async move |this, cx| {
            let outcome = match service.delete_role(&profile, &role_name).await {
                Err(error) => Err((false, error)),
                Ok(()) => match service.dynamic_security_snapshot(&profile).await {
                    Ok(snapshot) => Ok(snapshot),
                    Err(error) => Err((true, error)),
                },
            };
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.deleting_management = false;
                match outcome {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.selected_role_name = None;
                        this.notice = Some(("Mosquitto Role 已删除".into(), false));
                    }
                    Err((saved, error)) => {
                        let message = error.user_message().to_string();
                        this.notice = Some((
                            if saved {
                                format!("Role 已删除，但刷新列表失败：{message}")
                            } else {
                                format!("删除 Role 失败：{message}")
                            },
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

}
