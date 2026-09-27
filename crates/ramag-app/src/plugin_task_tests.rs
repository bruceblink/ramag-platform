use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use ramag_domain::{PluginCapability, PluginDescriptor, PluginId, Tool, ToolMeta};

use super::*;
use crate::{PluginEntryExecutionError, PluginTaskBudget, PluginTaskRunError, ToolRegistry};

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
    delay: Duration,
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

    fn execute(
        &self,
        _entry_id: &str,
        input: Vec<u8>,
        _context: &PluginContext,
    ) -> Result<PluginEntryFuture, PluginOperationError> {
        let delay = self.delay;
        Ok(Box::pin(async move {
            if !delay.is_zero() {
                smol::Timer::after(delay).await;
            }
            Ok(input)
        }))
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
        delay: Duration::ZERO,
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

#[test]
fn host_executes_valid_entry_and_enforces_input_boundary() {
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
        handle,
        delay: Duration::ZERO,
    };
    let mut policy = PluginPermissionPolicy::default();
    policy.grant(plugin_id.clone(), PluginCapability::new("task.scoped"));
    let host = StaticPluginHost::with_permission_policy(registry, policy);
    host.register_plugin(Arc::new(plugin)).unwrap();
    assert!(host.initialize_all().is_success());

    let execution = host
        .execute_entry(
            plugin_id.as_str(),
            "task.example",
            b"hello".to_vec(),
            PluginTaskBudget::new(Duration::from_secs(1), 32).unwrap(),
        )
        .unwrap();
    assert_eq!(smol::block_on(execution.join()).unwrap(), b"hello");
    let oversized_result = host
        .execute_entry(
            plugin_id.as_str(),
            "task.example",
            b"hello".to_vec(),
            PluginTaskBudget::new(Duration::from_secs(1), 2).unwrap(),
        )
        .unwrap();
    assert!(matches!(
        smol::block_on(oversized_result.join()),
        Err(PluginTaskRunError::ResultTooLarge { actual: 5, max: 2 })
    ));
    assert!(matches!(
        host.execute_entry(
            plugin_id.as_str(),
            "missing",
            Vec::new(),
            PluginTaskBudget::new(Duration::from_secs(1), 32).unwrap(),
        ),
        Err(PluginEntryExecutionError::EntryNotFound { .. })
    ));
    assert!(matches!(
        host.execute_entry(
            plugin_id.as_str(),
            "task.example",
            vec![0; 64 * 1024 + 1],
            PluginTaskBudget::new(Duration::from_secs(1), 32).unwrap(),
        ),
        Err(PluginEntryExecutionError::InputTooLarge { .. })
    ));
}

#[test]
fn host_shutdown_cancels_submitted_entry_execution() {
    let registry = Arc::new(ToolRegistry::new());
    let plugin_id = PluginId::new("task.slow").unwrap();
    let descriptor = PluginDescriptor::new(plugin_id.clone(), "Slow task", "task.slow")
        .with_capabilities([
            PluginCapability::new("ui.entry"),
            PluginCapability::new("task.scoped"),
        ]);
    let plugin = TaskPlugin {
        descriptor,
        tool: Arc::new(DummyTaskTool {
            meta: ToolMeta::new("task.slow", "Slow task", ""),
        }),
        handle: Arc::new(Mutex::new(None)),
        delay: Duration::from_secs(1),
    };
    let mut policy = PluginPermissionPolicy::default();
    policy.grant(plugin_id.clone(), PluginCapability::new("task.scoped"));
    let host = StaticPluginHost::with_permission_policy(registry, policy);
    host.register_plugin(Arc::new(plugin)).unwrap();
    assert!(host.initialize_all().is_success());

    let execution = host
        .execute_entry(
            plugin_id.as_str(),
            "task.slow",
            Vec::new(),
            PluginTaskBudget::new(Duration::from_secs(1), 32).unwrap(),
        )
        .unwrap();
    host.shutdown_all();
    let outcome = smol::block_on(execution.join_with_outcome());
    assert!(matches!(
        outcome.result(),
        Err(PluginTaskRunError::Cancelled)
    ));
    assert!(outcome.elapsed() >= Duration::ZERO);
    assert_eq!(outcome.output_bytes(), None);
}
