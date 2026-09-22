fn default_subscription_topics() -> Vec<MqttSubscription> {
    vec![MqttSubscription {
        filter: "+/#".into(),
        qos: MqttQos::AtLeastOnce,
        no_local: false,
    }]
}

fn default_subscription_statuses() -> Vec<MqttSubscriptionStatus> {
    pending_subscription_statuses(&default_subscription_topics())
}

fn pending_subscription_statuses(
    subscriptions: &[MqttSubscription],
) -> Vec<MqttSubscriptionStatus> {
    subscriptions
        .iter()
        .map(|subscription| MqttSubscriptionStatus {
            filter: subscription.filter.clone(),
            state: MqttSubscriptionState::Pending,
            reason: None,
        })
        .collect()
}

impl MqttView {
    /// Restores the default Topic list for a new, unsaved Broker configuration.
    fn reset_subscription_topics(&mut self) {
        self.subscription_topics = default_subscription_topics();
        self.reset_subscription_statuses();
        self.subscribe_no_local = false;
    }

    fn reset_subscription_statuses(&mut self) {
        self.subscription_statuses = pending_subscription_statuses(&self.subscription_topics);
    }

    fn set_subscription_status(
        &mut self,
        filter: &str,
        state: MqttSubscriptionState,
        reason: Option<String>,
    ) {
        if let Some(status) = self
            .subscription_statuses
            .iter_mut()
            .find(|status| status.filter == filter)
        {
            status.state = state;
            status.reason = reason;
        }
    }

    fn apply_subscription_status(&mut self, status: MqttSubscriptionStatus) {
        if !self
            .subscription_topics
            .iter()
            .any(|subscription| subscription.filter == status.filter)
        {
            return;
        }
        if let Some(existing) = self
            .subscription_statuses
            .iter_mut()
            .find(|existing| existing.filter == status.filter)
        {
            *existing = status;
        }
    }

    fn subscription_status(&self, filter: &str) -> MqttSubscriptionStatus {
        self.subscription_statuses
            .iter()
            .find(|status| status.filter == filter)
            .cloned()
            .unwrap_or_else(|| MqttSubscriptionStatus {
                filter: filter.to_string(),
                state: MqttSubscriptionState::Pending,
                reason: None,
            })
    }

    /// Switches protocol versions and clears options that MQTT 3.1.1 cannot encode.
    fn set_protocol_version(&mut self, protocol: MqttProtocolVersion) {
        self.protocol = protocol;
        if protocol == MqttProtocolVersion::V311 {
            let had_no_local = self
                .subscription_topics
                .iter()
                .any(|subscription| subscription.no_local)
                || self.subscribe_no_local;
            for subscription in &mut self.subscription_topics {
                subscription.no_local = false;
            }
            self.subscribe_no_local = false;
            if had_no_local {
                self.notice = Some((
                    "MQTT 3.1.1 不支持 No Local，已为当前订阅关闭该选项".into(),
                    false,
                ));
            }
        }
        self.reset_subscription_statuses();
    }

    /// Validates and appends one Topic Filter from the editor, rejecting duplicates
    /// before the list can produce an ambiguous subscribe request.
    fn add_subscription_topic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        let filter = value(&self.subscribe_filter, cx);
        if filter.is_empty() {
            self.notice = Some(("Topic Filter 不能为空".into(), true));
            cx.notify();
            return;
        }
        if self.subscription_topics.len() >= ramag_domain::entities::MAX_MQTT_SUBSCRIPTIONS {
            self.notice = Some(("订阅 Topic 数量已达到上限".into(), true));
            cx.notify();
            return;
        }
        if self
            .subscription_topics
            .iter()
            .any(|subscription| subscription.filter == filter)
        {
            self.notice = Some((format!("Topic Filter 已存在：{filter}"), true));
            cx.notify();
            return;
        }
        let subscription = MqttSubscription {
            filter,
            qos: self.subscribe_qos,
            no_local: self.subscribe_no_local,
        };
        if self.protocol == MqttProtocolVersion::V311 && subscription.no_local {
            self.notice = Some((
                "MQTT 3.1.1 不支持 No Local，请切换到 MQTT 5".into(),
                true,
            ));
            cx.notify();
            return;
        }
        if let Err(error) = subscription.validate() {
            self.notice = Some((error, true));
            cx.notify();
            return;
        }
        self.subscription_topics.push(subscription);
        self.reset_subscription_statuses();
        set_value(&self.subscribe_filter, "", window, cx);
        self.notice = Some(("已添加订阅 Topic".into(), false));
        cx.notify();
    }

    /// Removes one configured Topic Filter without touching the Broker connection.
    fn remove_subscription_topic(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.is_busy() || index >= self.subscription_topics.len() {
            return;
        }
        self.subscription_topics.remove(index);
        self.reset_subscription_statuses();
        self.notice = Some(("已移除订阅 Topic".into(), false));
        cx.notify();
    }

    /// Updates one Topic Filter's requested QoS without changing its filter text.
    fn update_subscription_qos(&mut self, index: usize, qos: MqttQos, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        let Some(subscription) = self.subscription_topics.get_mut(index) else {
            return;
        };
        subscription.qos = qos;
        self.reset_subscription_statuses();
        cx.notify();
    }

    /// Toggles No Local for one MQTT 5 subscription; MQTT 3.1.1 keeps it disabled.
    fn toggle_subscription_no_local(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.is_busy() || self.protocol == MqttProtocolVersion::V311 {
            return;
        }
        let Some(subscription) = self.subscription_topics.get_mut(index) else {
            return;
        };
        subscription.no_local = !subscription.no_local;
        self.reset_subscription_statuses();
        cx.notify();
    }
}

fn subscription_status_label(state: MqttSubscriptionState) -> &'static str {
    match state {
        MqttSubscriptionState::Pending => "未运行",
        MqttSubscriptionState::Subscribing => "订阅中",
        MqttSubscriptionState::Subscribed => "已订阅",
        MqttSubscriptionState::Unsubscribing => "取消中",
        MqttSubscriptionState::Rejected => "失败",
    }
}

/// Renders compact per-row QoS controls so each saved filter can be edited in place.
fn subscription_qos_selector(
    index: usize,
    selected: MqttQos,
    disabled: bool,
    cx: &mut Context<MqttView>,
) -> gpui_kit::Div {
    let base = format!("mqtt-subscription-{index}-qos");
    let mut controls = h_flex()
        .debug_selector({
            let base = base.clone();
            move || base.clone()
        })
        .items_center()
        .gap(px(3.0));
    for qos in [
        MqttQos::AtMostOnce,
        MqttQos::AtLeastOnce,
        MqttQos::ExactlyOnce,
    ] {
        let value = qos.as_u8();
        let selector = SharedString::from(format!("{base}-{value}"));
        let mut button = ramag_ui::clickable_button(selector.clone())
            .debug_selector({
                let selector = selector.clone();
                move || selector.to_string()
            })
            .xsmall()
            .label(value.to_string())
            .disabled(disabled)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.update_subscription_qos(index, qos, cx);
            }));
        button = if selected == qos {
            button.primary()
        } else {
            button.ghost()
        };
        controls = controls.child(button);
    }
    controls
}

/// Renders the per-row No Local toggle while keeping MQTT 3.1.1 visibly disabled.
fn subscription_no_local_toggle(
    index: usize,
    selected: bool,
    disabled: bool,
    cx: &mut Context<MqttView>,
) -> gpui_kit::component::button::Button {
    let selector = SharedString::from(format!("mqtt-subscription-{index}-no-local"));
    let mut button = ramag_ui::clickable_button(selector.clone())
        .debug_selector({
            let selector = selector.clone();
            move || selector.to_string()
        })
        .xsmall()
        .label(if selected {
            "No Local：开"
        } else {
            "No Local：关"
        })
        .disabled(disabled)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.toggle_subscription_no_local(index, cx);
        }));
    button = if selected {
        button.primary()
    } else {
        button.ghost()
    };
    button
}
