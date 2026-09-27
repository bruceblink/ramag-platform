//! 静态插件宿主注册、设置加载和生命周期编排。

use std::collections::HashMap;
use std::sync::Arc;

use futures::executor::block_on;
use parking_lot::Mutex;
use ramag_domain::{PluginDescriptor, PluginId, PluginRegistrationError, Storage};
use thiserror::Error;

use crate::plugin_lifecycle::{
    PluginContext, PluginDiagnostic, PluginDiagnosticFailure, PluginLifecycleFailure,
    PluginLifecycleReport, PluginLifecycleStage, PluginOperationError, PluginPermissionPolicy,
    PluginState, StaticPlugin, bounded_message,
};
use crate::{
    PluginSecretStore, PluginSettingsMigrator, PluginSettingsStore, PluginTaskBudget,
    PluginTaskExecution, ToolRegistry,
};

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
    permission_policy: PluginPermissionPolicy,
    storage: Option<Arc<dyn Storage>>,
    settings_migrator: PluginSettingsMigrator,
    failures: Mutex<HashMap<PluginId, PluginDiagnosticFailure>>,
    registration_failures: Mutex<Vec<PluginDiagnostic>>,
}

impl StaticPluginHost {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self::with_options(
            registry,
            PluginPermissionPolicy::default(),
            None,
            PluginSettingsMigrator::default(),
        )
    }

    pub fn with_permission_policy(
        registry: Arc<ToolRegistry>,
        permission_policy: PluginPermissionPolicy,
    ) -> Self {
        Self::with_options(
            registry,
            permission_policy,
            None,
            PluginSettingsMigrator::default(),
        )
    }

    pub fn with_storage(registry: Arc<ToolRegistry>, storage: Arc<dyn Storage>) -> Self {
        Self::with_options(
            registry,
            PluginPermissionPolicy::default(),
            Some(storage),
            PluginSettingsMigrator::default(),
        )
    }

    pub fn with_storage_and_permissions(
        registry: Arc<ToolRegistry>,
        storage: Arc<dyn Storage>,
        permission_policy: PluginPermissionPolicy,
        settings_migrator: PluginSettingsMigrator,
    ) -> Self {
        Self::with_options(
            registry,
            permission_policy,
            Some(storage),
            settings_migrator,
        )
    }

    /// 集中创建宿主状态，确保测试和生产构造器使用同一套生命周期、存储和迁移配置。
    fn with_options(
        registry: Arc<ToolRegistry>,
        permission_policy: PluginPermissionPolicy,
        storage: Option<Arc<dyn Storage>>,
        settings_migrator: PluginSettingsMigrator,
    ) -> Self {
        Self {
            registry,
            records: Mutex::new(Vec::new()),
            phase: Mutex::new(HostPhase::Registering),
            operation_lock: Mutex::new(()),
            permission_policy,
            storage,
            settings_migrator,
            failures: Mutex::new(HashMap::new()),
            registration_failures: Mutex::new(Vec::new()),
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
            self.record_registration_failure(
                descriptor,
                format!("插件宿主已开始生命周期处理，不能注册插件 `{plugin_id}`"),
            );
            return Err(PluginHostError::RegistrationClosed { plugin_id });
        }

        if let Err(source) = self
            .registry
            .register_plugin_entries(descriptor.clone(), plugin.tools())
        {
            self.record_registration_failure(descriptor, source.to_string());
            return Err(PluginHostError::Registration { plugin_id, source });
        }
        self.records.lock().push(PluginRecord {
            plugin,
            context: PluginContext::new(
                plugin_id.clone(),
                descriptor.capabilities.clone(),
                self.permission_policy.grants_for(&plugin_id),
            ),
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

            if let Err(error) = self.prepare_context(&record) {
                record.context.stop_tasks();
                record
                    .context
                    .transition(PluginState::Initializing, PluginState::Failed);
                self.registry
                    .unregister_plugin(record.context.plugin_id().as_str());
                self.failures.lock().insert(
                    plugin_id.clone(),
                    PluginDiagnosticFailure {
                        stage: PluginLifecycleStage::Initialize,
                        message: error.message().to_owned(),
                    },
                );
                report.failures.push(PluginLifecycleFailure {
                    plugin_id,
                    stage: PluginLifecycleStage::Initialize,
                    error,
                });
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
                    record.context.stop_tasks();
                    record
                        .context
                        .transition(PluginState::Initializing, PluginState::Failed);
                    self.registry
                        .unregister_plugin(record.context.plugin_id().as_str());
                    self.failures.lock().insert(
                        plugin_id.clone(),
                        PluginDiagnosticFailure {
                            stage: PluginLifecycleStage::Initialize,
                            message: error.message().to_owned(),
                        },
                    );
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
                record.context.stop_tasks();
                match record.plugin.shutdown(&record.context) {
                    Ok(()) => report.succeeded.push(plugin_id.clone()),
                    Err(error) => {
                        self.failures.lock().insert(
                            plugin_id.clone(),
                            PluginDiagnosticFailure {
                                stage: PluginLifecycleStage::Shutdown,
                                message: error.message().to_owned(),
                            },
                        );
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

    /// 校验并提交一个静态插件入口；返回值持有任务句柄直到等待或分离完成。
    pub fn execute_entry(
        &self,
        plugin_id: &str,
        entry_id: &str,
        input: Vec<u8>,
        budget: PluginTaskBudget,
    ) -> Result<PluginTaskExecution, PluginEntryExecutionError> {
        let record = self
            .records
            .lock()
            .iter()
            .find(|record| record.context.plugin_id().as_str() == plugin_id)
            .cloned()
            .ok_or_else(|| PluginEntryExecutionError::PluginNotFound {
                plugin_id: plugin_id.to_owned(),
            })?;
        if record.context.state() != PluginState::Ready {
            return Err(PluginEntryExecutionError::NotReady {
                plugin_id: plugin_id.to_owned(),
                state: record.context.state(),
            });
        }

        let entry = record
            .plugin
            .descriptor()
            .entry_descriptors()
            .into_iter()
            .find(|entry| entry.id == entry_id)
            .ok_or_else(|| PluginEntryExecutionError::EntryNotFound {
                plugin_id: plugin_id.to_owned(),
                entry_id: entry_id.to_owned(),
            })?;
        if input.len() > entry.input.max_bytes {
            return Err(PluginEntryExecutionError::InputTooLarge {
                entry_id: entry.id,
                actual: input.len(),
                max: entry.input.max_bytes,
            });
        }

        let task_budget = PluginTaskBudget::new(
            budget.timeout(),
            budget.max_result_bytes().min(entry.output.max_bytes),
        )
        .map_err(PluginEntryExecutionError::InvalidBudget)?;
        let task = record
            .context
            .start_task(format!("entry:{entry_id}"))
            .map_err(PluginEntryExecutionError::TaskUnavailable)?;
        let operation = record
            .plugin
            .execute(entry_id, input, &record.context)
            .map_err(PluginEntryExecutionError::Operation)?;
        Ok(task.spawn(operation, task_budget))
    }

    pub fn states(&self) -> Vec<(PluginId, PluginState)> {
        self.records
            .lock()
            .iter()
            .map(|record| (record.context.plugin_id().clone(), record.context.state()))
            .collect()
    }

    /// 返回所有已注册插件以及注册阶段失败项，供 UI 展示可用入口和故障原因。
    pub fn diagnostics(&self) -> Vec<PluginDiagnostic> {
        let failures = self.failures.lock().clone();
        let mut diagnostics = self
            .records
            .lock()
            .iter()
            .map(|record| {
                let plugin_id = record.context.plugin_id().clone();
                PluginDiagnostic {
                    descriptor: record.plugin.descriptor().clone(),
                    state: record.context.state(),
                    failure: failures.get(&plugin_id).cloned(),
                }
            })
            .collect::<Vec<_>>();
        diagnostics.extend(self.registration_failures.lock().clone());
        diagnostics
    }

    fn record_registration_failure(&self, descriptor: PluginDescriptor, message: String) {
        self.registration_failures.lock().push(PluginDiagnostic {
            descriptor,
            state: PluginState::Failed,
            failure: Some(PluginDiagnosticFailure {
                stage: PluginLifecycleStage::Registration,
                message: bounded_message(message),
            }),
        });
    }

    /// 在插件初始化前读取并校验两个快照；任一失败都阻止回调执行并由调用方隔离插件。
    fn prepare_context(&self, record: &PluginRecord) -> Result<(), PluginOperationError> {
        let descriptor = record.plugin.descriptor().clone();
        let Some(storage) = self.storage.clone() else {
            if descriptor.settings.is_empty() {
                return Ok(());
            }
            return Err(PluginOperationError::new(
                "插件声明了设置，但宿主没有配置插件存储",
            ));
        };
        let settings_store =
            PluginSettingsStore::with_migrator(storage.clone(), self.settings_migrator.clone());
        let secret_store = PluginSecretStore::new(storage);
        let result = block_on(async {
            let settings = settings_store
                .load(&descriptor)
                .await
                .map_err(|error| PluginOperationError::new(error.to_string()))?;
            let secrets = secret_store
                .load(&descriptor)
                .await
                .map_err(|error| PluginOperationError::new(error.to_string()))?;
            Ok::<_, PluginOperationError>((settings.snapshot, secrets))
        })?;
        record.context.install_snapshots(result.0, result.1);
        Ok(())
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

/// 静态插件入口提交前的校验或适配错误。
#[derive(Debug, Error)]
pub enum PluginEntryExecutionError {
    #[error("插件 `{plugin_id}` 未注册")]
    PluginNotFound { plugin_id: String },
    #[error("插件 `{plugin_id}` 当前状态为 {state}，入口不可执行")]
    NotReady {
        plugin_id: String,
        state: PluginState,
    },
    #[error("插件 `{plugin_id}` 没有入口 `{entry_id}`")]
    EntryNotFound { plugin_id: String, entry_id: String },
    #[error("入口 `{entry_id}` 输入超过 {max} 字节（实际 {actual} 字节）")]
    InputTooLarge {
        entry_id: String,
        actual: usize,
        max: usize,
    },
    #[error("入口预算无效：{0}")]
    InvalidBudget(#[source] crate::PluginTaskBudgetError),
    #[error("插件任务不可用：{0}")]
    TaskUnavailable(#[source] crate::PluginContextError),
    #[error("入口执行适配失败：{0}")]
    Operation(#[source] PluginOperationError),
}
