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

    fn client_id(profile: &MqttProfile) -> String {
        profile
            .client_id
            .clone()
            .unwrap_or_else(|| format!("ramag-{}", profile.id))
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
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
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
        run_subscription_v311(client, eventloop, filters, sink, cancelled).await
    }

    async fn subscribe_v5(
        profile: &MqttProfile,
        request: &MqttSubscribeRequest,
        sink: MqttMessageSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        let (client, eventloop) = create_v5_client(profile)?;
        let filters = request
            .subscriptions
            .iter()
            .map(|subscription| {
                rumqttc::v5::mqttbytes::v5::Filter::new(
                    subscription.filter.clone(),
                    qos_v5(subscription.qos),
                )
            })
            .collect::<Vec<_>>();
        run_subscription_v5(client, eventloop, filters, sink, cancelled).await
    }

    async fn run_subscription_v311(
        client: AsyncClient,
        mut eventloop: EventLoop,
        filters: Vec<rumqttc::SubscribeFilter>,
        sink: MqttMessageSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        async move {
            client
                .subscribe_many(filters)
                .await
                .map_err(|error| mqtt_client_error("订阅 MQTT 3.1.1 Topic", error.to_string()))?;
            loop {
                if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                    return Ok(());
                }
                match eventloop
                    .poll()
                    .await
                    .map_err(|error| mqtt_connection_error(error.to_string()))?
                {
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
        .await
    }

    async fn run_subscription_v5(
        client: rumqttc::v5::AsyncClient,
        mut eventloop: rumqttc::v5::EventLoop,
        filters: Vec<rumqttc::v5::mqttbytes::v5::Filter>,
        sink: MqttMessageSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        async move {
            client
                .subscribe_many(filters)
                .await
                .map_err(|error| mqtt_client_error("订阅 MQTT 5 Topic", error.to_string()))?;
            loop {
                if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                    return Ok(());
                }
                match eventloop
                    .poll()
                    .await
                    .map_err(|error| mqtt_connection_error(error.to_string()))?
                {
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
        .await
    }
