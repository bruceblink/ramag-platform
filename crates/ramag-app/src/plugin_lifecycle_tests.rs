use std::sync::Arc;

use parking_lot::Mutex;
use ramag_domain::{PluginDescriptor, PluginId, Tool, ToolMeta};

use super::*;
use crate::ToolRegistry;

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
