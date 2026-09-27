use std::sync::Arc;

use futures::executor::block_on;
use parking_lot::Mutex;
use ramag_domain::{
    PluginCapability, PluginDescriptor, PluginEntryDescriptor, PluginId, PluginSettingDefinition,
    PluginSettingKind, PluginSettingValue, Storage, Tool, ToolMeta,
};
use ramag_infra_storage::RedbStorage;
use tempfile::tempdir;

use super::*;
use crate::{PluginSettingsMigrator, PluginSettingsStore, ToolRegistry};

struct DummyTool {
    meta: ToolMeta,
}

impl Tool for DummyTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

struct RecordingPlugin {
    descriptor: PluginDescriptor,
    tool: Arc<DummyTool>,
    events: Arc<Mutex<Vec<String>>>,
    fail_initialize: bool,
    fail_shutdown: bool,
}

impl RecordingPlugin {
    fn new(
        id: &str,
        events: Arc<Mutex<Vec<String>>>,
        fail_initialize: bool,
        fail_shutdown: bool,
    ) -> Self {
        let plugin_id = PluginId::new(id).unwrap();
        Self {
            descriptor: PluginDescriptor::new(plugin_id, id, id),
            tool: Arc::new(DummyTool {
                meta: ToolMeta::new(id, id, ""),
            }),
            events,
            fail_initialize,
            fail_shutdown,
        }
    }
}

impl StaticPlugin for RecordingPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn tool(&self) -> Arc<dyn Tool> {
        self.tool.clone()
    }

    fn initialize(&self, context: &PluginContext) -> Result<(), PluginOperationError> {
        context
            .ensure_available()
            .map_err(|error| PluginOperationError::new(error.to_string()))?;
        self.events
            .lock()
            .push(format!("initialize:{}", context.plugin_id()));
        if self.fail_initialize {
            Err(PluginOperationError::new("initialize failed"))
        } else {
            Ok(())
        }
    }

    fn shutdown(&self, context: &PluginContext) -> Result<(), PluginOperationError> {
        context
            .ensure_available()
            .map_err(|error| PluginOperationError::new(error.to_string()))?;
        self.events
            .lock()
            .push(format!("shutdown:{}", context.plugin_id()));
        if self.fail_shutdown {
            Err(PluginOperationError::new("shutdown failed"))
        } else {
            Ok(())
        }
    }
}

fn host_with_plugins(
    plugins: impl IntoIterator<Item = RecordingPlugin>,
) -> (StaticPluginHost, Arc<Mutex<Vec<String>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let host = StaticPluginHost::new(Arc::new(ToolRegistry::new()));
    for plugin in plugins {
        host.register_plugin(Arc::new(plugin)).unwrap();
    }
    (host, events)
}

#[test]
fn initialization_runs_in_registration_order_and_keeps_ready_plugins() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (host, _) = host_with_plugins([
        RecordingPlugin::new("first", events.clone(), false, false),
        RecordingPlugin::new("second", events.clone(), false, false),
        RecordingPlugin::new("third", events.clone(), false, false),
    ]);

    let report = host.initialize_all();

    assert!(report.is_success());
    assert_eq!(
        report
            .succeeded
            .iter()
            .map(PluginId::as_str)
            .collect::<Vec<_>>(),
        ["first", "second", "third"]
    );
    assert_eq!(
        *events.lock(),
        ["initialize:first", "initialize:second", "initialize:third"]
    );
    assert_eq!(host.registry().order(), ["first", "second", "third"]);
    assert_eq!(host.state("second"), Some(PluginState::Ready));
}

#[test]
fn initialization_failure_removes_only_failed_plugin_and_continues() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (host, _) = host_with_plugins([
        RecordingPlugin::new("first", events.clone(), false, false),
        RecordingPlugin::new("broken", events.clone(), true, false),
        RecordingPlugin::new("third", events.clone(), false, false),
    ]);

    let report = host.initialize_all();

    assert!(!report.is_success());
    assert_eq!(report.succeeded.len(), 2);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].plugin_id.as_str(), "broken");
    assert_eq!(report.failures[0].stage, PluginLifecycleStage::Initialize);
    assert_eq!(host.state("broken"), Some(PluginState::Failed));
    assert_eq!(host.registry().order(), ["first", "third"]);
    assert_eq!(host.registry().count(), 2);
    let diagnostic = host
        .diagnostics()
        .into_iter()
        .find(|diagnostic| diagnostic.descriptor.id.as_str() == "broken")
        .expect("初始化失败插件应保留诊断");
    assert_eq!(diagnostic.state, PluginState::Failed);
    assert_eq!(
        diagnostic.failure.as_ref().map(|failure| failure.stage),
        Some(PluginLifecycleStage::Initialize)
    );
    assert_eq!(
        *events.lock(),
        ["initialize:first", "initialize:broken", "initialize:third"]
    );
}

#[test]
fn registration_failure_does_not_create_a_lifecycle_record_or_block_next_plugin() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let host = StaticPluginHost::new(Arc::new(ToolRegistry::new()));
    let mismatched_tool = Arc::new(DummyTool {
        meta: ToolMeta::new("actual", "Actual", ""),
    });
    let descriptor =
        PluginDescriptor::new(PluginId::new("declared").unwrap(), "Declared", "declared");
    let error = host
        .register_plugin(Arc::new(StaticPluginAdapter::new(
            descriptor,
            mismatched_tool,
        )))
        .unwrap_err();

    assert!(matches!(
        error,
        PluginHostError::Registration {
            source: ramag_domain::PluginRegistrationError::EntryIdMismatch { .. },
            ..
        }
    ));
    assert!(host.states().is_empty());
    let diagnostic = host
        .diagnostics()
        .into_iter()
        .find(|diagnostic| diagnostic.descriptor.id.as_str() == "declared")
        .expect("注册失败插件应保留诊断");
    assert_eq!(diagnostic.state, PluginState::Failed);
    assert_eq!(
        diagnostic.failure.as_ref().map(|failure| failure.stage),
        Some(PluginLifecycleStage::Registration)
    );

    host.register_plugin(Arc::new(RecordingPlugin::new(
        "valid", events, false, false,
    )))
    .unwrap();
    let report = host.initialize_all();
    assert!(report.is_success());
    assert_eq!(host.registry().order(), ["valid"]);
}

#[test]
fn shutdown_runs_in_reverse_order_and_is_idempotent() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (host, _) = host_with_plugins([
        RecordingPlugin::new("first", events.clone(), false, false),
        RecordingPlugin::new("second", events.clone(), false, false),
        RecordingPlugin::new("third", events.clone(), false, false),
    ]);
    host.initialize_all();
    events.lock().clear();

    let report = host.shutdown_all();

    assert!(report.is_success());
    assert_eq!(
        report
            .succeeded
            .iter()
            .map(PluginId::as_str)
            .collect::<Vec<_>>(),
        ["third", "second", "first"]
    );
    assert_eq!(
        *events.lock(),
        ["shutdown:third", "shutdown:second", "shutdown:first"]
    );
    assert_eq!(host.registry().count(), 0);
    assert_eq!(
        host.states(),
        [
            (PluginId::new("first").unwrap(), PluginState::Unloaded),
            (PluginId::new("second").unwrap(), PluginState::Unloaded),
            (PluginId::new("third").unwrap(), PluginState::Unloaded),
        ]
    );
    assert!(host.shutdown_all().is_success());
}

#[test]
fn shutdown_failure_does_not_prevent_reverse_cleanup() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (host, _) = host_with_plugins([
        RecordingPlugin::new("first", events.clone(), false, false),
        RecordingPlugin::new("broken", events.clone(), false, true),
        RecordingPlugin::new("third", events.clone(), false, false),
    ]);
    host.initialize_all();
    events.lock().clear();

    let report = host.shutdown_all();

    assert_eq!(
        *events.lock(),
        ["shutdown:third", "shutdown:broken", "shutdown:first"]
    );
    assert_eq!(report.succeeded.len(), 2);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].plugin_id.as_str(), "broken");
    assert_eq!(report.failures[0].stage, PluginLifecycleStage::Shutdown);
    assert_eq!(host.registry().count(), 0);
    assert_eq!(host.state("broken"), Some(PluginState::Unloaded));
    let diagnostic = host
        .diagnostics()
        .into_iter()
        .find(|diagnostic| diagnostic.descriptor.id.as_str() == "broken")
        .expect("关闭失败插件应保留诊断");
    assert_eq!(diagnostic.state, PluginState::Unloaded);
    assert_eq!(
        diagnostic.failure.as_ref().map(|failure| failure.stage),
        Some(PluginLifecycleStage::Shutdown)
    );
}

#[test]
fn saved_context_is_rejected_after_shutdown() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (host, _) = host_with_plugins([RecordingPlugin::new("example", events, false, false)]);
    host.initialize_all();
    let context = host.context("example").unwrap();
    assert!(context.ensure_available().is_ok());

    host.shutdown_all();

    assert_eq!(context.state(), PluginState::Unloaded);
    assert!(matches!(
        context.ensure_available(),
        Err(PluginContextError::Unavailable {
            state: PluginState::Unloaded,
            ..
        })
    ));
}

#[test]
fn registration_is_closed_after_initialization_starts() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (host, _) =
        host_with_plugins([RecordingPlugin::new("first", events.clone(), false, false)]);
    host.initialize_all();

    let error = host
        .register_plugin(Arc::new(RecordingPlugin::new("late", events, false, false)))
        .unwrap_err();

    assert!(matches!(
        error,
        PluginHostError::RegistrationClosed { plugin_id } if plugin_id.as_str() == "late"
    ));
    assert_eq!(host.registry().order(), ["first"]);
    assert!(host.diagnostics().iter().any(|diagnostic| {
        diagnostic.descriptor.id.as_str() == "late"
            && diagnostic
                .failure
                .as_ref()
                .is_some_and(|failure| failure.stage == PluginLifecycleStage::Registration)
    }));
}

#[test]
fn operation_errors_are_bounded() {
    let error = PluginOperationError::new("x".repeat(MAX_PLUGIN_OPERATION_ERROR_BYTES * 2));

    assert!(error.message().len() <= MAX_PLUGIN_OPERATION_ERROR_BYTES);
    assert!(error.message().ends_with("..."));
}

struct MultiEntryPlugin {
    descriptor: PluginDescriptor,
    tools: Vec<Arc<DummyTool>>,
    events: Arc<Mutex<Vec<String>>>,
}

impl MultiEntryPlugin {
    fn new(events: Arc<Mutex<Vec<String>>>) -> Self {
        let plugin_id = PluginId::new("bundle.example").unwrap();
        let descriptor =
            PluginDescriptor::new(plugin_id, "Bundle", "bundle.first").with_entries(vec![
                PluginEntryDescriptor::new("bundle.first", "First"),
                PluginEntryDescriptor::new("bundle.second", "Second"),
            ]);
        Self {
            descriptor,
            tools: vec![
                Arc::new(DummyTool {
                    meta: ToolMeta::new("bundle.first", "First", ""),
                }),
                Arc::new(DummyTool {
                    meta: ToolMeta::new("bundle.second", "Second", ""),
                }),
            ],
            events,
        }
    }
}

impl StaticPlugin for MultiEntryPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn tool(&self) -> Arc<dyn Tool> {
        self.tools[0].clone()
    }

    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools
            .iter()
            .cloned()
            .map(|tool| tool as Arc<dyn Tool>)
            .collect()
    }

    fn initialize(&self, context: &PluginContext) -> Result<(), PluginOperationError> {
        context
            .ensure_available()
            .map_err(|error| PluginOperationError::new(error.to_string()))?;
        self.events.lock().push("initialized".into());
        Ok(())
    }
}

#[test]
fn host_registers_multiple_entries_and_unloads_them_as_one_plugin() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let host = StaticPluginHost::new(Arc::new(ToolRegistry::new()));
    host.register_plugin(Arc::new(MultiEntryPlugin::new(events.clone())))
        .unwrap();

    assert_eq!(host.registry().order(), ["bundle.first", "bundle.second"]);
    assert_eq!(host.registry().plugin_descriptors().len(), 1);
    assert_eq!(
        host.catalog()
            .iter()
            .map(|entry| entry.entry_id.as_str())
            .collect::<Vec<_>>(),
        ["bundle.first", "bundle.second"]
    );
    assert!(host.catalog().iter().all(|entry| entry.desktop));
    assert!(host.catalog().iter().all(|entry| !entry.web));
    assert_eq!(host.initialize_all().succeeded.len(), 1);
    assert_eq!(*events.lock(), ["initialized"]);

    host.shutdown_all();
    assert_eq!(host.registry().count(), 0);
    assert!(host.catalog().is_empty());
}

struct SettingsPlugin {
    descriptor: PluginDescriptor,
    tool: Arc<DummyTool>,
    events: Arc<Mutex<Vec<String>>>,
}

impl SettingsPlugin {
    fn new(events: Arc<Mutex<Vec<String>>>) -> Self {
        let plugin_id = PluginId::new("settings.example").unwrap();
        let descriptor = PluginDescriptor::new(plugin_id, "Settings", "settings.example")
            .with_capabilities([
                PluginCapability::new("ui.entry"),
                PluginCapability::new("storage.plugin"),
            ])
            .with_settings(vec![
                PluginSettingDefinition::new("mode", PluginSettingKind::Enum)
                    .with_enum_values(["safe", "full"])
                    .with_default(PluginSettingValue::String("safe".into())),
            ]);
        Self {
            descriptor,
            tool: Arc::new(DummyTool {
                meta: ToolMeta::new("settings.example", "Settings", ""),
            }),
            events,
        }
    }
}

impl StaticPlugin for SettingsPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn tool(&self) -> Arc<dyn Tool> {
        self.tool.clone()
    }

    fn initialize(&self, context: &PluginContext) -> Result<(), PluginOperationError> {
        let snapshot = context
            .settings_snapshot()
            .map_err(|error| PluginOperationError::new(error.to_string()))?;
        let mode = match snapshot.get("mode") {
            Some(PluginSettingValue::String(value)) => value.clone(),
            _ => "missing".into(),
        };
        self.events.lock().push(mode);
        Ok(())
    }
}

#[test]
fn host_loads_settings_before_initialize_and_exposes_only_checked_snapshot() {
    let directory = tempdir().unwrap();
    let storage: Arc<dyn Storage> = Arc::new(
        RedbStorage::open_with_key(directory.path().join("settings.redb").as_path(), &[7; 32])
            .unwrap(),
    );
    let descriptor = SettingsPlugin::new(Arc::new(Mutex::new(Vec::new()))).descriptor;
    let seed = PluginSettingsSnapshot::from_namespaced_values(
        &descriptor,
        [(
            "plugin.settings.example.mode",
            PluginSettingValue::String("full".into()),
        )],
    )
    .unwrap();
    block_on(PluginSettingsStore::new(storage.clone()).save(&seed)).unwrap();

    let events = Arc::new(Mutex::new(Vec::new()));
    let host = StaticPluginHost::with_storage_and_permissions(
        Arc::new(ToolRegistry::new()),
        storage,
        permissions_for_settings_plugin(),
        PluginSettingsMigrator::default(),
    );
    host.register_plugin(Arc::new(SettingsPlugin::new(events.clone())))
        .unwrap();

    let report = host.initialize_all();

    assert!(report.is_success());
    assert_eq!(*events.lock(), ["full"]);
    assert_eq!(host.registry().order(), ["settings.example"]);
}

#[test]
fn settings_plugin_without_storage_fails_before_initialize() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let host = StaticPluginHost::with_permission_policy(
        Arc::new(ToolRegistry::new()),
        permissions_for_settings_plugin(),
    );
    host.register_plugin(Arc::new(SettingsPlugin::new(events.clone())))
        .unwrap();

    let report = host.initialize_all();

    assert_eq!(report.succeeded.len(), 0);
    assert_eq!(report.failures.len(), 1);
    assert!(events.lock().is_empty());
    assert_eq!(host.registry().count(), 0);
    assert_eq!(host.state("settings.example"), Some(PluginState::Failed));
}

fn permissions_for_settings_plugin() -> PluginPermissionPolicy {
    let mut policy = PluginPermissionPolicy::default();
    policy.grant(
        PluginId::new("settings.example").unwrap(),
        PluginCapability::new("storage.plugin"),
    );
    policy
}

#[test]
fn capability_checks_require_declaration_and_explicit_grant() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let plugin = RecordingPlugin::new("capable", events, false, false);
    let mut policy = PluginPermissionPolicy::default();
    policy.grant(
        PluginId::new("capable").unwrap(),
        PluginCapability::new("task.scoped"),
    );
    let host = StaticPluginHost::with_permission_policy(Arc::new(ToolRegistry::new()), policy);
    host.register_plugin(Arc::new(plugin)).unwrap();
    host.initialize_all();
    let context = host.context("capable").unwrap();

    assert!(matches!(
        context.require_capability("task.scoped"),
        Err(PluginContextError::CapabilityNotDeclared { .. })
    ));
    assert!(matches!(
        context.require_capability("ui.entry"),
        Err(PluginContextError::CapabilityNotGranted { .. })
    ));
}

#[test]
fn granted_declared_capability_is_rejected_after_unload() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut plugin = RecordingPlugin::new("capable", events, false, false);
    plugin.descriptor = plugin
        .descriptor
        .clone()
        .with_capabilities([PluginCapability::new("task.scoped")]);
    let mut policy = PluginPermissionPolicy::default();
    policy.grant(
        PluginId::new("capable").unwrap(),
        PluginCapability::new("task.scoped"),
    );
    let host = StaticPluginHost::with_permission_policy(Arc::new(ToolRegistry::new()), policy);
    host.register_plugin(Arc::new(plugin)).unwrap();
    host.initialize_all();
    let context = host.context("capable").unwrap();
    assert!(context.require_capability("task.scoped").is_ok());

    host.shutdown_all();

    assert!(matches!(
        context.require_capability("task.scoped"),
        Err(PluginContextError::Unavailable {
            state: PluginState::Unloaded,
            ..
        })
    ));
}
