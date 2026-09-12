impl MqttView {
    pub fn new(service: Arc<MqttService>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = input(
            window,
            cx,
            MAX_PROFILE_NAME_BYTES,
            "配置名称（保存时必填）",
            false,
            "",
        );
        let host = input(
            window,
            cx,
            MAX_HOST_BYTES,
            "Broker 地址，例如 127.0.0.1",
            false,
            "",
        );
        let port = input(window, cx, 5, "1883", false, "1883");
        let client_id = input(
            window,
            cx,
            MAX_CLIENT_ID_BYTES,
            "Client ID（可选）",
            false,
            "",
        );
        let username = input(window, cx, MAX_USERNAME_BYTES, "用户名（可选）", false, "");
        let password = input(
            window,
            cx,
            MAX_PASSWORD_BYTES,
            "密码（留空保持已保存密码）",
            true,
            "",
        );
        let management_admin_username = input(
            window,
            cx,
            MAX_USERNAME_BYTES,
            "管理用户名（可选）",
            false,
            "",
        );
        let management_admin_password =
            input(window, cx, MAX_PASSWORD_BYTES, "管理密码（可选）", true, "");
        let password_file_path = input(
            window,
            cx,
            MAX_PATH_BYTES,
            "password_file 绝对路径",
            false,
            "",
        );
        let acl_file_path = input(window, cx, MAX_PATH_BYTES, "acl_file 绝对路径", false, "");
        let ca_cert_path = input(window, cx, MAX_PATH_BYTES, "CA 证书路径（可选）", false, "");
        let client_cert_path = input(
            window,
            cx,
            MAX_PATH_BYTES,
            "客户端证书路径（可选）",
            false,
            "",
        );
        let client_key_path = input(
            window,
            cx,
            MAX_PATH_BYTES,
            "客户端密钥路径（可选）",
            false,
            "",
        );
        let keep_alive = input(window, cx, 5, "Keep Alive 秒数", false, "60");
        let publish_topic = input(window, cx, MAX_TOPIC_BYTES, "发布 Topic", false, "");
        let publish_payload = input(window, cx, 16 * 1024 * 1024, "消息内容", false, "");
        let subscribe_filter = input(
            window,
            cx,
            MAX_TOPIC_BYTES,
            "Topic Filter，例如 sensors/#",
            false,
            "",
        );
        let search = input(window, cx, MAX_PROFILE_NAME_BYTES, "筛选配置…", false, "");
        let client_username = input(window, cx, MAX_USERNAME_BYTES, "用户名", false, "");
        let client_id_editor = input(
            window,
            cx,
            MAX_CLIENT_ID_BYTES,
            "Client ID（可选）",
            false,
            "",
        );
        let client_password = input(
            window,
            cx,
            MAX_PASSWORD_BYTES,
            "新密码（留空不修改）",
            true,
            "",
        );
        let client_text_name = input(window, cx, MAX_PROFILE_NAME_BYTES, "显示名称", false, "");
        let client_text_description = input(
            window,
            cx,
            MAX_MOSQUITTO_DESCRIPTION_BYTES,
            "描述",
            false,
            "",
        );
        let client_groups = input(
            window,
            cx,
            MAX_MOSQUITTO_BINDINGS_BYTES,
            "Group 名称，逗号分隔",
            false,
            "",
        );
        let client_roles = input(
            window,
            cx,
            MAX_MOSQUITTO_BINDINGS_BYTES,
            "Role 名称，逗号分隔",
            false,
            "",
        );
        let group_name_editor = input(window, cx, MAX_PROFILE_NAME_BYTES, "Group 名称", false, "");
        let group_text_name = input(window, cx, MAX_PROFILE_NAME_BYTES, "显示名称", false, "");
        let group_text_description = input(
            window,
            cx,
            MAX_MOSQUITTO_DESCRIPTION_BYTES,
            "描述",
            false,
            "",
        );
        let group_roles = input(
            window,
            cx,
            MAX_MOSQUITTO_BINDINGS_BYTES,
            "Role 名称，逗号分隔",
            false,
            "",
        );
        let role_name_editor = input(window, cx, MAX_PROFILE_NAME_BYTES, "Role 名称", false, "");
        let role_text_name = input(window, cx, MAX_PROFILE_NAME_BYTES, "显示名称", false, "");
        let role_text_description = input(
            window,
            cx,
            MAX_MOSQUITTO_DESCRIPTION_BYTES,
            "描述",
            false,
            "",
        );
        let role_acls = input(
            window,
            cx,
            MAX_MOSQUITTO_ACL_EDITOR_BYTES,
            "ACL 编辑器",
            false,
            "",
        );
        let static_content = input(
            window,
            cx,
            MAX_MOSQUITTO_STATIC_FILE_BYTES,
            "静态文件内容",
            false,
            "",
        );

        let fields = [
            &name,
            &host,
            &port,
            &client_id,
            &username,
            &password,
            &management_admin_username,
            &management_admin_password,
            &password_file_path,
            &acl_file_path,
            &ca_cert_path,
            &client_cert_path,
            &client_key_path,
            &keep_alive,
            &publish_topic,
            &publish_payload,
            &subscribe_filter,
            &search,
            &client_username,
            &client_id_editor,
            &client_password,
            &client_text_name,
            &client_text_description,
            &client_groups,
            &client_roles,
            &group_name_editor,
            &group_text_name,
            &group_text_description,
            &group_roles,
            &role_name_editor,
            &role_text_name,
            &role_text_description,
            &role_acls,
            &static_content,
        ];
        let mut subscriptions = Vec::with_capacity(fields.len());
        for field in fields {
            subscriptions.push(cx.subscribe(field, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.notice = None;
                    cx.notify();
                }
            }));
        }

        let mut view = Self {
            service,
            profiles: Vec::new(),
            selected_profile_id: None,
            section: MqttSection::Config,
            sidebar_visible: false,
            name,
            host,
            port,
            client_id,
            username,
            password,
            management_admin_username,
            management_admin_password,
            password_file_path,
            acl_file_path,
            ca_cert_path,
            client_cert_path,
            client_key_path,
            keep_alive,
            publish_topic,
            publish_payload,
            subscribe_filter,
            search,
            client_username,
            client_id_editor,
            client_password,
            client_text_name,
            client_text_description,
            client_groups,
            client_roles,
            client_disabled: false,
            group_name_editor,
            group_text_name,
            group_text_description,
            group_roles,
            role_name_editor,
            role_text_name,
            role_text_description,
            role_acls,
            role_allow_wildcards: false,
            static_content,
            transport: TransportKind::Tcp,
            protocol: MqttProtocolVersion::V5,
            clean_start: true,
            management_enabled: false,
            loading_profiles: true,
            saving: false,
            testing: false,
            deleting: false,
            publishing: false,
            loading_snapshot: false,
            loading_management: false,
            saving_management: false,
            deleting_management: false,
            loading_static_file: false,
            saving_static_file: false,
            snapshot: None,
            management_snapshot: None,
            management_section: MosquittoManagementSection::Clients,
            selected_client_username: None,
            selected_group_name: None,
            selected_role_name: None,
            static_file_kind: MosquittoStaticFileKind::Password,
            static_file: None,
            messages: VecDeque::new(),
            subscription_running: false,
            subscription_cancelled: None,
            profile_request_id: 0,
            operation_id: 0,
            snapshot_request_id: 0,
            subscription_request_id: 0,
            notice: None,
            snapshot_error: None,
            management_error: None,
            management_operation_id: 0,
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        view.load_profiles(window, cx);
        view
    }

    fn selected_profile(&self) -> Option<&MqttProfile> {
        self.selected_profile_id
            .as_ref()
            .and_then(|id| self.profiles.iter().find(|profile| &profile.id == id))
    }

    fn is_busy(&self) -> bool {
        self.saving || self.testing || self.deleting || self.publishing || self.loading_profiles
    }

    /// Detect the width where the profile list would leave the active MQTT workbench unusable.
    fn sidebar_is_narrow(window: &Window) -> bool {
        window.viewport_size().width < px(MQTT_SIDEBAR_COLLAPSE_BREAKPOINT)
    }

    fn load_profiles(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.profile_request_id = self.profile_request_id.wrapping_add(1);
        let request_id = self.profile_request_id;
        self.loading_profiles = true;
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.list_profiles().await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.profile_request_id != request_id {
                    return;
                }
                this.loading_profiles = false;
                match result {
                    Ok(profiles) => {
                        let selected = this
                            .selected_profile_id
                            .clone()
                            .filter(|id| profiles.iter().any(|profile| &profile.id == id));
                        this.profiles = profiles;
                        if let Some(id) =
                            selected.or_else(|| this.profiles.first().map(|p| p.id.clone()))
                        {
                            if let Some(profile) =
                                this.profiles.iter().find(|p| p.id == id).cloned()
                            {
                                this.selected_profile_id = Some(id);
                                this.set_form_from_profile(&profile, window, cx);
                            }
                        } else {
                            this.reset_form(window, cx);
                        }
                    }
                    Err(error) => {
                        this.notice = Some((
                            format!("加载 MQTT 配置失败：{}", error.user_message()),
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn reset_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_subscription();
        self.selected_profile_id = None;
        self.transport = TransportKind::Tcp;
        self.protocol = MqttProtocolVersion::V5;
        self.clean_start = true;
        self.management_enabled = false;
        for field in [
            &self.name,
            &self.host,
            &self.client_id,
            &self.username,
            &self.password,
            &self.management_admin_username,
            &self.management_admin_password,
            &self.password_file_path,
            &self.acl_file_path,
            &self.ca_cert_path,
            &self.client_cert_path,
            &self.client_key_path,
            &self.publish_topic,
            &self.publish_payload,
            &self.subscribe_filter,
        ] {
            set_value(field, "", window, cx);
        }
        set_value(&self.port, "1883", window, cx);
        set_value(&self.keep_alive, "60", window, cx);
        for field in [
            &self.client_username,
            &self.client_id_editor,
            &self.client_password,
            &self.client_text_name,
            &self.client_text_description,
            &self.client_groups,
            &self.client_roles,
            &self.group_name_editor,
            &self.group_text_name,
            &self.group_text_description,
            &self.group_roles,
            &self.role_name_editor,
            &self.role_text_name,
            &self.role_text_description,
            &self.role_acls,
            &self.static_content,
        ] {
            set_value(field, "", window, cx);
        }
        self.management_section = MosquittoManagementSection::Clients;
        self.selected_client_username = None;
        self.selected_group_name = None;
        self.selected_role_name = None;
        self.client_disabled = false;
        self.role_allow_wildcards = false;
        self.static_file_kind = MosquittoStaticFileKind::Password;
        self.static_file = None;
        self.password.update(cx, |state, cx| {
            state.set_placeholder("密码（可选）", window, cx);
        });
        self.clear_runtime_state();
        self.client_disabled = false;
        self.role_allow_wildcards = false;
        self.notice = None;
    }

    fn set_form_from_profile(
        &mut self,
        profile: &MqttProfile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_subscription();
        self.transport = profile.transport;
        self.protocol = profile.protocol_version;
        self.clean_start = profile.clean_start;
        self.management_enabled = profile.management.enabled;
        set_value(&self.name, profile.name.clone(), window, cx);
        set_value(&self.host, profile.host.clone(), window, cx);
        set_value(&self.port, profile.port.to_string(), window, cx);
        set_value(
            &self.client_id,
            profile.client_id.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.username,
            profile.username.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.password, "", window, cx);
        set_value(
            &self.management_admin_username,
            profile
                .management
                .admin_username
                .clone()
                .unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.management_admin_password, "", window, cx);
        set_value(
            &self.password_file_path,
            profile
                .management
                .static_config
                .as_ref()
                .and_then(|config| config.password_file.clone())
                .unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.acl_file_path,
            profile
                .management
                .static_config
                .as_ref()
                .and_then(|config| config.acl_file.clone())
                .unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.ca_cert_path,
            profile.tls.ca_cert_path.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.client_cert_path,
            profile.tls.client_cert_path.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.client_key_path,
            profile.tls.client_key_path.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.keep_alive,
            profile.keep_alive_seconds.to_string(),
            window,
            cx,
        );
        self.password.update(cx, |state, cx| {
            state.set_placeholder(
                if profile.password.is_some() {
                    "已保存密码，留空保持；输入新值可替换"
                } else {
                    "密码（可选）"
                },
                window,
                cx,
            );
        });
        self.publish_topic.update(cx, |state, cx| {
            state.set_placeholder("发布 Topic", window, cx);
        });
        self.clear_runtime_state();
        self.notice = None;
    }

    fn clear_runtime_state(&mut self) {
        self.snapshot = None;
        self.management_snapshot = None;
        self.snapshot_error = None;
        self.management_error = None;
        self.selected_client_username = None;
        self.selected_group_name = None;
        self.selected_role_name = None;
        self.static_file = None;
        self.messages.clear();
    }

}
