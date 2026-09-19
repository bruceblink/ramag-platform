impl MqttView {
    fn local_server_config(&self, cx: &App) -> Result<MqttLocalServerConfig, String> {
        // 将 UI 文本转换为启动配置，所有范围和账号规则交给领域层统一校验。
        let port = value(&self.local_server_port, cx)
            .parse::<u16>()
            .map_err(|_| "本地 MQTT Broker 端口必须是 1 - 65535 的整数".to_string())?;
        let max_connections = value(&self.local_server_max_connections, cx)
            .parse::<u32>()
            .map_err(|_| "本地 MQTT Broker 连接上限必须是正整数".to_string())?;
        let config = MqttLocalServerConfig {
            bind_host: value(&self.local_server_bind_host, cx),
            port,
            max_connections,
            allow_anonymous: self.local_server_allow_anonymous,
            users: self.local_server_users.clone(),
        };
        config.validate()?;
        Ok(config)
    }

    fn local_server_running(&self) -> bool {
        self.local_server_status
            .as_ref()
            .is_some_and(|status| status.running)
    }

    fn load_local_server_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.local_server_loading || self.local_server_starting || self.local_server_stopping {
            return;
        }
        self.local_server_loading = true;
        self.local_server_notice = None;
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.local_server_status().await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.local_server_loading = false;
                match result {
                    Ok(status) => {
                        this.local_server_status = Some(status);
                        if this.local_server_running() {
                            this.start_local_server_events(window, cx);
                            this.load_local_server_snapshot(window, cx);
                        }
                    }
                    Err(error) => {
                        this.local_server_notice = Some((
                            format!("读取本地 MQTT Broker 状态失败：{}", error.user_message()),
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn start_local_server_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.local_server_running() || self.local_server_event_cancelled.is_some() {
            return;
        }
        self.local_server_event_request_id = self.local_server_event_request_id.wrapping_add(1);
        let request_id = self.local_server_event_request_id;
        let cancelled = Arc::new(AtomicBool::new(false));
        self.local_server_event_cancelled = Some(cancelled.clone());
        let (sender, receiver) = bounded(MAX_LOCAL_SERVER_EVENTS);
        let event_sink = Arc::new(move |event| match sender.try_send(event) {
            Ok(()) => MqttLocalServerEventSinkResult::Accepted,
            Err(TrySendError::Full(_)) => MqttLocalServerEventSinkResult::Backpressured,
            Err(TrySendError::Closed(_)) => MqttLocalServerEventSinkResult::Closed,
        });
        let receiver_cancelled = cancelled.clone();
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                if this
                    .update_in(cx, |this, _, cx| {
                        if this.local_server_event_request_id != request_id {
                            return;
                        }
                        if this.local_server_events.len() >= MAX_LOCAL_SERVER_EVENTS {
                            this.local_server_events.pop_front();
                        }
                        this.local_server_events.push_back(event);
                        cx.notify();
                    })
                    .is_err()
                {
                    receiver_cancelled.store(true, Ordering::Release);
                    break;
                }
            }
        })
        .detach();

        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .subscribe_local_server_events(event_sink, cancelled)
                .await;
            let _ = this.update_in(cx, |this, _window, cx| {
                if this.local_server_event_request_id != request_id {
                    return;
                }
                this.local_server_event_cancelled = None;
                if let Err(error) = result {
                    this.local_server_notice = Some((
                        format!("本地 Broker 事件流已停止：{}", error.user_message()),
                        true,
                    ));
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn clear_local_server_events(&mut self) {
        self.local_server_events.clear();
    }

    fn load_local_server_snapshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.local_server_running()
            || self.local_server_snapshot_loading
            || self.local_server_starting
            || self.local_server_stopping
            || self.local_server_publishing
        {
            return;
        }
        self.local_server_snapshot_request_id =
            self.local_server_snapshot_request_id.wrapping_add(1);
        let request_id = self.local_server_snapshot_request_id;
        self.local_server_snapshot_loading = true;
        self.local_server_snapshot_error = None;
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.local_server_snapshot().await;
            let _ = this.update_in(cx, |this, _window, cx| {
                if this.local_server_snapshot_request_id != request_id {
                    return;
                }
                this.local_server_snapshot_loading = false;
                match result {
                    Ok(snapshot) => {
                        this.local_server_snapshot = Some(snapshot);
                        this.local_server_notice = Some((
                            "本地 MQTT Broker 快照读取完成".into(),
                            false,
                        ));
                    }
                    Err(error) => {
                        let message = error.user_message();
                        this.local_server_snapshot_error = Some(message.clone());
                        this.local_server_notice = Some((
                            format!("读取本地 MQTT Broker 客户端失败：{message}"),
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn start_local_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.local_server_loading || self.local_server_starting || self.local_server_stopping {
            return;
        }
        if self.local_server_running() {
            return;
        }
        let config = match self.local_server_config(cx) {
            Ok(config) => config,
            Err(error) => {
                self.local_server_notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        self.local_server_starting = true;
        self.local_server_notice = None;
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.start_local_server(&config).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.local_server_starting = false;
                match result {
                    Ok(status) => {
                        let endpoint = local_server_endpoint(&status);
                        this.local_server_status = Some(status);
                        this.start_local_server_events(window, cx);
                        this.load_local_server_snapshot(window, cx);
                        this.local_server_notice = Some((
                            format!("本地 MQTT Broker 已启动：{endpoint}"),
                            false,
                        ));
                    }
                    Err(error) => {
                        this.local_server_notice = Some((
                            format!("启动本地 MQTT Broker 失败：{}", error.user_message()),
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn stop_local_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.local_server_loading || self.local_server_starting || self.local_server_stopping {
            return;
        }
        if !self.local_server_running() {
            return;
        }
        self.local_server_stopping = true;
        self.local_server_event_request_id = self.local_server_event_request_id.wrapping_add(1);
        if let Some(cancelled) = self.local_server_event_cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
        self.local_server_events.clear();
        self.local_server_notice = None;
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.stop_local_server().await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.local_server_stopping = false;
                match result {
                    Ok(status) => {
                        this.local_server_status = Some(status);
                        this.local_server_snapshot = None;
                        this.local_server_snapshot_error = None;
                        this.local_server_notice = Some((
                            "本地 MQTT Broker 已停止".to_string(),
                            false,
                        ));
                    }
                    Err(error) => {
                        this.local_server_notice = Some((
                            format!("停止本地 MQTT Broker 失败：{}", error.user_message()),
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn publish_local_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.local_server_running()
            || self.local_server_publishing
            || self.local_server_starting
            || self.local_server_stopping
        {
            return;
        }
        let payload_text = self.local_server_publish_payload.read(cx).value().to_string();
        let payload = match encode_publish_payload(
            self.local_server_publish_payload_format,
            &payload_text,
        ) {
            Ok(payload) => payload,
            Err(error) => {
                self.local_server_notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let request = MqttPublishRequest {
            topic: value(&self.local_server_publish_topic, cx),
            payload,
            qos: self.local_server_publish_qos,
            retain: self.local_server_publish_retain,
            user_properties: Vec::new(),
        };
        if let Err(error) = request.validate() {
            self.local_server_notice = Some((error, true));
            cx.notify();
            return;
        }
        self.local_server_publish_id = self.local_server_publish_id.wrapping_add(1);
        let publish_id = self.local_server_publish_id;
        self.local_server_publishing = true;
        self.local_server_notice = Some(("正在向本地 MQTT Broker 发布消息…".into(), false));
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.publish_local_server(&request).await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.local_server_publish_id != publish_id {
                    return;
                }
                this.local_server_publishing = false;
                this.local_server_notice = Some(match result {
                    Ok(result) => (
                        format!(
                            "已注入发布到 {}（QoS {}{}）",
                            result.topic,
                            result.qos.as_u8(),
                            if request.retain { "，Retain" } else { "" }
                        ),
                        false,
                    ),
                    Err(error) => (
                        format!("本地 Broker 发布失败：{}", error.user_message()),
                        true,
                    ),
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_local_server_anonymous(&mut self) {
        self.local_server_allow_anonymous = !self.local_server_allow_anonymous;
        self.local_server_notice = None;
    }

    fn add_local_server_user(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.local_server_running() || self.local_server_starting || self.local_server_stopping {
            return;
        }
        let user = MqttLocalServerUser {
            username: value(&self.local_server_username, cx),
            password: value(&self.local_server_password, cx),
        };
        let mut users = self.local_server_users.clone();
        users.push(user.clone());
        let candidate = MqttLocalServerConfig {
            users,
            ..MqttLocalServerConfig::default()
        };
        if let Err(error) = candidate.validate() {
            self.local_server_notice = Some((error, true));
            cx.notify();
            return;
        }
        self.local_server_users.push(user);
        set_value(&self.local_server_username, "", window, cx);
        set_value(&self.local_server_password, "", window, cx);
        self.local_server_notice = Some((
            "账号已加入本地 Broker 配置；重启服务后生效".to_string(),
            false,
        ));
        cx.notify();
    }

    fn remove_local_server_user(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.local_server_running() || self.local_server_starting || self.local_server_stopping {
            return;
        }
        if index >= self.local_server_users.len() {
            return;
        }
        let user = self.local_server_users.remove(index);
        self.local_server_notice = Some((
            format!("已移除账号 {}；重启服务后生效", user.username),
            false,
        ));
        cx.notify();
    }

    fn use_local_server_for_client(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let host = value(&self.local_server_bind_host, cx);
        let port = value(&self.local_server_port, cx);
        if port.parse::<u16>().is_err() {
            self.local_server_notice = Some((
                "本地 MQTT Broker 端口必须是 1 - 65535 的整数".to_string(),
                true,
            ));
            cx.notify();
            return;
        }
        set_value(&self.host, host, window, cx);
        set_value(&self.port, port, window, cx);
        if let Some(user) = self.local_server_users.first() {
            set_value(&self.username, user.username.clone(), window, cx);
            set_value(&self.password, user.password.clone(), window, cx);
        }
        self.section = MqttSection::Config;
        self.notice = Some((
            "本地 Broker 地址已填入客户端配置；保存后即可发布或订阅".to_string(),
            false,
        ));
        cx.notify();
    }
}

fn local_server_endpoint(status: &MqttLocalServerStatus) -> String {
    if status.bind_host.contains(':') {
        format!("[{}]:{}", status.bind_host, status.port)
    } else {
        format!("{}:{}", status.bind_host, status.port)
    }
}
