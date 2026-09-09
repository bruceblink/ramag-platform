//! 领域实体与接口，不依赖 UI 和基础设施实现。

pub mod entities;
pub mod error;
pub mod traits;

pub use error::{DomainError, KafkaError, KafkaErrorCategory, Result};
pub use traits::{
    Driver, KafkaAdminDriver, KafkaBrokerMetricsDriver, KafkaDriver, KafkaMessageTailSink,
    KafkaMessageTailSinkResult, KafkaProducerDriver, KafkaTransport, KvDriver, PluginApiVersion,
    PluginCapability, PluginDescriptor, PluginId, PluginRegistrationError, PluginSettingDefinition,
    PluginSettingKind, PluginSettingValue, SshDriver, Storage, Tool, ToolMeta,
};
