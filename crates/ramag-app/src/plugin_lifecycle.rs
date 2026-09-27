//! 静态插件生命周期编排；不加载外部代码，也不暴露 UI 内部对象。

use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use parking_lot::RwLock;
use ramag_domain::{PluginCapability, PluginDescriptor, PluginId, PluginRegistrationError, Tool};
use thiserror::Error;

use crate::plugin_tasks::{PluginTaskError, PluginTaskHandle, PluginTaskRegistry};
use crate::{PluginSecretSnapshot, PluginSettingsSnapshot};

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
    declared_capabilities: HashSet<PluginCapability>,
    granted_capabilities: HashSet<PluginCapability>,
    settings: RwLock<Option<PluginSettingsSnapshot>>,
    secrets: RwLock<Option<PluginSecretSnapshot>>,
    tasks: Arc<PluginTaskRegistry>,
    state: AtomicU8,
}

impl PluginContext {
    pub(crate) fn new(
        plugin_id: PluginId,
        declared_capabilities: impl IntoIterator<Item = PluginCapability>,
        granted_capabilities: impl IntoIterator<Item = PluginCapability>,
    ) -> Self {
        Self {
            inner: Arc::new(PluginContextInner {
                plugin_id,
                declared_capabilities: declared_capabilities.into_iter().collect(),
                granted_capabilities: granted_capabilities.into_iter().collect(),
                settings: RwLock::new(None),
                secrets: RwLock::new(None),
                tasks: Arc::new(PluginTaskRegistry::new()),
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

    /// 检查插件当前是否获准使用指定能力；声明、授予和生命周期都必须同时满足。
    pub fn require_capability(&self, capability: &str) -> Result<(), PluginContextError> {
        self.ensure_available()?;
        let capability_value = PluginCapability::new(capability);
        if !self.inner.declared_capabilities.contains(&capability_value) {
            return Err(PluginContextError::CapabilityNotDeclared {
                plugin_id: self.plugin_id().clone(),
                capability: capability.to_owned(),
            });
        }
        if !self.inner.granted_capabilities.contains(&capability_value) {
            return Err(PluginContextError::CapabilityNotGranted {
                plugin_id: self.plugin_id().clone(),
                capability: capability.to_owned(),
            });
        }
        Ok(())
    }

    /// 返回已校验的普通设置副本；读取设置必须同时拥有存储能力和已加载快照。
    pub fn settings_snapshot(&self) -> Result<PluginSettingsSnapshot, PluginContextError> {
        self.require_capability("storage.plugin")?;
        self.inner
            .settings
            .read()
            .clone()
            .ok_or_else(|| PluginContextError::SettingsUnavailable {
                plugin_id: self.plugin_id().clone(),
            })
    }

    /// 返回已校验的敏感设置副本；秘密值不会混入普通设置快照。
    pub fn secret_snapshot(&self) -> Result<PluginSecretSnapshot, PluginContextError> {
        self.require_capability("storage.plugin")?;
        self.inner
            .secrets
            .read()
            .clone()
            .ok_or_else(|| PluginContextError::SecretsUnavailable {
                plugin_id: self.plugin_id().clone(),
            })
    }

    /// 为插件创建受能力和数量上限约束的任务句柄；句柄丢弃后释放活动任务名额。
    pub fn start_task(
        &self,
        name: impl Into<String>,
    ) -> Result<PluginTaskHandle, PluginContextError> {
        self.require_capability("task.scoped")?;
        if self.state() == PluginState::ShuttingDown {
            return Err(PluginContextError::TaskUnavailable {
                plugin_id: self.plugin_id().clone(),
                source: PluginTaskError::NotAccepting,
            });
        }
        self.inner
            .tasks
            .start(name)
            .map_err(|source| PluginContextError::TaskUnavailable {
                plugin_id: self.plugin_id().clone(),
                source,
            })
    }

    /// 宿主在初始化回调前安装已校验的普通设置和秘密快照；插件只能通过只读公开方法取得副本。
    pub(crate) fn install_snapshots(
        &self,
        settings: PluginSettingsSnapshot,
        secrets: PluginSecretSnapshot,
    ) {
        *self.inner.settings.write() = Some(settings);
        *self.inner.secrets.write() = Some(secrets);
    }

    pub(crate) fn transition(&self, from: PluginState, to: PluginState) -> bool {
        self.inner
            .state
            .compare_exchange(from as u8, to as u8, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn mark_unloaded(&self) {
        self.stop_tasks();
        self.inner
            .state
            .store(PluginState::Unloaded as u8, Ordering::Release);
    }

    pub(crate) fn stop_tasks(&self) {
        self.inner.tasks.cancel_all();
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
    #[error("插件 `{plugin_id}` 未声明能力 `{capability}`")]
    CapabilityNotDeclared {
        plugin_id: PluginId,
        capability: String,
    },
    #[error("插件 `{plugin_id}` 未获准使用能力 `{capability}`")]
    CapabilityNotGranted {
        plugin_id: PluginId,
        capability: String,
    },
    #[error("插件 `{plugin_id}` 的普通设置尚未加载")]
    SettingsUnavailable { plugin_id: PluginId },
    #[error("插件 `{plugin_id}` 的秘密设置尚未加载")]
    SecretsUnavailable { plugin_id: PluginId },
    #[error("插件 `{plugin_id}` 的任务不可用：{source}")]
    TaskUnavailable {
        plugin_id: PluginId,
        #[source]
        source: PluginTaskError,
    },
}

/// 宿主对插件能力的授予策略；默认不授予任何能力，避免清单声明自动扩大权限。
#[derive(Debug, Clone, Default)]
pub struct PluginPermissionPolicy {
    grants: HashMap<PluginId, HashSet<PluginCapability>>,
}

impl PluginPermissionPolicy {
    /// 授予一个插件已声明的能力；未声明的能力仍会在上下文检查时拒绝。
    pub fn grant(&mut self, plugin_id: PluginId, capability: PluginCapability) {
        self.grants.entry(plugin_id).or_default().insert(capability);
    }

    pub(crate) fn grants_for(&self, plugin_id: &PluginId) -> HashSet<PluginCapability> {
        self.grants.get(plugin_id).cloned().unwrap_or_default()
    }
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
    Registration,
    Initialize,
    Shutdown,
}

impl std::fmt::Display for PluginLifecycleStage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Registration => "注册",
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

/// 可供设置页展示的单条插件失败信息；消息沿用生命周期错误的有界长度。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDiagnosticFailure {
    pub stage: PluginLifecycleStage,
    pub message: String,
}

/// 插件描述、生命周期状态和最近一次失败的只读快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDiagnostic {
    pub descriptor: PluginDescriptor,
    pub state: PluginState,
    pub failure: Option<PluginDiagnosticFailure>,
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

    /// 返回该插件提供的全部工具入口；旧插件默认只暴露兼容的单入口。
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![self.tool()]
    }

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

pub use crate::plugin_host::{PluginHostError, StaticPluginHost};

pub(crate) fn bounded_message(mut message: String) -> String {
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

#[cfg(test)]
#[path = "plugin_task_tests.rs"]
mod task_tests;
