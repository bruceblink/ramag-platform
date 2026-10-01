//! 跨工具的轻量偏好。状态放在 App Global 中，修改后所有已打开视图立即读取同一值。

pub(crate) mod status;

#[cfg(test)]
#[path = "preferences/test_storage.rs"]
pub(crate) mod test_storage;

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures::lock::Mutex;
use gpui_kit::{App, Global};
use parking_lot::RwLock;
use ramag_domain::traits::Storage;

/// Result for the most recent draft; Saved is reported only after storage confirms the write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreferenceSaveStatus {
    Saving,
    Saved,
    Failed(String),
}

/// One session-local draft for a static preference key, never a history of previous values.
/// Keep a failed value for explicit retry; release it after success. Callers only pass UI
/// preferences, not connection secrets. The value is never included in diagnostics.
#[derive(Clone)]
struct PreferenceEntry {
    revision: u64,
    value: String,
    status: PreferenceSaveStatus,
}

/// Serializes writes across keys and replaces queued drafts for the same static key.
/// Revisions protect the status as well as disk ordering when a newer draft arrives
/// during a write. Only the background executor awaits storage; the app owns repainting.
#[derive(Clone)]
struct PreferenceWriter {
    next_revision: Arc<AtomicU64>,
    entries: Arc<RwLock<HashMap<&'static str, PreferenceEntry>>>,
    lock: Arc<Mutex<()>>,
}

impl Default for PreferenceWriter {
    fn default() -> Self {
        Self {
            next_revision: Arc::new(AtomicU64::new(0)),
            entries: Arc::new(RwLock::new(HashMap::new())),
            lock: Arc::new(Mutex::new(())),
        }
    }
}

impl PreferenceWriter {
    /// Records a bounded preference key's newest draft before asynchronous persistence begins.
    fn begin(&self, key: &'static str, value: String) -> u64 {
        let revision = self
            .next_revision
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        self.entries.write().insert(
            key,
            PreferenceEntry {
                revision,
                value,
                status: PreferenceSaveStatus::Saving,
            },
        );
        revision
    }

    /// Applies a completion only while it still belongs to the latest draft for this key.
    fn complete(&self, key: &'static str, revision: u64, result: Result<(), String>) -> bool {
        let mut entries = self.entries.write();
        let Some(entry) = entries
            .get_mut(key)
            .filter(|entry| entry.revision == revision)
        else {
            return false;
        };
        entry.status = match result {
            Ok(()) => {
                entry.value = String::new();
                PreferenceSaveStatus::Saved
            }
            Err(reason) => PreferenceSaveStatus::Failed(reason.chars().take(512).collect()),
        };
        true
    }

    fn status(&self, key: &'static str) -> Option<PreferenceSaveStatus> {
        self.entries
            .read()
            .get(key)
            .map(|entry| entry.status.clone())
    }

    fn retry_value(&self, key: &'static str) -> Option<String> {
        self.entries.read().get(key).and_then(|entry| {
            matches!(entry.status, PreferenceSaveStatus::Failed(_)).then(|| entry.value.clone())
        })
    }
}

struct PreferenceWriterGlobal(PreferenceWriter);
impl Global for PreferenceWriterGlobal {}

fn writer(cx: &mut App) -> PreferenceWriter {
    if let Some(global) = cx.try_global::<PreferenceWriterGlobal>() {
        global.0.clone()
    } else {
        let writer = PreferenceWriter::default();
        cx.set_global(PreferenceWriterGlobal(writer.clone()));
        writer
    }
}

/// Returns an owned status for a preference key used by the current app's settings.
pub(crate) fn preference_save_status(key: &'static str, cx: &App) -> Option<PreferenceSaveStatus> {
    cx.try_global::<PreferenceWriterGlobal>()
        .and_then(|global| global.0.status(key))
}

/// Retries the last failed draft for a key; the ordinary latest-value writer still owns ordering.
pub(crate) fn retry_failed_preference(key: &'static str, cx: &mut App) {
    let Some(value) = cx
        .try_global::<PreferenceWriterGlobal>()
        .and_then(|global| global.0.retry_value(key))
    else {
        return;
    };
    persist_preference_latest(key, value, cx);
}

/// 串行写入并丢弃同 key 的过期任务，保证用户快速连续操作后“最后一次选择”最终落盘。
pub fn persist_preference_latest(key: &'static str, value: String, cx: &mut App) {
    let writer = writer(cx);
    let revision = writer.begin(key, value.clone());
    cx.refresh_windows();
    let Some(storage) = crate::theme::storage_from_cx(cx) else {
        writer.complete(key, revision, Err("应用存储不可用".to_string()));
        cx.refresh_windows();
        return;
    };
    persist_with_writer(key, value, storage, writer, revision, cx);
}

/// 使用调用方持有的存储执行“同 key 仅最新值落盘”；适合本身已注入 Storage 的工具视图。
pub fn persist_preference_latest_with_storage(
    key: &'static str,
    value: String,
    storage: Arc<dyn Storage>,
    cx: &mut App,
) {
    let writer = writer(cx);
    let revision = writer.begin(key, value.clone());
    cx.refresh_windows();
    persist_with_writer(key, value, storage, writer, revision, cx);
}

/// Performs storage work on the background executor and applies its result on the app thread.
fn persist_with_writer(
    key: &'static str,
    value: String,
    storage: Arc<dyn Storage>,
    writer: PreferenceWriter,
    revision: u64,
    cx: &mut App,
) {
    let operation = async move {
        storage
            .set_preference(key, &value)
            .await
            .map_err(|error| error.to_string())
    };
    schedule_write(key, writer, revision, operation, cx);
}

/// Awaits the storage future off-thread, then publishes only a current revision on the app thread.
fn schedule_write<F>(
    key: &'static str,
    writer: PreferenceWriter,
    revision: u64,
    operation: F,
    cx: &mut App,
) where
    F: Future<Output = Result<(), String>> + Send + 'static,
{
    cx.spawn(async move |app| {
        let writer_for_task = writer.clone();
        let result = app
            .background_executor()
            .spawn(async move {
                let _guard = writer_for_task.lock.lock().await;
                if writer_for_task
                    .entries
                    .read()
                    .get(key)
                    .is_none_or(|entry| entry.revision != revision)
                {
                    return None;
                }
                let result = operation.await;
                if let Err(reason) = &result {
                    tracing::warn!(operation = "ui_preference_persist", error = %reason, preference = key, "persist preference failed");
                }
                Some(result)
            })
            .await;
        if let Some(result) = result {
            app.update(|cx| {
                if writer.complete(key, revision, result) {
                    cx.refresh_windows();
                }
            });
        }
    })
    .detach();
}

#[cfg(test)]
#[path = "preferences_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "preferences/status_tests.rs"]
mod status_tests;
