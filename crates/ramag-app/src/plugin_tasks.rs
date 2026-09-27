//! 插件任务的有界句柄和生命周期取消状态。

use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use parking_lot::Mutex;
use thiserror::Error;

/// 单个插件同时保留的任务句柄上限。
pub const MAX_PLUGIN_TASKS: usize = 8;
/// 任务名称进入诊断前允许的最大字节数。
pub const MAX_PLUGIN_TASK_NAME_BYTES: usize = 128;

#[derive(Debug, Default)]
struct TaskInner {
    next_id: u64,
    accepting: bool,
    tasks: HashMap<u64, Arc<AtomicBool>>,
}

/// 由一个 `PluginContext` 持有的任务登记表；关闭时统一发出取消信号。
#[derive(Debug, Default)]
pub(crate) struct PluginTaskRegistry {
    inner: Mutex<TaskInner>,
}

impl PluginTaskRegistry {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(TaskInner {
                accepting: true,
                ..TaskInner::default()
            }),
        }
    }

    /// 为插件创建一个有界任务句柄；任务实际执行器由后续平台层决定。
    pub(crate) fn start(
        self: &Arc<Self>,
        name: impl Into<String>,
    ) -> Result<PluginTaskHandle, PluginTaskError> {
        let name = name.into();
        validate_name(&name)?;
        let mut inner = self.inner.lock();
        if !inner.accepting {
            return Err(PluginTaskError::NotAccepting);
        }
        if inner.tasks.len() >= MAX_PLUGIN_TASKS {
            return Err(PluginTaskError::LimitReached {
                max: MAX_PLUGIN_TASKS,
            });
        }
        inner.next_id = inner.next_id.saturating_add(1);
        if inner.next_id == 0 {
            inner.next_id = 1;
        }
        let id = inner.next_id;
        let cancellation = Arc::new(AtomicBool::new(false));
        inner.tasks.insert(id, cancellation.clone());
        Ok(PluginTaskHandle {
            id,
            name,
            cancellation,
            registry: self.clone(),
        })
    }

    /// 停止接受新任务并取消全部仍被插件持有的句柄。
    pub(crate) fn cancel_all(&self) {
        let mut inner = self.inner.lock();
        inner.accepting = false;
        for cancellation in inner.tasks.values() {
            cancellation.store(true, Ordering::Release);
        }
    }

    #[cfg(test)]
    fn active_count(&self) -> usize {
        self.inner.lock().tasks.len()
    }
}

/// 插件任务句柄；丢弃句柄会从登记表移除，取消状态可安全跨线程读取。
pub struct PluginTaskHandle {
    id: u64,
    name: String,
    cancellation: Arc<AtomicBool>,
    registry: Arc<PluginTaskRegistry>,
}

impl std::fmt::Debug for PluginTaskHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PluginTaskHandle")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("cancelled", &self.cancellation_requested())
            .finish()
    }
}

impl PluginTaskHandle {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn cancellation_requested(&self) -> bool {
        self.cancellation.load(Ordering::Acquire)
    }

    pub fn cancellation_flag(&self) -> Arc<AtomicBool> {
        self.cancellation.clone()
    }
}

impl Drop for PluginTaskHandle {
    fn drop(&mut self) {
        self.registry.inner.lock().tasks.remove(&self.id);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginTaskError {
    #[error("任务名称不能为空")]
    EmptyName,
    #[error("任务名称超过 {MAX_PLUGIN_TASK_NAME_BYTES} 字节")]
    NameTooLong,
    #[error("任务名称包含控制字符")]
    InvalidName,
    #[error("活动任务达到上限 {max}")]
    LimitReached { max: usize },
    #[error("插件已停止接受新任务")]
    NotAccepting,
}

fn validate_name(name: &str) -> Result<(), PluginTaskError> {
    if name.is_empty() {
        return Err(PluginTaskError::EmptyName);
    }
    if name.len() > MAX_PLUGIN_TASK_NAME_BYTES {
        return Err(PluginTaskError::NameTooLong);
    }
    if name.chars().any(char::is_control) {
        return Err(PluginTaskError::InvalidName);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_count_is_bounded_and_drop_releases_a_slot() {
        let registry = Arc::new(PluginTaskRegistry::new());
        let mut handles = Vec::new();
        for index in 0..MAX_PLUGIN_TASKS {
            handles.push(registry.start(format!("task-{index}")).unwrap());
        }
        assert_eq!(registry.active_count(), MAX_PLUGIN_TASKS);
        assert!(matches!(
            registry.start("overflow"),
            Err(PluginTaskError::LimitReached { .. })
        ));

        drop(handles.pop());
        assert_eq!(registry.active_count(), MAX_PLUGIN_TASKS - 1);
        assert!(registry.start("replacement").is_ok());
    }

    #[test]
    fn cancel_all_marks_existing_handles_and_rejects_new_work() {
        let registry = Arc::new(PluginTaskRegistry::new());
        let handle = registry.start("query").unwrap();
        registry.cancel_all();

        assert!(handle.cancellation_requested());
        assert!(matches!(
            registry.start("late"),
            Err(PluginTaskError::NotAccepting)
        ));
    }

    #[test]
    fn task_names_are_bounded_and_control_free() {
        let registry = Arc::new(PluginTaskRegistry::new());
        assert!(matches!(
            registry.start(""),
            Err(PluginTaskError::EmptyName)
        ));
        assert!(matches!(
            registry.start("\nwork"),
            Err(PluginTaskError::InvalidName)
        ));
        assert!(matches!(
            registry.start("x".repeat(MAX_PLUGIN_TASK_NAME_BYTES + 1)),
            Err(PluginTaskError::NameTooLong)
        ));
    }
}
