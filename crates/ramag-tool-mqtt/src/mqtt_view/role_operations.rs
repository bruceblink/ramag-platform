impl MqttView {
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
        let profile_context_id = self.profile_context_id;
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
                if this.management_operation_id != operation_id
                    || this.profile_context_id != profile_context_id
                {
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
