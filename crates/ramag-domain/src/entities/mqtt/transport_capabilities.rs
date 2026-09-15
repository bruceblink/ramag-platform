
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MqttTransportBackend {
    Native,
    TestDouble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttTransportCapabilities {
    pub backend: MqttTransportBackend,
    pub build_available: bool,
    pub mqtt311: bool,
    pub mqtt5: bool,
    pub tcp: bool,
    pub tls: bool,
    pub subscribe: bool,
    pub publish: bool,
    pub dynamic_security: bool,
    pub static_config: bool,
    pub metrics: bool,
    pub retained_topics: bool,
    pub online_clients: bool,
}

impl MqttTransportCapabilities {
    pub const fn unknown() -> Self {
        Self {
            backend: MqttTransportBackend::TestDouble,
            build_available: false,
            mqtt311: false,
            mqtt5: false,
            tcp: false,
            tls: false,
            subscribe: false,
            publish: false,
            dynamic_security: false,
            static_config: false,
            metrics: false,
            retained_topics: false,
            online_clients: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MqttTransportCapability {
    Mqtt311,
    Mqtt5,
    Tcp,
    Tls,
    Subscribe,
    Publish,
    DynamicSecurity,
    StaticConfig,
    Metrics,
    RetainedTopics,
    OnlineClients,
}

impl MqttTransportCapabilities {
    pub const fn supports(self, capability: MqttTransportCapability) -> bool {
        match capability {
            MqttTransportCapability::Mqtt311 => self.mqtt311,
            MqttTransportCapability::Mqtt5 => self.mqtt5,
            MqttTransportCapability::Tcp => self.tcp,
            MqttTransportCapability::Tls => self.tls,
            MqttTransportCapability::Subscribe => self.subscribe,
            MqttTransportCapability::Publish => self.publish,
            MqttTransportCapability::DynamicSecurity => self.dynamic_security,
            MqttTransportCapability::StaticConfig => self.static_config,
            MqttTransportCapability::Metrics => self.metrics,
            MqttTransportCapability::RetainedTopics => self.retained_topics,
            MqttTransportCapability::OnlineClients => self.online_clients,
        }
    }
}
