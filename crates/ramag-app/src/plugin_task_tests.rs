use std::sync::Arc;

use parking_lot::Mutex;
use ramag_domain::{PluginCapability, PluginDescriptor, PluginId, Tool, ToolMeta};

use super::*;
use crate::ToolRegistry;

struct DummyTaskTool {
    meta: ToolMeta,
}

impl Tool for DummyTaskTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

fn task_context() -> PluginContext {
    let context = PluginContext::new(
        PluginId::new("task.example").unwrap(),
        [PluginCapability::new("task.scoped")],
        [PluginCapability::new("task.scoped")],
    );
    assert!(context.transition(PluginState::Registered, PluginState::Initializing));
    context
}

#[test]
fn task_start_requires_explicit_capability() {
    let context = PluginContext::new(
        PluginId::new("task.example").unwrap(),
        [PluginCapability::new("ui.entry")],
        [PluginCapability::new("ui.entry")],
    );
    assert!(context.transition(PluginState::Registered, PluginState::Initializing));

    assert!(matches!(
        context.start_task("query"),
        Err(PluginContextError::CapabilityNotDeclared { .. })
    ));
}

#[test]
fn context_stop_tasks_cancels_existing_handles_and_rejects_new_work() {
    let context = task_context();
    let handle = context.start_task("query").unwrap();

    context.stop_tasks();

    assert!(handle.cancellation_requested());
    assert!(matches!(
        context.start_task("late"),
        Err(PluginContextError::TaskUnavailable {
            source: PluginTaskError::NotAccepting,
            ..
        })
    ));
}

struct TaskPlugin {
    descriptor: PluginDescriptor,
    tool: Arc<DummyTaskTool>,
    handle: Arc<Mutex<Option<PluginTaskHandle>>>,
}

impl StaticPlugin for TaskPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn tool(&self) -> Arc<dyn Tool> {
        self.tool.clone()
    }

    fn initialize(&self, context: &PluginContext) -> Result<(), PluginOperationError> {
        let handle = context
            .start_task("background")
            .map_err(|error| PluginOperationError::new(error.to_string()))?;
        *self.handle.lock() = Some(handle);
        Ok(())
    }
}

#[test]
fn host_shutdown_cancels_plugin_tasks_before_shutdown_callback() {
    let registry = Arc::new(ToolRegistry::new());
    let handle = Arc::new(Mutex::new(None));
    let plugin_id = PluginId::new("task.example").unwrap();
    let descriptor = PluginDescriptor::new(plugin_id.clone(), "Task", "task.example")
        .with_capabilities([
            PluginCapability::new("ui.entry"),
            PluginCapability::new("task.scoped"),
        ]);
    let plugin = TaskPlugin {
        descriptor,
        tool: Arc::new(DummyTaskTool {
            meta: ToolMeta::new("task.example", "Task", ""),
        }),
        handle: handle.clone(),
    };
    let mut policy = PluginPermissionPolicy::default();
    policy.grant(plugin_id, PluginCapability::new("task.scoped"));
    let host = StaticPluginHost::with_permission_policy(registry, policy);
    host.register_plugin(Arc::new(plugin)).unwrap();
    assert!(host.initialize_all().is_success());
    assert!(!handle.lock().as_ref().unwrap().cancellation_requested());

    host.shutdown_all();

    assert!(handle.lock().as_ref().unwrap().cancellation_requested());
}
