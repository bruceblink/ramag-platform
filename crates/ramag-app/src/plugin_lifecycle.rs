//! 静态插件生命周期编排；不加载外部代码，也不暴露 UI 内部对象。

use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use parking_lot::Mutex;
use ramag_domain::{PluginDescriptor, PluginId, PluginRegistrationError, Tool};
use thiserror::Error;

use crate::ToolRegistry;

/// 单个插件生命周期错误允许进入诊断和日志的最大字节数。
pub const MAX_PLUGIN_OPERATION_ERROR_BYTES: usize = 512;

/// 静态插件当前所处的生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PluginState {
    Registered = 0,
    Initializing = 1,
    Ready = 2,
    ShuttingDown = 3,
    Failed = 4,
    Unloaded = 5,
}

impl PluginState {
    fn from_raw(value: u8) -> Self {
        match value {
            0 => Self::Registered,
            1 => Self::Initializing,
            2 => Self::Ready,
            3 => Self::ShuttingDown,
            4 => Self::Failed,
            _ => Self::Unloaded,
        }
    }
}

impl std::fmt::Display for PluginState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Registered => "Registered",
            Self::Initializing => "Initializing",
            Self::Ready => "Ready",
            Self::ShuttingDown => "ShuttingDown",
            Self::Failed => "Failed",
            Self::Unloaded => "Unloaded",
        };
        formatter.write_str(name)
    }
}

/// 插件保存的上下文在生命周期结束后仍可安全调用，但会被明确拒绝。
#[derive(Clone)]
pub struct PluginContext {
    inner: Arc<PluginContextInner>,
}

struct PluginContextInner {
    plugin_id: PluginId,
    state: AtomicU8,
}

impl PluginContext {
    fn new(plugin_id: PluginId) -> Self {
        Self {
            inner: Arc::new(PluginContextInner {
                plugin_id,
                state: AtomicU8::new(PluginState::Registered as u8),
            }),
        }
    }

    /// 返回当前上下文所属的稳定插件 ID。
    pub fn plugin_id(&self) -> &PluginId {
        &self.inner.plugin_id
    }

    /// 返回宿主记录的最新生命周期状态。
    pub fn state(&self) -> PluginState {
        PluginState::from_raw(self.inner.state.load(Ordering::Acquire))
    }

    /// 检查插件是否仍可访问受控平台服务。
    pub fn ensure_available(&self) -> Result<(), PluginContextError> {
        let state = self.state();
        if matches!(
            state,
            PluginState::Initializing | PluginState::Ready | PluginState::ShuttingDown
        ) {
            return Ok(());
        }
        Err(PluginContextError::Unavailable {
            plugin_id: self.plugin_id().clone(),
            state,
        })
    }

    fn transition(&self, from: PluginState, to: PluginState) -> bool {
        self.inner
            .state
            .compare_exchange(from as u8, to as u8, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn mark_unloaded(&self) {
        self.inner
            .state
            .store(PluginState::Unloaded as u8, Ordering::Release);
    }
}

/// 插件在生命周期回调中访问已关闭上下文时返回的错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginContextError {
    #[error("插件 `{plugin_id}` 当前状态为 {state}，不允许访问运行时上下文")]
    Unavailable {
        plugin_id: PluginId,
        state: PluginState,
    },
}

/// 静态插件生命周期回调返回的有界错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct PluginOperationError {
    message: String,
}

impl PluginOperationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: bounded_message(message.into()),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

/// 生命周期报告中的处理阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginLifecycleStage {
    Initialize,
    Shutdown,
}

impl std::fmt::Display for PluginLifecycleStage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Initialize => "初始化",
            Self::Shutdown => "关闭",
        })
    }
}

/// 单个插件失败的有界诊断，不影响其他插件继续处理。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginLifecycleFailure {
    pub plugin_id: PluginId,
    pub stage: PluginLifecycleStage,
    pub error: PluginOperationError,
}

/// 一次初始化或关闭操作的结果；成功项和失败项都按处理顺序记录。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginLifecycleReport {
    pub succeeded: Vec<PluginId>,
    pub failures: Vec<PluginLifecycleFailure>,
}

impl PluginLifecycleReport {
    pub fn is_success(&self) -> bool {
        self.failures.is_empty()
    }
}

/// 编译期静态插件的最小生命周期接口。
pub trait StaticPlugin: Send + Sync {
    fn descriptor(&self) -> &PluginDescriptor;
    fn tool(&self) -> Arc<dyn Tool>;

    /// 宿主已将插件置为 Initializing 后调用；默认实现只确认上下文有效。
    fn initialize(&self, context: &PluginContext) -> Result<(), PluginOperationError> {
        context
            .ensure_available()
            .map_err(|error| PluginOperationError::new(error.to_string()))
    }

    /// 宿主已将插件置为 ShuttingDown 后调用；默认实现不持有额外资源。
    fn shutdown(&self, context: &PluginContext) -> Result<(), PluginOperationError> {
        context
            .ensure_available()
            .map_err(|error| PluginOperationError::new(error.to_string()))
    }
}

/// 将既有 `Tool` 包装为没有额外生命周期资源的静态插件。
pub struct StaticPluginAdapter {
    descriptor: PluginDescriptor,
    tool: Arc<dyn Tool>,
}

impl StaticPluginAdapter {
    pub fn new(descriptor: PluginDescriptor, tool: Arc<dyn Tool>) -> Self {
        Self { descriptor, tool }
    }

    pub fn from_tool(tool: Arc<dyn Tool>) -> Result<Self, PluginRegistrationError> {
        let meta = tool.meta();
        let plugin_id = PluginId::new(meta.id.clone())?;
        let descriptor = PluginDescriptor::new(plugin_id, meta.name.clone(), meta.id.clone())
            .with_description(meta.description.clone());
        Ok(Self::new(descriptor, tool))
    }
}

impl StaticPlugin for StaticPluginAdapter {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn tool(&self) -> Arc<dyn Tool> {
        self.tool.clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostPhase {
    Registering,
    Running,
    ShuttingDown,
    Stopped,
}

struct PluginRecord {
    plugin: Arc<dyn StaticPlugin>,
    context: PluginContext,
}

impl Clone for PluginRecord {
    fn clone(&self) -> Self {
        Self {
            plugin: self.plugin.clone(),
            context: self.context.clone(),
        }
    }
}

/// 管理静态插件注册、初始化、失败隔离和逆序关闭。
pub struct StaticPluginHost {
    registry: Arc<ToolRegistry>,
    records: Mutex<Vec<PluginRecord>>,
    phase: Mutex<HostPhase>,
    operation_lock: Mutex<()>,
}

impl StaticPluginHost {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            records: Mutex::new(Vec::new()),
            phase: Mutex::new(HostPhase::Registering),
            operation_lock: Mutex::new(()),
        }
    }

    pub fn registry(&self) -> Arc<ToolRegistry> {
        self.registry.clone()
    }

    /// 注册成功后返回；初始化由 `initialize_all` 统一按注册顺序执行。
    pub fn register_plugin(&self, plugin: Arc<dyn StaticPlugin>) -> Result<(), PluginHostError> {
        let _operation = self.operation_lock.lock();
        let descriptor = plugin.descriptor().clone();
        let plugin_id = descriptor.id.clone();
        if *self.phase.lock() != HostPhase::Registering {
            return Err(PluginHostError::RegistrationClosed { plugin_id });
        }

        self.registry
            .register_plugin(descriptor, plugin.tool())
            .map_err(|source| PluginHostError::Registration {
                plugin_id: plugin_id.clone(),
                source,
            })?;
        self.records.lock().push(PluginRecord {
            plugin,
            context: PluginContext::new(plugin_id.clone()),
        });
        tracing::info!(operation = "plugin_host_register", plugin_id = %plugin_id, "static plugin accepted by host");
        Ok(())
    }

    /// 按注册顺序初始化；一个插件失败不会阻塞后续插件。
    pub fn initialize_all(&self) -> PluginLifecycleReport {
        let _operation = self.operation_lock.lock();
        {
            let mut phase = self.phase.lock();
            if *phase != HostPhase::Registering {
                return PluginLifecycleReport::default();
            }
            *phase = HostPhase::Running;
        }

        let records = self.records.lock().clone();
        let mut report = PluginLifecycleReport::default();
        for record in records {
            let plugin_id = record.context.plugin_id().clone();
            if !record
                .context
                .transition(PluginState::Registered, PluginState::Initializing)
            {
                continue;
            }

            match record.plugin.initialize(&record.context) {
                Ok(()) => {
                    let _ = record
                        .context
                        .transition(PluginState::Initializing, PluginState::Ready);
                    report.succeeded.push(plugin_id);
                }
                Err(error) => {
                    record
                        .context
                        .transition(PluginState::Initializing, PluginState::Failed);
                    self.registry
                        .unregister_plugin(record.context.plugin_id().as_str());
                    tracing::warn!(
                        operation = "plugin_initialize",
                        plugin_id = %plugin_id,
                        error = %error,
                        "static plugin initialization failed; plugin disabled"
                    );
                    report.failures.push(PluginLifecycleFailure {
                        plugin_id,
                        stage: PluginLifecycleStage::Initialize,
                        error,
                    });
                }
            }
        }
        report
    }

    /// 按注册顺序逆序关闭已就绪插件；失败仍会释放当前插件并继续处理。
    pub fn shutdown_all(&self) -> PluginLifecycleReport {
        let _operation = self.operation_lock.lock();
        {
            let mut phase = self.phase.lock();
            if matches!(*phase, HostPhase::ShuttingDown | HostPhase::Stopped) {
                return PluginLifecycleReport::default();
            }
            *phase = HostPhase::ShuttingDown;
        }

        let records = self
            .records
            .lock()
            .clone()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        let mut report = PluginLifecycleReport::default();
        for record in records {
            let plugin_id = record.context.plugin_id().clone();
            if record.context.state() == PluginState::Ready
                && record
                    .context
                    .transition(PluginState::Ready, PluginState::ShuttingDown)
            {
                match record.plugin.shutdown(&record.context) {
                    Ok(()) => report.succeeded.push(plugin_id.clone()),
                    Err(error) => {
                        tracing::warn!(
                            operation = "plugin_shutdown",
                            plugin_id = %plugin_id,
                            error = %error,
                            "static plugin shutdown failed; continuing"
                        );
                        report.failures.push(PluginLifecycleFailure {
                            plugin_id: plugin_id.clone(),
                            stage: PluginLifecycleStage::Shutdown,
                            error,
                        });
                    }
                }
            }
            record.context.mark_unloaded();
            self.registry.unregister_plugin(plugin_id.as_str());
        }
        *self.phase.lock() = HostPhase::Stopped;
        report
    }

    pub fn state(&self, plugin_id: &str) -> Option<PluginState> {
        self.records
            .lock()
            .iter()
            .find(|record| record.context.plugin_id().as_str() == plugin_id)
            .map(|record| record.context.state())
    }

    pub fn context(&self, plugin_id: &str) -> Option<PluginContext> {
        self.records
            .lock()
            .iter()
            .find(|record| record.context.plugin_id().as_str() == plugin_id)
            .map(|record| record.context.clone())
    }

    pub fn states(&self) -> Vec<(PluginId, PluginState)> {
        self.records
            .lock()
            .iter()
            .map(|record| (record.context.plugin_id().clone(), record.context.state()))
            .collect()
    }
}

/// 插件未能进入宿主注册表时返回的错误。
#[derive(Debug, Error)]
pub enum PluginHostError {
    #[error("插件 `{plugin_id}` 注册失败：{source}")]
    Registration {
        plugin_id: PluginId,
        #[source]
        source: PluginRegistrationError,
    },
    #[error("插件宿主已开始生命周期处理，不能注册插件 `{plugin_id}`")]
    RegistrationClosed { plugin_id: PluginId },
}

fn bounded_message(mut message: String) -> String {
    if message.len() <= MAX_PLUGIN_OPERATION_ERROR_BYTES {
        return message;
    }
    const SUFFIX: &str = "...";
    let mut end = MAX_PLUGIN_OPERATION_ERROR_BYTES - SUFFIX.len();
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message.truncate(end);
    message.push_str(SUFFIX);
    message
}

#[cfg(test)]
#[path = "plugin_lifecycle_tests.rs"]
mod tests;
