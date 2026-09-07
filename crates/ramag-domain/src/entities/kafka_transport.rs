use serde::{Deserialize, Serialize};

/// Kafka 传输实现的来源；产品层不依赖具体客户端类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KafkaTransportBackend {
    NativeRdkafka,
    PureRust,
    TestDouble,
}

/// 传输层在当前构建和配置下能提供的协议能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaTransportCapabilities {
    pub backend: KafkaTransportBackend,
    pub build_available: bool,
    pub metadata: bool,
    pub fetch: bool,
    pub list_offsets: bool,
    pub consumer_groups: bool,
    pub topic_admin: bool,
    pub config_admin: bool,
    pub acl_admin: bool,
    pub tls: bool,
    pub sasl: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KafkaTransportCapability {
    Metadata,
    Fetch,
    ListOffsets,
    ConsumerGroups,
    TopicAdmin,
    ConfigAdmin,
    AclAdmin,
    Tls,
    Sasl,
}

impl KafkaTransportCapabilities {
    pub const fn native(build_available: bool, tls: bool, sasl: bool) -> Self {
        Self {
            backend: KafkaTransportBackend::NativeRdkafka,
            build_available,
            metadata: build_available,
            fetch: build_available,
            list_offsets: build_available,
            consumer_groups: build_available,
            topic_admin: build_available,
            config_admin: build_available,
            acl_admin: build_available,
            tls: build_available && tls,
            sasl: build_available && sasl,
        }
    }

    pub const fn unknown() -> Self {
        Self {
            backend: KafkaTransportBackend::TestDouble,
            build_available: false,
            metadata: false,
            fetch: false,
            list_offsets: false,
            consumer_groups: false,
            topic_admin: false,
            config_admin: false,
            acl_admin: false,
            tls: false,
            sasl: false,
        }
    }

    pub const fn supports(self, capability: KafkaTransportCapability) -> bool {
        match capability {
            KafkaTransportCapability::Metadata => self.metadata,
            KafkaTransportCapability::Fetch => self.fetch,
            KafkaTransportCapability::ListOffsets => self.list_offsets,
            KafkaTransportCapability::ConsumerGroups => self.consumer_groups,
            KafkaTransportCapability::TopicAdmin => self.topic_admin,
            KafkaTransportCapability::ConfigAdmin => self.config_admin,
            KafkaTransportCapability::AclAdmin => self.acl_admin,
            KafkaTransportCapability::Tls => self.tls,
            KafkaTransportCapability::Sasl => self.sasl,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_capabilities_follow_build_and_security_features() {
        let unavailable = KafkaTransportCapabilities::native(false, true, true);
        assert_eq!(unavailable.backend, KafkaTransportBackend::NativeRdkafka);
        assert!(!unavailable.build_available);
        assert!(!unavailable.supports(KafkaTransportCapability::Metadata));
        assert!(!unavailable.supports(KafkaTransportCapability::Tls));

        let available = KafkaTransportCapabilities::native(true, true, false);
        assert!(available.supports(KafkaTransportCapability::Metadata));
        assert!(available.supports(KafkaTransportCapability::ConfigAdmin));
        assert!(available.supports(KafkaTransportCapability::Tls));
        assert!(!available.supports(KafkaTransportCapability::Sasl));
    }

    #[test]
    fn unknown_capabilities_are_explicitly_unavailable() {
        let capabilities = KafkaTransportCapabilities::unknown();
        assert_eq!(capabilities.backend, KafkaTransportBackend::TestDouble);
        assert!(!capabilities.build_available);
        assert!(!capabilities.supports(KafkaTransportCapability::Fetch));
    }
}
