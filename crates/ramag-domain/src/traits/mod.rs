//! 由基础设施层实现、应用层依赖的领域接口。

pub mod clipboard_driver;
pub mod doc_driver;
pub mod driver;
pub mod git_driver;
pub mod jumpserver_driver;
pub mod kafka_driver;
pub mod kafka_transport;
pub mod kv_driver;
pub mod mqtt_driver;
pub mod mqtt_transport;
pub mod object_storage_driver;
pub mod plugin;
pub mod ssh_driver;
pub mod storage;
pub mod tool;
pub mod update_driver;

pub use clipboard_driver::ClipboardDriver;
pub use doc_driver::DocDriver;
pub use driver::{CancelHandle, Driver};
pub use git_driver::GitDriver;
pub use jumpserver_driver::JumpServerDriver;
pub use kafka_driver::{
    KafkaAdminDriver, KafkaBrokerMetricsDriver, KafkaConnectDriver, KafkaDriver, KafkaKsqlDbDriver,
    KafkaMessageTailSink, KafkaMessageTailSinkResult, KafkaMonitoringDriver, KafkaProducerDriver,
    KafkaSchemaRegistryDriver,
};
pub use kafka_transport::KafkaTransport;
pub use kv_driver::KvDriver;
pub use mqtt_driver::{MosquittoDynamicSecurityDriver, MosquittoStaticConfigDriver, MqttDriver};
pub use mqtt_transport::MqttTransport;
pub use object_storage_driver::ObjectStorageDriver;
pub use plugin::{
    CURRENT_PLUGIN_API_VERSION, KNOWN_PLUGIN_CAPABILITIES, MAX_PLUGIN_CAPABILITIES,
    MAX_PLUGIN_DESCRIPTION_BYTES, MAX_PLUGIN_ENTRY_ID_BYTES, MAX_PLUGIN_ENUM_VALUES,
    MAX_PLUGIN_ID_BYTES, MAX_PLUGIN_NAME_BYTES, MAX_PLUGIN_SETTING_KEY_BYTES,
    MAX_PLUGIN_SETTING_LIST_ITEMS, MAX_PLUGIN_SETTING_VALUE_BYTES, MAX_PLUGIN_SETTINGS,
    PluginApiVersion, PluginCapability, PluginDescriptor, PluginId, PluginRegistrationError,
    PluginSettingDefinition, PluginSettingKind, PluginSettingValue,
};
pub use ssh_driver::SshDriver;
pub use storage::Storage;
pub use tool::{Tool, ToolMeta};
pub use update_driver::UpdateDriver;
