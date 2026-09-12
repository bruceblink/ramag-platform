impl MqttView {
    fn load_static_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let kind = self.static_file_kind;
        self.loading_static_file = true;
        self.static_file = None;
        self.notice = Some((format!("正在读取 Mosquitto {}…", kind.label()), false));
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.read_static_file(&profile, kind).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.loading_static_file = false;
                match result {
                    Ok(file) => {
                        set_value(&this.static_content, file.content.clone(), window, cx);
                        this.static_file = Some(file);
                        this.notice = Some(("Mosquitto 静态文件读取完成".into(), false));
                    }
                    Err(error) => {
                        this.notice = Some((
                            format!("读取 Mosquitto 静态文件失败：{}", error.user_message()),
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save_static_file(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let Some(mut file) = self.static_file.clone() else {
            self.notice = Some(("请先读取要保存的 Mosquitto 静态文件".into(), true));
            cx.notify();
            return;
        };
        file.content = value(&self.static_content, cx);
        if let Err(error) = file.validate() {
            self.notice = Some((error, true));
            cx.notify();
            return;
        }
        let service = self.service.clone();
        self.saving_static_file = true;
        self.notice = Some(("正在保存 Mosquitto 静态文件…".into(), false));
        cx.spawn(async move |this, cx| {
            let result = service.write_static_file(&profile, &file).await;
            let _ = this.update(cx, |this, cx| {
                this.saving_static_file = false;
                match result {
                    Ok(()) => {
                        this.static_file = Some(file);
                        this.notice = Some((
                            "Mosquitto 静态文件已保存；Broker 可能需要 reload 或重启才能使用新内容"
                                .into(),
                            false,
                        ));
                    }
                    Err(error) => {
                        this.notice = Some((
                            format!("保存 Mosquitto 静态文件失败：{}", error.user_message()),
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn new_client(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_client_username = None;
        self.client_disabled = false;
        for field in [
            &self.client_username,
            &self.client_id_editor,
            &self.client_password,
            &self.client_text_name,
            &self.client_text_description,
            &self.client_groups,
            &self.client_roles,
        ] {
            set_value(field, "", window, cx);
        }
        cx.notify();
    }

    fn new_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_group_name = None;
        for field in [
            &self.group_name_editor,
            &self.group_text_name,
            &self.group_text_description,
            &self.group_roles,
        ] {
            set_value(field, "", window, cx);
        }
        cx.notify();
    }

    fn new_role(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_role_name = None;
        self.role_allow_wildcards = false;
        for field in [
            &self.role_name_editor,
            &self.role_text_name,
            &self.role_text_description,
            &self.role_acls,
        ] {
            set_value(field, "", window, cx);
        }
        cx.notify();
    }

    fn select_section(&mut self, section: MqttSection, cx: &mut Context<Self>) {
        if self.section != section {
            self.section = section;
            cx.notify();
        }
    }

}
