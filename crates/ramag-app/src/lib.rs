// 测试允许 unwrap、expect 和 panic，不影响生产代码审计。
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

//! 应用层：编排领域接口实现业务用例。

mod blocking;
pub mod connection_transfer;
mod plugin_catalog;
mod plugin_host;
mod plugin_lifecycle;
mod plugin_secrets;
mod plugin_settings;
mod plugin_settings_store;
mod plugin_tasks;
pub mod tool_registry;
pub mod usecases;

pub use blocking::run_blocking;
pub use plugin_catalog::{
    MAX_PLUGIN_CATALOG_ENTRIES, PluginCatalog, PluginCatalogAcceptance, PluginCatalogEntry,
    PluginCatalogError, PluginDataHandling,
};
pub use plugin_host::PluginEntryExecutionError;
pub use plugin_lifecycle::{
    MAX_PLUGIN_OPERATION_ERROR_BYTES, PluginContext, PluginContextError, PluginDiagnostic,
    PluginDiagnosticFailure, PluginEntryFuture, PluginHostError, PluginLifecycleFailure,
    PluginLifecycleReport, PluginLifecycleStage, PluginOperationError, PluginPermissionPolicy,
    PluginState, StaticPlugin, StaticPluginAdapter, StaticPluginHost,
};
pub use plugin_secrets::{
    CURRENT_PLUGIN_SECRET_FORMAT_VERSION, MAX_PLUGIN_SECRET_PLAINTEXT_BYTES,
    MAX_PLUGIN_SECRET_STORED_BYTES, PluginSecretError, PluginSecretSnapshot, PluginSecretStore,
    PluginSecretStoreError,
};
pub use plugin_settings::{PluginSettingsError, PluginSettingsSnapshot};
pub use plugin_settings_store::{
    CURRENT_PLUGIN_SETTINGS_FORMAT_VERSION, MAX_PLUGIN_SETTINGS_MIGRATION_STEPS,
    MAX_PLUGIN_SETTINGS_PAYLOAD_BYTES, PluginSettingsLoad, PluginSettingsMigrationError,
    PluginSettingsMigrationFailure, PluginSettingsMigrationRegistrationError,
    PluginSettingsMigrator, PluginSettingsStore, PluginSettingsStoreError,
};
pub use plugin_tasks::{
    MAX_PLUGIN_TASK_NAME_BYTES, MAX_PLUGIN_TASK_RESULT_BYTES, MAX_PLUGIN_TASK_TIMEOUT,
    MAX_PLUGIN_TASKS, PluginTaskBudget, PluginTaskBudgetError, PluginTaskError,
    PluginTaskExecution, PluginTaskHandle, PluginTaskRunError,
};
pub use tool_registry::{TOOL_ORDER_PREF_KEY, ToolRegistry};
pub use usecases::{
    AUTO_CHECK_INTERVAL, AccountVerification, ApiService, AvailableUpdate, ClipboardService,
    CollaborationService, CollaborationServiceError, ConnectionService, ContainerRegistryService,
    ContainerService, DataSyncConfirmation, DataSyncExecutionContext, DataSyncGate,
    DataSyncGatePhase, DataSyncGateSnapshot, DataSyncObjectCatalog, DataSyncPermit,
    DataSyncPreflightReport, DataSyncService, HotkeyState, KafkaService,
    MAX_DATA_SYNC_CATALOG_OBJECTS, MongoService, MqttService, ObjectListingPage,
    ObjectStorageMountResult, ObjectStorageService, PreparedDataSync, RedisService,
    SavedObjectStorageAccount, SshService, StartedDataSync, UPDATE_CHECK_PREF_KEY,
    UpdateCheckResult, UpdatePlatform, UpdateService, asset_name_for, configured_mounts,
    convert_id_to_integer, convert_id_to_string, current_platform, new_api_cancellation,
};
