//! MQTT 基础设施层。
//!
//! `native` 特性启用 `rumqttc`，默认构建仍保留无 native 依赖的能力探测和明确的
//! 不支持结果。适配器在独立 Tokio 运行时中驱动 EventLoop，再把领域消息交给有界 sink。

use std::sync::{Arc, atomic::AtomicBool};
#[cfg(feature = "native")]
use std::time::Duration;

use async_trait::async_trait;
use ramag_domain::entities::{
    MqttBrokerSnapshot, MqttMessageSink, MqttProfile, MqttPublishRequest, MqttPublishResult,
    MqttSubscribeRequest, MqttTransportBackend, MqttTransportCapabilities,
};
use ramag_domain::error::{DomainError, MqttError, MqttErrorCategory, Result};
use ramag_domain::traits::{MqttDriver, MqttTransport};

#[cfg(feature = "native")]
use native::{publish_native, subscribe_native, test_connection_native};

#[cfg(feature = "native")]
const EVENT_LOOP_CAPACITY: usize = 32;

#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMqttTransport;

impl NativeMqttTransport {
    pub const fn new() -> Self {
        Self
    }
}

impl MqttTransport for NativeMqttTransport {
    fn capabilities(&self) -> MqttTransportCapabilities {
        self.transport_capabilities()
    }
}

impl NativeMqttTransport {
    pub const fn transport_capabilities(&self) -> MqttTransportCapabilities {
        MqttTransportCapabilities {
            backend: MqttTransportBackend::Native,
            build_available: cfg!(feature = "native"),
            mqtt311: cfg!(feature = "native"),
            mqtt5: cfg!(feature = "native"),
            tcp: cfg!(feature = "native"),
            tls: cfg!(feature = "native"),
            subscribe: cfg!(feature = "native"),
            publish: cfg!(feature = "native"),
            dynamic_security: false,
            static_config: false,
            metrics: false,
            retained_topics: false,
            online_clients: false,
        }
    }
}

#[async_trait]
impl MqttDriver for NativeMqttTransport {
    fn transport_capabilities(&self) -> MqttTransportCapabilities {
        Self::transport_capabilities(self)
    }

    async fn test_connection(&self, profile: &MqttProfile) -> Result<()> {
        #[cfg(feature = "native")]
        {
            let profile = profile.clone();
            return run_native(move || test_connection_native(profile)).await;
        }
        #[cfg(not(feature = "native"))]
        {
            let _ = profile;
            Err(native_unavailable("测试 MQTT 连接"))
        }
    }

    async fn publish(
        &self,
        profile: &MqttProfile,
        request: &MqttPublishRequest,
    ) -> Result<MqttPublishResult> {
        #[cfg(feature = "native")]
        {
            let profile = profile.clone();
            let request = request.clone();
            return run_native(move || publish_native(profile, request)).await;
        }
        #[cfg(not(feature = "native"))]
        {
            let _ = (profile, request);
            Err(native_unavailable("发布 MQTT 消息"))
        }
    }

    async fn subscribe(
        &self,
        profile: &MqttProfile,
        request: &MqttSubscribeRequest,
        sink: MqttMessageSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        #[cfg(feature = "native")]
        {
            let profile = profile.clone();
            let request = request.clone();
            return run_native(move || subscribe_native(profile, request, sink, cancelled)).await;
        }
        #[cfg(not(feature = "native"))]
        {
            let _ = (profile, request, sink, cancelled);
            Err(native_unavailable("订阅 MQTT 消息"))
        }
    }

    async fn broker_snapshot(&self, _profile: &MqttProfile) -> Result<MqttBrokerSnapshot> {
        Err(DomainError::Mqtt(MqttError::new(
            MqttErrorCategory::Unsupported,
            "读取 MQTT Broker 快照",
            "当前 native MQTT 适配器不能从标准 MQTT 数据面读取完整的主题目录或在线客户端列表",
        )))
    }
}

#[cfg(not(feature = "native"))]
fn native_unavailable(operation: &'static str) -> DomainError {
    DomainError::Mqtt(MqttError::new(
        MqttErrorCategory::Unsupported,
        operation,
        "当前构建未启用 MQTT native 客户端；请启用 native feature",
    ))
}

#[cfg(feature = "native")]
async fn run_native<F, Fut, T>(operation: F) -> Result<T>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    smol::unblock(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                DomainError::Mqtt(MqttError::new(
                    MqttErrorCategory::Unknown,
                    "初始化 MQTT 运行时",
                    format!("无法初始化 MQTT 运行时：{error}"),
                ))
            })?;
        runtime
            .block_on(tokio::time::timeout(Duration::from_secs(15), operation()))
            .map_err(|_| {
                DomainError::Mqtt(MqttError::new(
                    MqttErrorCategory::Timeout,
                    "执行 MQTT 操作",
                    "MQTT 操作超时",
                ))
            })?
    })
    .await
}

#[cfg(feature = "native")]
mod native {
    use super::*;
    use chrono::Utc;
    use ramag_domain::entities::{
        MqttMessage, MqttProtocolVersion, MqttQos, MqttTlsConfig, MqttTransport, MqttUserProperty,
        TlsVerify,
    };
    use rumqttc::{AsyncClient, Event, EventLoop, Incoming, MqttOptions, Outgoing, QoS, Transport};
    use std::fs;
    use std::time::Duration;

    const MAX_TLS_FILE_BYTES: u64 = 4 * 1024 * 1024;

    type TlsMaterial = (Option<Vec<u8>>, Option<(Vec<u8>, Vec<u8>)>);

    pub(super) async fn test_connection_native(profile: MqttProfile) -> Result<()> {
        match profile.protocol_version {
            MqttProtocolVersion::V311 => {
                let (_, eventloop) = create_v311_client(&profile)?;
                wait_v311_connection(eventloop).await
            }
            MqttProtocolVersion::V5 => {
                let (_, eventloop) = create_v5_client(&profile)?;
                wait_v5_connection(eventloop).await
            }
        }
    }

    pub(super) async fn publish_native(
        profile: MqttProfile,
        request: MqttPublishRequest,
    ) -> Result<MqttPublishResult> {
        match profile.protocol_version {
            MqttProtocolVersion::V311 => publish_v311(&profile, &request).await,
            MqttProtocolVersion::V5 => publish_v5(&profile, &request).await,
        }
    }

    pub(super) async fn subscribe_native(
        profile: MqttProfile,
        request: MqttSubscribeRequest,
        sink: MqttMessageSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        match profile.protocol_version {
            MqttProtocolVersion::V311 => subscribe_v311(&profile, &request, sink, cancelled).await,
            MqttProtocolVersion::V5 => subscribe_v5(&profile, &request, sink, cancelled).await,
        }
    }

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

    fn qos_v311(qos: MqttQos) -> QoS {
        match qos {
            MqttQos::AtMostOnce => QoS::AtMostOnce,
            MqttQos::AtLeastOnce => QoS::AtLeastOnce,
            MqttQos::ExactlyOnce => QoS::ExactlyOnce,
        }
    }

    fn qos_v5(qos: MqttQos) -> rumqttc::v5::mqttbytes::QoS {
        match qos {
            MqttQos::AtMostOnce => rumqttc::v5::mqttbytes::QoS::AtMostOnce,
            MqttQos::AtLeastOnce => rumqttc::v5::mqttbytes::QoS::AtLeastOnce,
            MqttQos::ExactlyOnce => rumqttc::v5::mqttbytes::QoS::ExactlyOnce,
        }
    }

    fn qos_from_v311(qos: QoS) -> MqttQos {
        match qos {
            QoS::AtMostOnce => MqttQos::AtMostOnce,
            QoS::AtLeastOnce => MqttQos::AtLeastOnce,
            QoS::ExactlyOnce => MqttQos::ExactlyOnce,
        }
    }

    fn qos_from_v5(qos: rumqttc::v5::mqttbytes::QoS) -> MqttQos {
        match qos {
            rumqttc::v5::mqttbytes::QoS::AtMostOnce => MqttQos::AtMostOnce,
            rumqttc::v5::mqttbytes::QoS::AtLeastOnce => MqttQos::AtLeastOnce,
            rumqttc::v5::mqttbytes::QoS::ExactlyOnce => MqttQos::ExactlyOnce,
        }
    }

    fn nonzero_packet_id(packet_id: u16) -> Option<u16> {
        (packet_id != 0).then_some(packet_id)
    }

    fn user_properties(request: &MqttPublishRequest) -> Vec<(String, String)> {
        request
            .user_properties
            .iter()
            .map(|property| (property.name.clone(), property.value.clone()))
            .collect()
    }

    fn build_v311_transport(tls: &MqttTlsConfig) -> Result<Transport> {
        let (ca, client_auth) = load_tls_material(tls)?;
        if tls.verify == TlsVerify::None {
            return Err(DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Unsupported,
                "创建 MQTT TLS 连接",
                "MQTT native 适配器不允许关闭服务器证书校验",
            )));
        }
        if tls.verify == TlsVerify::Ca {
            return Err(DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Unsupported,
                "创建 MQTT TLS 连接",
                "当前 MQTT native 适配器暂不支持仅校验证书链而跳过主机名校验",
            )));
        }
        Ok(match (ca, client_auth) {
            (None, None) => Transport::tls_with_default_config(),
            (Some(ca), auth) => Transport::tls(ca, auth, None),
            (None, Some(_)) => {
                return Err(DomainError::InvalidConfig(
                    "配置客户端证书时必须同时配置 CA 证书".into(),
                ));
            }
        })
    }

    fn build_v5_transport(tls: &MqttTlsConfig) -> Result<Transport> {
        build_v311_transport(tls)
    }

    fn load_tls_material(tls: &MqttTlsConfig) -> Result<TlsMaterial> {
        let ca = read_tls_file(tls.ca_cert_path.as_deref())?;
        let client_cert = read_tls_file(tls.client_cert_path.as_deref())?;
        let client_key = read_tls_file(tls.client_key_path.as_deref())?;
        let client_auth = match (client_cert, client_key) {
            (None, None) => None,
            (Some(cert), Some(key)) => Some((cert, key)),
            _ => {
                return Err(DomainError::InvalidConfig(
                    "客户端证书和客户端密钥必须同时配置".into(),
                ));
            }
        };
        Ok((ca, client_auth))
    }

    fn read_tls_file(path: Option<&str>) -> Result<Option<Vec<u8>>> {
        let Some(path) = path else {
            return Ok(None);
        };
        let metadata = fs::metadata(path).map_err(|_| {
            DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Tls,
                "读取 MQTT TLS 文件",
                "无法读取 MQTT TLS 文件",
            ))
        })?;
        if metadata.len() > MAX_TLS_FILE_BYTES {
            return Err(DomainError::InvalidConfig(
                "MQTT TLS 文件超过 4 MiB 上限".into(),
            ));
        }
        let contents = fs::read(path).map_err(|_| {
            DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Tls,
                "读取 MQTT TLS 文件",
                "无法读取 MQTT TLS 文件",
            ))
        })?;
        Ok(Some(contents))
    }

    fn mqtt_connection_error(message: String) -> DomainError {
        DomainError::Mqtt(MqttError::new(
            MqttErrorCategory::Network,
            "连接 MQTT Broker",
            format!("连接 MQTT Broker 失败：{message}"),
        ))
    }

    fn mqtt_client_error(operation: &'static str, message: String) -> DomainError {
        DomainError::Mqtt(MqttError::new(
            MqttErrorCategory::Protocol,
            operation,
            format!("{operation}失败：{message}"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_build_exposes_native_capability_without_claiming_runtime_support() {
        let capabilities = NativeMqttTransport::new().transport_capabilities();
        assert_eq!(capabilities.backend, MqttTransportBackend::Native);
        assert_eq!(capabilities.build_available, cfg!(feature = "native"));
    }
}
