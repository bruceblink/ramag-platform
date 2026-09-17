fn default_subscription_topics() -> Vec<MqttSubscription> {
    vec![MqttSubscription {
        filter: "+/#".into(),
        qos: MqttQos::AtLeastOnce,
        no_local: false,
    }]
}

impl MqttView {
    /// Replaces the editable Topic list when a new Broker configuration is selected.
    /// The reference tool starts with a catch-all filter, while the list remains
    /// local to the current form until the profile persistence slice is added.
    fn reset_subscription_topics(&mut self) {
        self.subscription_topics = default_subscription_topics();
        self.subscribe_no_local = false;
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
        cx.notify();
    }
}

/// Renders compact per-row QoS controls so each saved filter can be edited in place.
fn subscription_qos_selector(
    index: usize,
    selected: MqttQos,
    disabled: bool,
    cx: &mut Context<MqttView>,
) -> gpui::Div {
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
) -> gpui_component::button::Button {
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
