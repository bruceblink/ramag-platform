use std::collections::VecDeque;

    fn create_v311_client(profile: &MqttProfile) -> Result<(AsyncClient, EventLoop)> {
        let mut options = MqttOptions::new(client_id(profile), &profile.host, profile.port);
        options
            .set_keep_alive(Duration::from_secs(u64::from(profile.keep_alive_seconds)))
            .set_clean_session(profile.clean_start)
            .set_max_packet_size(
                ramag_domain::entities::MAX_MQTT_PUBLISH_PAYLOAD_BYTES,
                ramag_domain::entities::MAX_MQTT_PUBLISH_PAYLOAD_BYTES,
            );
        set_v311_credentials(&mut options, profile);
        if profile.transport == MqttTransport::Tls {
            options.set_transport(build_v311_transport(&profile.tls)?);
        }
        Ok(AsyncClient::new(options, EVENT_LOOP_CAPACITY))
    }
    fn create_v5_client(
        profile: &MqttProfile,
    ) -> Result<(rumqttc::v5::AsyncClient, rumqttc::v5::EventLoop)> {
        let mut options =
            rumqttc::v5::MqttOptions::new(client_id(profile), &profile.host, profile.port);
        options
            .set_keep_alive(Duration::from_secs(u64::from(profile.keep_alive_seconds)))
            .set_clean_start(profile.clean_start)
            .set_session_expiry_interval(profile.session_expiry_seconds)
            .set_max_packet_size(Some(
                ramag_domain::entities::MAX_MQTT_PUBLISH_PAYLOAD_BYTES as u32,
            ));
        set_v5_credentials(&mut options, profile);
        if profile.transport == MqttTransport::Tls {
            options.set_transport(build_v5_transport(&profile.tls)?);
        }
        Ok(rumqttc::v5::AsyncClient::new(options, EVENT_LOOP_CAPACITY))
    }

    /// Uses the saved Client ID when present; otherwise creates a distinct
    /// ephemeral ID so concurrent Ramag operations do not disconnect each other.
    fn client_id(profile: &MqttProfile) -> String {
        profile.client_id.clone().unwrap_or_else(|| {
            let suffix = NEXT_EPHEMERAL_MQTT_CLIENT_ID.fetch_add(1, Ordering::Relaxed);
            format!("ramag-{}-{suffix}", profile.id)
        })
    }

    fn set_v311_credentials(options: &mut MqttOptions, profile: &MqttProfile) {
        if let (Some(username), Some(password)) = (&profile.username, &profile.password) {
            options.set_credentials(username, password);
        }
    }

    fn set_v5_credentials(options: &mut rumqttc::v5::MqttOptions, profile: &MqttProfile) {
        if let (Some(username), Some(password)) = (&profile.username, &profile.password) {
            options.set_credentials(username, password);
        }
    }

    async fn wait_v311_connection(mut eventloop: EventLoop) -> Result<()> {
        async move {
            loop {
                match eventloop
                    .poll()
                    .await
                    .map_err(|error| mqtt_connection_error(error.to_string()))?
                {
                    Event::Incoming(Incoming::ConnAck(_)) => return Ok(()),
                    Event::Incoming(_) | Event::Outgoing(_) => {}
                }
            }
        }
        .await
    }

    async fn wait_v5_connection(mut eventloop: rumqttc::v5::EventLoop) -> Result<()> {
        async move {
            loop {
                match eventloop
                    .poll()
                    .await
                    .map_err(|error| mqtt_connection_error(error.to_string()))?
                {
                    rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::ConnAck(_)) => {
                        return Ok(());
                    }
                    rumqttc::v5::Event::Incoming(_) | rumqttc::v5::Event::Outgoing(_) => {}
                }
            }
        }
        .await
    }

    async fn publish_v311(
        profile: &MqttProfile,
        request: &MqttPublishRequest,
    ) -> Result<MqttPublishResult> {
        let (client, mut eventloop) = create_v311_client(profile)?;
        let qos = qos_v311(request.qos);
        async move {
            client
                .publish(&request.topic, qos, request.retain, request.payload.clone())
                .await
                .map_err(|error| mqtt_client_error("发布 MQTT 3.1.1 消息", error.to_string()))?;
            let mut connected = false;
            loop {
                match eventloop
                    .poll()
                    .await
                    .map_err(|error| mqtt_connection_error(error.to_string()))?
                {
                    Event::Incoming(Incoming::ConnAck(_)) => connected = true,
                    Event::Outgoing(Outgoing::Publish(packet_id))
                        if connected && qos == QoS::AtMostOnce =>
                    {
                        return Ok(MqttPublishResult {
                            topic: request.topic.clone(),
                            packet_id: nonzero_packet_id(packet_id),
                            qos: request.qos,
                        });
                    }
                    Event::Incoming(Incoming::PubAck(ack))
                        if connected && qos == QoS::AtLeastOnce =>
                    {
                        return Ok(MqttPublishResult {
                            topic: request.topic.clone(),
                            packet_id: Some(ack.pkid),
                            qos: request.qos,
                        });
                    }
                    Event::Incoming(Incoming::PubComp(ack))
                        if connected && qos == QoS::ExactlyOnce =>
                    {
                        return Ok(MqttPublishResult {
                            topic: request.topic.clone(),
                            packet_id: Some(ack.pkid),
                            qos: request.qos,
                        });
                    }
                    Event::Incoming(_) | Event::Outgoing(_) => {}
                }
            }
        }
        .await
    }

    async fn publish_v5(
        profile: &MqttProfile,
        request: &MqttPublishRequest,
    ) -> Result<MqttPublishResult> {
        let (client, mut eventloop) = create_v5_client(profile)?;
        let qos = qos_v5(request.qos);
        let properties = if request.user_properties.is_empty() {
            None
        } else {
            Some(rumqttc::v5::mqttbytes::v5::PublishProperties {
                user_properties: user_properties(request),
                ..Default::default()
            })
        };
        async move {
            match properties {
                Some(properties) => {
                    client
                        .publish_with_properties(
                            &request.topic,
                            qos,
                            request.retain,
                            bytes::Bytes::from(request.payload.clone()),
                            properties,
                        )
                        .await
                }
                None => {
                    client
                        .publish(
                            &request.topic,
                            qos,
                            request.retain,
                            bytes::Bytes::from(request.payload.clone()),
                        )
                        .await
                }
            }
            .map_err(|error| mqtt_client_error("发布 MQTT 5 消息", error.to_string()))?;
            let mut connected = false;
            loop {
                match eventloop
                    .poll()
                    .await
                    .map_err(|error| mqtt_connection_error(error.to_string()))?
                {
                    rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::ConnAck(_)) => {
                        connected = true
                    }
                    rumqttc::v5::Event::Outgoing(Outgoing::Publish(packet_id))
                        if connected && qos == rumqttc::v5::mqttbytes::QoS::AtMostOnce =>
                    {
                        return Ok(MqttPublishResult {
                            topic: request.topic.clone(),
                            packet_id: nonzero_packet_id(packet_id),
                            qos: request.qos,
                        });
                    }
                    rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::PubAck(ack))
                        if connected && qos == rumqttc::v5::mqttbytes::QoS::AtLeastOnce =>
                    {
                        return Ok(MqttPublishResult {
                            topic: request.topic.clone(),
                            packet_id: Some(ack.pkid),
                            qos: request.qos,
                        });
                    }
                    rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::PubComp(ack))
                        if connected && qos == rumqttc::v5::mqttbytes::QoS::ExactlyOnce =>
                    {
                        return Ok(MqttPublishResult {
                            topic: request.topic.clone(),
                            packet_id: Some(ack.pkid),
                            qos: request.qos,
                        });
                    }
                    rumqttc::v5::Event::Incoming(_) | rumqttc::v5::Event::Outgoing(_) => {}
                }
            }
        }
        .await
    }

    async fn subscribe_v311(
        profile: &MqttProfile,
        request: &MqttSubscribeRequest,
        sink: MqttMessageSink,
        status_sink: MqttSubscriptionStatusSink,
        commands: MqttSubscriptionCommandReceiver,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        if request
            .subscriptions
            .iter()
            .any(|subscription| subscription.no_local)
        {
            return Err(DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Unsupported,
                "订阅 MQTT 3.1.1 Topic",
                "MQTT 3.1.1 不支持 No Local 订阅选项，请切换到 MQTT 5",
            )));
        }
        let (client, eventloop) = create_v311_client(profile)?;
        let filters = request
            .subscriptions
            .iter()
            .map(|subscription| {
                rumqttc::SubscribeFilter::new(
                    subscription.filter.clone(),
                    qos_v311(subscription.qos),
                )
            })
            .collect::<Vec<_>>();
        let filter_names = request
            .subscriptions
            .iter()
            .map(|subscription| subscription.filter.clone())
            .collect::<Vec<_>>();
        run_subscription_v311(
            client,
            eventloop,
            filters,
            filter_names,
            sink,
            status_sink,
            commands,
            cancelled,
        )
        .await
    }

    async fn subscribe_v5(
        profile: &MqttProfile,
        request: &MqttSubscribeRequest,
        sink: MqttMessageSink,
        status_sink: MqttSubscriptionStatusSink,
        commands: MqttSubscriptionCommandReceiver,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        let (client, eventloop) = create_v5_client(profile)?;
        let filters = request
            .subscriptions
            .iter()
            .map(|subscription| {
                let mut filter = rumqttc::v5::mqttbytes::v5::Filter::new(
                    subscription.filter.clone(),
                    qos_v5(subscription.qos),
                );
                filter.nolocal = subscription.no_local;
                filter
            })
            .collect::<Vec<_>>();
        let filter_names = request
            .subscriptions
            .iter()
            .map(|subscription| subscription.filter.clone())
            .collect::<Vec<_>>();
        run_subscription_v5(
            client,
            eventloop,
            filters,
            filter_names,
            sink,
            status_sink,
            commands,
            cancelled,
        )
        .await
    }

    async fn run_subscription_v311(
        client: AsyncClient,
        mut eventloop: EventLoop,
        filters: Vec<rumqttc::SubscribeFilter>,
        filter_names: Vec<String>,
        sink: MqttMessageSink,
        status_sink: MqttSubscriptionStatusSink,
        commands: MqttSubscriptionCommandReceiver,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        let cancellation_client = client.clone();
        let cancellation_signal = cancelled.clone();
        let cancellation_task = tokio::spawn(async move {
            while !cancellation_signal.load(std::sync::atomic::Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            let _ = cancellation_client.disconnect().await;
        });
        let result = async {
            if let Err(error) = client.subscribe_many(filters).await {
                emit_rejected_subscription_statuses(
                    &status_sink,
                    &filter_names,
                    error.to_string(),
                );
                return Err(mqtt_client_error("订阅 MQTT 3.1.1 Topic", error.to_string()));
            }
            let mut pending_subscribes = VecDeque::from([filter_names]);
            let mut pending_unsubscribes = VecDeque::new();
            let mut commands_closed = false;
            loop {
                if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                    return Ok(());
                }
                let event = if commands_closed {
                    eventloop.poll().await
                } else {
                    tokio::select! {
                        command = commands.recv() => {
                            match command {
                                Ok(command) => handle_subscription_command_v311(
                                    &client,
                                    command,
                                    &mut pending_subscribes,
                                    &mut pending_unsubscribes,
                                    &status_sink,
                                ),
                                Err(_) => commands_closed = true,
                            }
                            continue;
                        }
                        event = eventloop.poll() => event,
                    }
                };
                let event = match event {
                    Ok(event) => event,
                    Err(_error) if cancelled.load(std::sync::atomic::Ordering::Acquire) => {
                        return Ok(())
                    }
                    Err(error) => return Err(mqtt_connection_error(error.to_string())),
                };
                match event {
                    Event::Incoming(Incoming::SubAck(ack)) => {
                        let filters = pending_subscribes.pop_front().unwrap_or_default();
                        for (filter, code) in filters.iter().zip(ack.return_codes.iter()) {
                            let (state, reason) = match code {
                                rumqttc::mqttbytes::v4::SubscribeReasonCode::Success(_) => {
                                    (MqttSubscriptionState::Subscribed, None)
                                }
                                rumqttc::mqttbytes::v4::SubscribeReasonCode::Failure => (
                                    MqttSubscriptionState::Rejected,
                                    Some("Broker 拒绝订阅".into()),
                                ),
                            };
                            emit_subscription_status(&status_sink, filter, state, reason);
                        }
                    }
                    Event::Incoming(Incoming::UnsubAck(_ack)) => {
                        if let Some(filter) = pending_unsubscribes.pop_front() {
                            emit_subscription_status(
                                &status_sink,
                                &filter,
                                MqttSubscriptionState::Pending,
                                None,
                            );
                        }
                    }
                    Event::Incoming(Incoming::Publish(publish)) => {
                        let message = MqttMessage {
                            topic: publish.topic,
                            payload: publish.payload.to_vec(),
                            qos: qos_from_v311(publish.qos),
                            retain: publish.retain,
                            duplicate: publish.dup,
                            received_at: Utc::now(),
                            user_properties: vec![],
                        };
                        message.validate().map_err(DomainError::InvalidConfig)?;
                        if matches!(
                            sink(message),
                            ramag_domain::entities::MqttMessageSinkResult::Closed
                        ) {
                            return Ok(());
                        }
                    }
                    Event::Incoming(_) | Event::Outgoing(_) => {}
                }
            }
        }
        .await;
        cancellation_task.abort();
        result
    }

    fn handle_subscription_command_v311(
        client: &AsyncClient,
        command: MqttSubscriptionCommand,
        pending_subscribes: &mut VecDeque<Vec<String>>,
        pending_unsubscribes: &mut VecDeque<String>,
        status_sink: &MqttSubscriptionStatusSink,
    ) {
        match command {
            MqttSubscriptionCommand::Subscribe(subscription) => {
                let filter = subscription.filter.clone();
                if subscription.no_local {
                    emit_subscription_status(
                        status_sink,
                        &filter,
                        MqttSubscriptionState::Rejected,
                        Some("MQTT 3.1.1 不支持 No Local 订阅选项".into()),
                    );
                    return;
                }
                let request = rumqttc::SubscribeFilter::new(filter.clone(), qos_v311(subscription.qos));
                match client.try_subscribe_many(std::iter::once(request)) {
                    Ok(()) => pending_subscribes.push_back(vec![filter]),
                    Err(error) => emit_subscription_status(
                        status_sink,
                        &filter,
                        MqttSubscriptionState::Rejected,
                        Some(format!("发送订阅请求失败：{error}")),
                    ),
                }
            }
            MqttSubscriptionCommand::Unsubscribe { filter } => {
                match client.try_unsubscribe(filter.clone()) {
                    Ok(()) => pending_unsubscribes.push_back(filter),
                    Err(error) => emit_subscription_status(
                        status_sink,
                        &filter,
                        MqttSubscriptionState::Rejected,
                        Some(format!("发送取消订阅请求失败：{error}")),
                    ),
                }
            }
        }
    }

    async fn run_subscription_v5(
        client: rumqttc::v5::AsyncClient,
        mut eventloop: rumqttc::v5::EventLoop,
        filters: Vec<rumqttc::v5::mqttbytes::v5::Filter>,
        filter_names: Vec<String>,
        sink: MqttMessageSink,
        status_sink: MqttSubscriptionStatusSink,
        commands: MqttSubscriptionCommandReceiver,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        let cancellation_client = client.clone();
        let cancellation_signal = cancelled.clone();
        let cancellation_task = tokio::spawn(async move {
            while !cancellation_signal.load(std::sync::atomic::Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            let _ = cancellation_client.disconnect().await;
        });
        let result = async {
            if let Err(error) = client.subscribe_many(filters).await {
                emit_rejected_subscription_statuses(
                    &status_sink,
                    &filter_names,
                    error.to_string(),
                );
                return Err(mqtt_client_error("订阅 MQTT 5 Topic", error.to_string()));
            }
            let mut pending_subscribes = VecDeque::from([filter_names]);
            let mut pending_unsubscribes = VecDeque::new();
            let mut commands_closed = false;
            loop {
                if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                    return Ok(());
                }
                let event = if commands_closed {
                    eventloop.poll().await
                } else {
                    tokio::select! {
                        command = commands.recv() => {
                            match command {
                                Ok(command) => handle_subscription_command_v5(
                                    &client,
                                    command,
                                    &mut pending_subscribes,
                                    &mut pending_unsubscribes,
                                    &status_sink,
                                ),
                                Err(_) => commands_closed = true,
                            }
                            continue;
                        }
                        event = eventloop.poll() => event,
                    }
                };
                let event = match event {
                    Ok(event) => event,
                    Err(_error) if cancelled.load(std::sync::atomic::Ordering::Acquire) => {
                        return Ok(())
                    }
                    Err(error) => return Err(mqtt_connection_error(error.to_string())),
                };
                match event {
                    rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::SubAck(ack)) => {
                        let filters = pending_subscribes.pop_front().unwrap_or_default();
                        for (filter, code) in filters.iter().zip(ack.return_codes.iter()) {
                            let (state, reason) = match code {
                                rumqttc::v5::mqttbytes::v5::SubscribeReasonCode::Success(_) => {
                                    (MqttSubscriptionState::Subscribed, None)
                                }
                                other => (
                                    MqttSubscriptionState::Rejected,
                                    Some(format!("Broker 拒绝订阅：{other:?}")),
                                ),
                            };
                            emit_subscription_status(&status_sink, filter, state, reason);
                        }
                    }
                    rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::UnsubAck(ack)) => {
                        if let Some(filter) = pending_unsubscribes.pop_front() {
                            let (state, reason) = match ack.reasons.first() {
                                Some(
                                    rumqttc::v5::mqttbytes::v5::UnsubAckReason::Success
                                    | rumqttc::v5::mqttbytes::v5::UnsubAckReason::NoSubscriptionExisted,
                                )
                                | None => (MqttSubscriptionState::Pending, None),
                                Some(other) => (
                                    MqttSubscriptionState::Rejected,
                                    Some(format!("Broker 拒绝取消订阅：{other:?}")),
                                ),
                            };
                            emit_subscription_status(&status_sink, &filter, state, reason);
                        }
                    }
                    rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::Publish(publish)) => {
                        let topic = String::from_utf8(publish.topic.to_vec()).map_err(|_| {
                            DomainError::Mqtt(MqttError::new(
                                MqttErrorCategory::Protocol,
                                "接收 MQTT 5 消息",
                                "Broker 返回了无效的 Topic 编码",
                            ))
                        })?;
                        let user_properties =
                            publish
                                .properties
                                .as_ref()
                                .map_or_else(Vec::new, |properties| {
                                    properties
                                        .user_properties
                                        .iter()
                                        .map(|(name, value)| MqttUserProperty {
                                            name: name.clone(),
                                            value: value.clone(),
                                        })
                                        .collect()
                                });
                        let message = MqttMessage {
                            topic,
                            payload: publish.payload.to_vec(),
                            qos: qos_from_v5(publish.qos),
                            retain: publish.retain,
                            duplicate: publish.dup,
                            received_at: Utc::now(),
                            user_properties,
                        };
                        message.validate().map_err(DomainError::InvalidConfig)?;
                        if matches!(
                            sink(message),
                            ramag_domain::entities::MqttMessageSinkResult::Closed
                        ) {
                            return Ok(());
                        }
                    }
                    rumqttc::v5::Event::Incoming(_) | rumqttc::v5::Event::Outgoing(_) => {}
                }
            }
        }
        .await;
        cancellation_task.abort();
        result
    }

    fn handle_subscription_command_v5(
        client: &rumqttc::v5::AsyncClient,
        command: MqttSubscriptionCommand,
        pending_subscribes: &mut VecDeque<Vec<String>>,
        pending_unsubscribes: &mut VecDeque<String>,
        status_sink: &MqttSubscriptionStatusSink,
    ) {
        match command {
            MqttSubscriptionCommand::Subscribe(subscription) => {
                let filter_name = subscription.filter.clone();
                let mut filter = rumqttc::v5::mqttbytes::v5::Filter::new(
                    filter_name.clone(),
                    qos_v5(subscription.qos),
                );
                filter.nolocal = subscription.no_local;
                match client.try_subscribe_many(std::iter::once(filter)) {
                    Ok(()) => pending_subscribes.push_back(vec![filter_name]),
                    Err(error) => emit_subscription_status(
                        status_sink,
                        &filter_name,
                        MqttSubscriptionState::Rejected,
                        Some(format!("发送订阅请求失败：{error}")),
                    ),
                }
            }
            MqttSubscriptionCommand::Unsubscribe { filter } => {
                match client.try_unsubscribe(filter.clone()) {
                    Ok(()) => pending_unsubscribes.push_back(filter),
                    Err(error) => emit_subscription_status(
                        status_sink,
                        &filter,
                        MqttSubscriptionState::Rejected,
                        Some(format!("发送取消订阅请求失败：{error}")),
                    ),
                }
            }
        }
    }

    fn emit_rejected_subscription_statuses(
        status_sink: &MqttSubscriptionStatusSink,
        filters: &[String],
        reason: String,
    ) {
        for filter in filters {
            emit_subscription_status(
                status_sink,
                filter,
                MqttSubscriptionState::Rejected,
                Some(reason.clone()),
            );
        }
    }

    fn emit_subscription_status(
        status_sink: &MqttSubscriptionStatusSink,
        filter: &str,
        state: MqttSubscriptionState,
        reason: Option<String>,
    ) {
        status_sink(MqttSubscriptionStatus {
            filter: filter.to_string(),
            state,
            reason,
        });
    }
