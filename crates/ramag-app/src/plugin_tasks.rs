//! 插件任务的有界句柄和生命周期取消状态。

use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use futures::{
    future::{Either, FutureExt, select},
    pin_mut,
};
use parking_lot::Mutex;
use thiserror::Error;

/// 单个插件同时保留的任务句柄上限。
pub const MAX_PLUGIN_TASKS: usize = 8;
/// 任务名称进入诊断前允许的最大字节数。
pub const MAX_PLUGIN_TASK_NAME_BYTES: usize = 128;
/// 单个任务允许占用的最长执行时间。
pub const MAX_PLUGIN_TASK_TIMEOUT: Duration = Duration::from_secs(60);
/// 单个任务允许返回的最大结果字节数。
pub const MAX_PLUGIN_TASK_RESULT_BYTES: usize = 1024 * 1024;
const MAX_PLUGIN_TASK_ERROR_BYTES: usize = 512;
const TASK_CANCEL_POLL: Duration = Duration::from_millis(10);

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

    /// 在句柄生命周期内运行一个异步任务，并统一应用取消、超时和结果上限。
    pub async fn run<F>(
        &self,
        operation: F,
        budget: PluginTaskBudget,
    ) -> Result<Vec<u8>, PluginTaskRunError>
    where
        F: std::future::Future<Output = Result<Vec<u8>, String>>,
    {
        if self.cancellation_requested() {
            return Err(PluginTaskRunError::Cancelled);
        }

        let operation = operation.fuse();
        let timeout = smol::Timer::after(budget.timeout).fuse();
        let cancelled = wait_for_cancellation(self.cancellation.clone()).fuse();
        pin_mut!(operation, timeout, cancelled);

        let outcome = select(select(operation, timeout), cancelled).await;
        let result = match outcome {
            Either::Left((Either::Left((result, _)), _)) => result,
            Either::Left((Either::Right((_, _)), _)) => return Err(PluginTaskRunError::TimedOut),
            Either::Right((_, _)) => return Err(PluginTaskRunError::Cancelled),
        }
        .map_err(|message| PluginTaskRunError::Failed {
            message: bound_task_error(message),
        })?;

        if result.len() > budget.max_result_bytes {
            return Err(PluginTaskRunError::ResultTooLarge {
                actual: result.len(),
                max: budget.max_result_bytes,
            });
        }
        Ok(result)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginTaskBudget {
    timeout: Duration,
    max_result_bytes: usize,
}

impl PluginTaskBudget {
    pub fn new(timeout: Duration, max_result_bytes: usize) -> Result<Self, PluginTaskBudgetError> {
        if timeout.is_zero() || timeout > MAX_PLUGIN_TASK_TIMEOUT {
            return Err(PluginTaskBudgetError::InvalidTimeout);
        }
        if max_result_bytes == 0 || max_result_bytes > MAX_PLUGIN_TASK_RESULT_BYTES {
            return Err(PluginTaskBudgetError::InvalidResultLimit);
        }
        Ok(Self {
            timeout,
            max_result_bytes,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PluginTaskBudgetError {
    #[error("任务超时时间必须大于 0 且不超过 {MAX_PLUGIN_TASK_TIMEOUT:?}")]
    InvalidTimeout,
    #[error("任务结果上限必须在 1 到 {MAX_PLUGIN_TASK_RESULT_BYTES} 字节之间")]
    InvalidResultLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginTaskRunError {
    #[error("任务已取消")]
    Cancelled,
    #[error("任务执行超时")]
    TimedOut,
    #[error("任务结果超过上限 {max} 字节（实际 {actual} 字节）")]
    ResultTooLarge { actual: usize, max: usize },
    #[error("任务执行失败：{message}")]
    Failed { message: String },
}

async fn wait_for_cancellation(cancellation: Arc<AtomicBool>) {
    loop {
        if cancellation.load(Ordering::Acquire) {
            return;
        }
        smol::Timer::after(TASK_CANCEL_POLL).await;
    }
}

fn bound_task_error(message: String) -> String {
    if message.len() <= MAX_PLUGIN_TASK_ERROR_BYTES {
        return message;
    }
    let mut end = MAX_PLUGIN_TASK_ERROR_BYTES - 3;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &message[..end])
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

    #[test]
    fn task_budget_rejects_zero_and_excessive_limits() {
        assert!(matches!(
            PluginTaskBudget::new(Duration::ZERO, 1),
            Err(PluginTaskBudgetError::InvalidTimeout)
        ));
        assert!(matches!(
            PluginTaskBudget::new(MAX_PLUGIN_TASK_TIMEOUT + Duration::from_secs(1), 1),
            Err(PluginTaskBudgetError::InvalidTimeout)
        ));
        assert!(matches!(
            PluginTaskBudget::new(Duration::from_secs(1), 0),
            Err(PluginTaskBudgetError::InvalidResultLimit)
        ));
        assert!(matches!(
            PluginTaskBudget::new(Duration::from_secs(1), MAX_PLUGIN_TASK_RESULT_BYTES + 1),
            Err(PluginTaskBudgetError::InvalidResultLimit)
        ));
    }

    #[test]
    fn task_run_returns_success_and_bounds_failures_and_results() {
        let registry = Arc::new(PluginTaskRegistry::new());
        let handle = registry.start("query").unwrap();
        let budget = PluginTaskBudget::new(Duration::from_secs(1), 4).unwrap();

        let result = futures::executor::block_on(
            handle.run(async { Ok::<_, String>(b"ok".to_vec()) }, budget),
        )
        .unwrap();
        assert_eq!(result, b"ok");

        let too_large = futures::executor::block_on(
            handle.run(async { Ok::<_, String>(b"12345".to_vec()) }, budget),
        );
        assert!(matches!(
            too_large,
            Err(PluginTaskRunError::ResultTooLarge { actual: 5, max: 4 })
        ));

        let failed = futures::executor::block_on(handle.run(
            async { Err::<Vec<u8>, _>("x".repeat(MAX_PLUGIN_TASK_ERROR_BYTES + 20)) },
            budget,
        ));
        assert!(matches!(
            failed,
            Err(PluginTaskRunError::Failed { message }) if message.len() <= MAX_PLUGIN_TASK_ERROR_BYTES
        ));
    }

    #[test]
    fn task_run_reports_timeout_and_cancellation() {
        let registry = Arc::new(PluginTaskRegistry::new());
        let handle = registry.start("query").unwrap();
        let timeout = PluginTaskBudget::new(Duration::from_millis(10), 1).unwrap();
        let timed_out = futures::executor::block_on(handle.run(
            async {
                smol::Timer::after(Duration::from_secs(1)).await;
                Ok::<_, String>(Vec::new())
            },
            timeout,
        ));
        assert!(matches!(timed_out, Err(PluginTaskRunError::TimedOut)));

        let flag = handle.cancellation_flag();
        let canceller = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(5));
            flag.store(true, Ordering::Release);
        });
        let cancelled = futures::executor::block_on(handle.run(
            async {
                smol::Timer::after(Duration::from_secs(1)).await;
                Ok::<_, String>(Vec::new())
            },
            PluginTaskBudget::new(Duration::from_secs(1), 1).unwrap(),
        ));
        canceller.join().unwrap();
        assert!(matches!(cancelled, Err(PluginTaskRunError::Cancelled)));
    }
}
