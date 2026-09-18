impl MqttView {
    fn select_profile(&mut self, id: MqttProfileId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
        else {
            return;
        };
        self.selected_profile_id = Some(id);
        self.section = MqttSection::Overview;
        self.set_form_from_profile(&profile, window, cx);
        cx.notify();
    }

    fn new_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.section = MqttSection::Config;
        self.reset_form(window, cx);
        cx.notify();
    }

    fn form_profile(&self, cx: &App) -> Result<MqttProfile, String> {
        self.form_profile_with_name(cx, None)
    }

    // A connection test does not persist a profile, so it may run before the
    // user has entered the display name required for saving.
    fn form_profile_for_connection_test(&self, cx: &App) -> Result<MqttProfile, String> {
        self.form_profile_with_name(cx, Some("MQTT 连接测试"))
    }

    fn form_profile_with_name(
        &self,
        cx: &App,
        fallback_name: Option<&str>,
    ) -> Result<MqttProfile, String> {
        let entered_name = value(&self.name, cx);
        let name = if entered_name.is_empty() {
            fallback_name.unwrap_or_default().to_string()
        } else {
            entered_name
        };
        let host = value(&self.host, cx);
        let port = value(&self.port, cx)
            .parse::<u16>()
            .map_err(|_| "端口必须是 1 - 65535 的整数".to_string())?;
        let keep_alive_seconds = value(&self.keep_alive, cx)
            .parse::<u16>()
            .map_err(|_| "Keep Alive 必须是 0 - 65535 的整数".to_string())?;
        let mut profile = self
            .selected_profile()
            .cloned()
            .unwrap_or_else(|| MqttProfile::new(name.clone(), host.clone(), port));
        profile.name = name;
        profile.host = host;
        profile.port = port;
        profile.transport = self.transport;
        profile.protocol_version = self.protocol;
        profile.client_id = optional_value(&self.client_id, cx);
        profile.username = optional_value(&self.username, cx);
        if let Some(password) = optional_value(&self.password, cx) {
            profile.password = Some(password);
        }
        profile.tls = MqttTlsConfig {
            verify: profile.tls.verify,
            ca_cert_path: optional_value(&self.ca_cert_path, cx),
            client_cert_path: optional_value(&self.client_cert_path, cx),
            client_key_path: optional_value(&self.client_key_path, cx),
        };
        profile.keep_alive_seconds = keep_alive_seconds;
        profile.clean_start = self.clean_start;
        profile.subscriptions = self.subscription_topics.clone();
        profile.management.enabled = self.management_enabled;
        if !self.management_enabled {
            profile.management = Default::default();
        } else {
            if let Some(username) = optional_value(&self.management_admin_username, cx) {
                profile.management.admin_username = Some(username);
            }
            if let Some(password) = optional_value(&self.management_admin_password, cx) {
                profile.management.admin_password = Some(password);
            }
            let password_file = optional_value(&self.password_file_path, cx);
            let acl_file = optional_value(&self.acl_file_path, cx);
            profile.management.static_config = match (password_file, acl_file) {
                (None, None) => None,
                (password_file, acl_file) => Some(MosquittoStaticConfig {
                    target: profile
                        .management
                        .static_config
                        .as_ref()
                        .map(|config| config.target.clone())
                        .unwrap_or(MosquittoConfigTarget::Local),
                    password_file,
                    acl_file,
                }),
            };
        }
        profile.validate()?;
        Ok(profile)
    }

    fn save_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy() || self.subscription_running {
            return;
        }
        let profile = match self.form_profile(cx) {
            Ok(profile) => profile,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let service = self.service.clone();
        let id = profile.id.clone();
        let name = profile.name.clone();
        let profile_context_id = self.profile_context_id;
        self.operation_id = self.operation_id.wrapping_add(1);
        let operation_id = self.operation_id;
        self.saving = true;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.save_profile(&profile).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.operation_id != operation_id || this.profile_context_id != profile_context_id {
                    return;
                }
                this.saving = false;
                match result {
                    Ok(()) => {
                        if let Some(existing) = this.profiles.iter_mut().find(|item| item.id == id)
                        {
                            *existing = profile.clone();
                        } else {
                            this.profiles.push(profile.clone());
                        }
                        this.selected_profile_id = Some(id);
                        this.set_form_from_profile(&profile, window, cx);
                        this.notice = Some((format!("已保存「{name}」"), false));
                    }
                    Err(error) => {
                        this.notice = Some((format!("保存失败：{}", error.user_message()), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn delete_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy() || self.subscription_running {
            return;
        }
        let Some(id) = self.selected_profile_id.clone() else {
            self.notice = Some(("请先选择要删除的 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let service = self.service.clone();
        let profile_context_id = self.profile_context_id;
        self.operation_id = self.operation_id.wrapping_add(1);
        let operation_id = self.operation_id;
        self.deleting = true;
        self.notice = Some(("正在删除本机配置…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service.delete_profile(&id).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.operation_id != operation_id || this.profile_context_id != profile_context_id {
                    return;
                }
                this.deleting = false;
                match result {
                    Ok(()) => {
                        this.profiles.retain(|profile| profile.id != id);
                        this.reset_form(window, cx);
                        this.notice = Some(("配置已删除".into(), false));
                    }
                    Err(error) => {
                        this.notice = Some((format!("删除失败：{}", error.user_message()), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn test_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy() || self.subscription_running {
            return;
        }
        let profile = match self.form_profile_for_connection_test(cx) {
            Ok(profile) => profile,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let service = self.service.clone();
        let profile_context_id = self.profile_context_id;
        self.operation_id = self.operation_id.wrapping_add(1);
        let operation_id = self.operation_id;
        self.testing = true;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.test_connection(&profile).await;
            let _ = this.update(cx, |this, cx| {
                if this.operation_id != operation_id || this.profile_context_id != profile_context_id {
                    return;
                }
                this.testing = false;
                this.notice = Some(match result {
                    Ok(()) => ("MQTT 连接测试成功".into(), false),
                    Err(error) => (format!("连接测试失败：{}", error.user_message()), true),
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn load_snapshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let profile_context_id = self.profile_context_id;
        self.snapshot_request_id = self.snapshot_request_id.wrapping_add(1);
        let request_id = self.snapshot_request_id;
        let service = self.service.clone();
        self.loading_snapshot = true;
        self.snapshot_error = None;
        self.notice = Some(("正在读取 Broker 状态…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service.broker_snapshot(&profile).await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.snapshot_request_id != request_id || this.profile_context_id != profile_context_id {
                    return;
                }
                this.loading_snapshot = false;
                match result {
                    Ok(snapshot) => {
                        this.snapshot = Some(snapshot);
                        this.notice = Some(("Broker 状态读取完成".into(), false));
                    }
                    Err(error) => {
                        let message = error.user_message();
                        this.snapshot_error = Some(message.clone());
                        this.notice = Some((format!("读取 Broker 状态失败：{message}"), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn publish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.publishing || self.subscription_running {
            return;
        }
        // Build the request from the visible form so unsaved Broker edits take effect;
        // form_profile still starts from the selected profile to preserve saved secrets.
        let Some(profile) = self.form_profile(cx).ok() else {
            self.notice = Some(("请先填写有效的 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let payload = match encode_publish_payload(
            self.publish_payload_format,
            &value(&self.publish_payload, cx),
        ) {
            Ok(payload) => payload,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let request = MqttPublishRequest {
            topic: value(&self.publish_topic, cx),
            payload,
            qos: self.publish_qos,
            retain: self.publish_retain,
            user_properties: Vec::new(),
        };
        if let Err(error) = request.validate() {
            self.notice = Some((error, true));
            cx.notify();
            return;
        }
        let service = self.service.clone();
        let profile_context_id = self.profile_context_id;
        self.operation_id = self.operation_id.wrapping_add(1);
        let operation_id = self.operation_id;
        self.publishing = true;
        self.notice = Some(("正在发布 MQTT 消息…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service.publish(&profile, &request).await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.operation_id != operation_id || this.profile_context_id != profile_context_id {
                    return;
                }
                this.publishing = false;
                this.notice = Some(match result {
                    Ok(result) => (
                        format!("已发布到 {}（QoS {}）", result.topic, result.qos.as_u8()),
                        false,
                    ),
                    Err(error) => (format!("发布失败：{}", error.user_message()), true),
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn start_subscription(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_subscription_for_topics(self.subscription_topics.clone(), window, cx);
    }

    fn start_subscription_for_topics(
        &mut self,
        subscriptions: Vec<MqttSubscription>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.subscription_running || self.subscription_stopping {
            return;
        }
        // Use the same current-form snapshot for subscriptions as for publishing,
        // so a user can test a changed endpoint before saving the profile.
        let Some(profile) = self.form_profile(cx).ok() else {
            self.notice = Some(("请先填写有效的 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let request = MqttSubscribeRequest { subscriptions };
        if self.protocol == MqttProtocolVersion::V311
            && request
                .subscriptions
                .iter()
                .any(|subscription| subscription.no_local)
        {
            self.notice = Some((
                "MQTT 3.1.1 不支持 No Local，请切换到 MQTT 5".into(),
                true,
            ));
            cx.notify();
            return;
        }
        if let Err(error) = request.validate() {
            self.notice = Some((error, true));
            cx.notify();
            return;
        }
        self.subscription_request_id = self.subscription_request_id.wrapping_add(1);
        let request_id = self.subscription_request_id;
        let profile_context_id = self.profile_context_id;
        let active_filters = request
            .subscriptions
            .iter()
            .map(|subscription| subscription.filter.clone())
            .collect::<Vec<_>>();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.subscription_cancelled = Some(cancelled.clone());
        let (command_sender, command_receiver) =
            bounded(ramag_domain::entities::MAX_MQTT_SUBSCRIPTIONS);
        self.subscription_commands = Some(command_sender);
        self.subscription_running = true;
        self.subscription_stopping = false;
        for filter in &active_filters {
            self.set_subscription_status(filter, MqttSubscriptionState::Subscribing, None);
        }
        self.message_timeline_paused = false;
        self.messages.clear();
        self.notice = Some(("已启动订阅，等待 Broker 确认…".into(), false));
        let (sender, receiver) = bounded(32);
        let sink = Arc::new(move |message: MqttMessage| match sender.try_send(message) {
            Ok(()) => MqttMessageSinkResult::Accepted,
            Err(TrySendError::Full(_)) => MqttMessageSinkResult::Backpressured,
            Err(TrySendError::Closed(_)) => MqttMessageSinkResult::Closed,
        });
        let (status_sender, status_receiver) =
            bounded(ramag_domain::entities::MAX_MQTT_SUBSCRIPTIONS);
        let status_sink: MqttSubscriptionStatusSink = Arc::new(move |status| {
            let _ = status_sender.try_send(status);
        });
        let receiver_cancelled = cancelled.clone();
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(message) = receiver.recv().await {
                if this
                    .update_in(cx, |this, _, cx| {
                        if this.subscription_request_id != request_id
                            || this.profile_context_id != profile_context_id
                        {
                            return;
                        }
                        if this.append_received_message(message) {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    receiver_cancelled.store(true, Ordering::Release);
                    break;
                }
            }
        })
        .detach();
        let status_receiver_cancelled = cancelled.clone();
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(status) = status_receiver.recv().await {
                if this
                    .update_in(cx, |this, _, cx| {
                        if this.subscription_request_id != request_id
                            || this.profile_context_id != profile_context_id
                        {
                            return;
                        }
                        this.apply_subscription_status(status);
                        cx.notify();
                    })
                    .is_err()
                {
                    status_receiver_cancelled.store(true, Ordering::Release);
                    break;
                }
            }
        })
        .detach();
        let service = self.service.clone();
        let operation_cancelled = cancelled;
        let active_filters_for_task = active_filters.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .subscribe(
                    &profile,
                    &request,
                    sink,
                    status_sink,
                    command_receiver,
                    operation_cancelled,
                )
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.subscription_request_id != request_id
                    || this.profile_context_id != profile_context_id
                {
                    return;
                }
                this.subscription_running = false;
                this.subscription_stopping = false;
                this.message_timeline_paused = false;
                this.subscription_cancelled = None;
                this.subscription_commands = None;
                if let Err(error) = result {
                    let reason = error.user_message();
                    for filter in &active_filters_for_task {
                        this.set_subscription_status(
                            filter,
                            MqttSubscriptionState::Rejected,
                            Some(reason.clone()),
                        );
                    }
                    this.notice = Some((format!("订阅结束：{reason}"), true));
                } else {
                    for filter in &active_filters_for_task {
                        this.set_subscription_status(filter, MqttSubscriptionState::Pending, None);
                    }
                    this.notice = Some(("订阅已结束".into(), false));
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_subscription_topic(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.subscription_stopping {
            return;
        }
        let Some(subscription) = self.subscription_topics.get(index).cloned() else {
            return;
        };
        let state = self.subscription_status(&subscription.filter).state;
        match state {
            MqttSubscriptionState::Pending | MqttSubscriptionState::Rejected => {
                if !self.subscription_running {
                    self.start_subscription_for_topics(vec![subscription], window, cx);
                    return;
                }
                self.set_subscription_status(
                    &subscription.filter,
                    MqttSubscriptionState::Subscribing,
                    None,
                );
                self.send_subscription_command(
                    subscription.filter.clone(),
                    MqttSubscriptionCommand::Subscribe(subscription),
                );
            }
            MqttSubscriptionState::Subscribed => {
                if !self.subscription_running {
                    return;
                }
                let filter = subscription.filter;
                self.set_subscription_status(
                    &filter,
                    MqttSubscriptionState::Unsubscribing,
                    None,
                );
                self.send_subscription_command(
                    filter.clone(),
                    MqttSubscriptionCommand::Unsubscribe { filter },
                );
            }
            MqttSubscriptionState::Subscribing | MqttSubscriptionState::Unsubscribing => return,
        }
        cx.notify();
    }

    fn send_subscription_command(
        &mut self,
        filter: String,
        command: MqttSubscriptionCommand,
    ) {
        let result = match self.subscription_commands.as_ref() {
            Some(sender) => sender.try_send(command),
            None => Err(TrySendError::Closed(command)),
        };
        if let Err(error) = result {
            let reason = match error {
                TrySendError::Full(_) => "订阅控制队列已满".to_string(),
                TrySendError::Closed(_) => "订阅会话已结束".to_string(),
            };
            self.set_subscription_status(
                &filter,
                MqttSubscriptionState::Rejected,
                Some(reason.clone()),
            );
            self.notice = Some((format!("Topic 操作失败：{reason}"), true));
        }
    }

    fn stop_subscription(&mut self) {
        if !self.subscription_running {
            return;
        }
        self.subscription_stopping = true;
        if let Some(cancelled) = self.subscription_cancelled.as_ref() {
            cancelled.store(true, Ordering::Release);
        }
    }

}
