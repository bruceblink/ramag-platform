//! Versioned monitor-workspace presets.
//!
//! A preset stores only the fixed Ramag monitor surface's sampling and
//! presentation choices. Connections, credentials, samples, and the global
//! application appearance remain outside this snapshot boundary.

use std::collections::BTreeMap;

use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

use crate::{MonitorPresentationSettings, MonitorSettings};

pub const MONITOR_PRESETS_PREF_KEY: &str = "monitor_presets";
const CURRENT_PRESET_VERSION: u8 = 1;
const MAX_PRESETS: usize = 100;
const MAX_PRESET_NAME_BYTES: usize = 64;
const MAX_PRESET_LIBRARY_BYTES: usize = 256 * 1024;

/// Names reserved by the reference application's built-in workspace presets.
pub const BUILTIN_PRESET_NAMES: [&str; 4] = ["Default", "Minimal", "GPU Focus", "Developer"];

/// One bounded snapshot of monitor controls; it never contains live readings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorPreset {
    #[serde(default)]
    pub monitor_settings: MonitorSettings,
    #[serde(default)]
    pub presentation: MonitorPresentationSettings,
}

impl MonitorPreset {
    pub fn bounded(mut self) -> Self {
        self.presentation = self.presentation.bounded();
        self
    }
}

/// Named monitor-workspace snapshots with a version and hard storage limits.
/// The map is ordered for deterministic serialization and stable UI ordering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorPresetLibrary {
    #[serde(default = "default_version")]
    pub version: u8,
    #[serde(default)]
    pub presets: BTreeMap<String, MonitorPreset>,
}

impl Default for MonitorPresetLibrary {
    fn default() -> Self {
        Self {
            version: CURRENT_PRESET_VERSION,
            presets: BTreeMap::new(),
        }
    }
}

impl MonitorPresetLibrary {
    /// Parses a saved library without silently replacing invalid snapshots.
    pub fn parse(raw: &str) -> Result<Self, String> {
        if raw.is_empty() {
            return Ok(Self::default());
        }
        if raw.len() > MAX_PRESET_LIBRARY_BYTES {
            return Err("监控预设超过存储大小限制".into());
        }
        let library: Self =
            serde_json::from_str(raw).map_err(|error| format!("监控预设格式无效：{error}"))?;
        library.validate()?;
        Ok(library)
    }

    /// Serializes only a valid, bounded library so malformed state cannot overwrite storage.
    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        let json =
            serde_json::to_string(self).map_err(|error| format!("序列化监控预设失败：{error}"))?;
        if json.len() > MAX_PRESET_LIBRARY_BYTES {
            return Err("监控预设超过存储大小限制".into());
        }
        Ok(json)
    }

    /// Adds or replaces one named snapshot after validating the complete candidate.
    pub fn upsert(&mut self, name: &str, preset: MonitorPreset) -> Result<(), String> {
        let name = validate_name(name)?;
        if !self.presets.contains_key(&name) && self.presets.len() >= MAX_PRESETS {
            return Err(format!("监控预设最多保存 {MAX_PRESETS} 项"));
        }
        let mut candidate = self.clone();
        candidate.presets.insert(name, preset.bounded());
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Renames a snapshot without losing its state or overwriting another name.
    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), String> {
        let from = validate_name(from)?;
        let to = validate_name(to)?;
        if from == to {
            return Ok(());
        }
        if self.presets.contains_key(&to) {
            return Err("目标预设名称已存在".into());
        }
        let Some(preset) = self.presets.get(&from).cloned() else {
            return Err("源预设不存在".into());
        };
        let mut candidate = self.clone();
        candidate.presets.remove(&from);
        candidate.presets.insert(to, preset);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> bool {
        self.presets.remove(name).is_some()
    }

    fn validate(&self) -> Result<(), String> {
        if self.version != CURRENT_PRESET_VERSION {
            return Err(format!("不支持的监控预设版本：{}", self.version));
        }
        if self.presets.len() > MAX_PRESETS {
            return Err(format!("监控预设最多保存 {MAX_PRESETS} 项"));
        }
        for (name, preset) in &self.presets {
            validate_name(name)?;
            if preset.presentation != preset.presentation.clone().bounded() {
                return Err(format!("预设“{name}”包含超出限制的展示设置"));
            }
        }
        Ok(())
    }
}

fn default_version() -> u8 {
    CURRENT_PRESET_VERSION
}

fn validate_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("预设名称不能为空".into());
    }
    if name.len() > MAX_PRESET_NAME_BYTES {
        return Err(format!("预设名称不能超过 {MAX_PRESET_NAME_BYTES} 字节"));
    }
    if name.chars().any(char::is_control) {
        return Err("预设名称不能包含控制字符".into());
    }
    if BUILTIN_PRESET_NAMES
        .iter()
        .any(|builtin| builtin.eq_ignore_ascii_case(name))
    {
        return Err("内置预设名称受保护，请使用其他名称".into());
    }
    Ok(name.to_owned())
}

#[derive(Clone)]
struct MonitorPresetLibraryState {
    library: MonitorPresetLibrary,
    load_error: Option<String>,
}

pub struct MonitorPresetLibraryGlobal(MonitorPresetLibraryState);
impl Global for MonitorPresetLibraryGlobal {}

pub fn monitor_preset_library(cx: &App) -> MonitorPresetLibrary {
    cx.try_global::<MonitorPresetLibraryGlobal>()
        .map(|global| global.0.library.clone())
        .unwrap_or_default()
}

/// Reports invalid persisted data while keeping it untouched until explicit repair.
pub fn monitor_preset_library_load_error(cx: &App) -> Option<String> {
    cx.try_global::<MonitorPresetLibraryGlobal>()
        .and_then(|global| global.0.load_error.clone())
}

fn publish_monitor_preset_library(
    library: MonitorPresetLibrary,
    load_error: Option<String>,
    cx: &mut App,
) {
    cx.set_global(MonitorPresetLibraryGlobal(MonitorPresetLibraryState {
        library,
        load_error,
    }));
    cx.refresh_windows();
}

/// Replaces the in-memory library without clearing a persisted-load error.
/// Only a successful initialization from valid stored data unlocks saving again.
pub fn set_monitor_preset_library(library: MonitorPresetLibrary, cx: &mut App) {
    let load_error = monitor_preset_library_load_error(cx);
    publish_monitor_preset_library(library, load_error, cx);
}

pub fn init_monitor_preset_library(preference: Option<&str>, cx: &mut App) -> Result<(), String> {
    match preference.map(MonitorPresetLibrary::parse).transpose() {
        Ok(library) => {
            publish_monitor_preset_library(library.unwrap_or_default(), None, cx);
            Ok(())
        }
        Err(error) => {
            publish_monitor_preset_library(
                MonitorPresetLibrary::default(),
                Some(error.clone()),
                cx,
            );
            Err(error)
        }
    }
}

/// Publishes a valid library and queues persistence unless the stored value failed to load.
/// Returns validation and load-protection errors before any write is queued.
pub fn save_monitor_preset_library(
    library: MonitorPresetLibrary,
    cx: &mut App,
) -> Result<(), String> {
    if let Some(load_error) = monitor_preset_library_load_error(cx) {
        return Err(format!(
            "监控预设库读取失败，已拒绝覆盖原数据：{load_error}"
        ));
    }

    let json = library.to_json()?;
    publish_monitor_preset_library(library, None, cx);
    crate::preferences::persist_preference_latest(MONITOR_PRESETS_PREF_KEY, json, cx);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_preset_round_trips_and_keeps_monitor_scope() -> Result<(), String> {
        let mut library = MonitorPresetLibrary::default();
        library.upsert(
            "GPU Work",
            MonitorPreset {
                monitor_settings: MonitorSettings {
                    refresh_rate: crate::MonitorRefreshRate::HalfSecond,
                },
                presentation: MonitorPresentationSettings {
                    selected_devices: BTreeMap::from([("gpu".into(), "gpu-1".into())]),
                    selected_sensors: BTreeMap::from([("energy".into(), "gpu.power".into())]),
                    hidden_sensors: Default::default(),
                },
            },
        )?;
        let restored = MonitorPresetLibrary::parse(&library.to_json()?)?;
        assert_eq!(restored, library);
        assert_eq!(restored.version, 1);
        Ok(())
    }

    #[test]
    fn malformed_or_oversized_library_is_rejected() {
        assert!(MonitorPresetLibrary::parse(r#"{"version":2,"presets":{}}"#).is_err());
        assert!(MonitorPresetLibrary::parse(r#"{"version":1,"presets":{"":"bad"}}"#).is_err());
        assert!(MonitorPresetLibrary::parse(&" ".repeat(MAX_PRESET_LIBRARY_BYTES + 1)).is_err());
        assert!(
            MonitorPresetLibrary::default()
                .clone()
                .upsert("gpu focus", MonitorPreset::default())
                .is_err()
        );
        assert!(
            MonitorPresetLibrary::default()
                .clone()
                .upsert("  ", MonitorPreset::default())
                .is_err()
        );
    }

    #[test]
    fn rename_and_remove_preserve_candidate_boundaries() -> Result<(), String> {
        let mut library = MonitorPresetLibrary::default();
        library.upsert("Work", MonitorPreset::default())?;
        assert!(library.rename("Work", "Coding").is_ok());
        assert!(library.presets.contains_key("Coding"));
        assert!(!library.presets.contains_key("Work"));
        assert!(library.remove("Coding"));
        assert!(!library.remove("Coding"));
        Ok(())
    }

    #[gpui_kit::test]
    fn invalid_library_save_keeps_original_redb_value_after_reopen(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use ramag_domain::traits::Storage as _;
        use ramag_infra_storage::RedbStorage;
        use std::sync::Arc;

        cx.update(gpui_kit::component::init);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ramag-monitor-presets-test-{}-{stamp}.redb",
            std::process::id()
        ));
        let opened = RedbStorage::open_with_key(&path, &[0x6d; 32]);
        assert!(opened.is_ok(), "isolated preference database should open");
        let Ok(store) = opened else { return };
        let original = r#"{"version":7,"presets":{}}"#;
        let write_result =
            futures::executor::block_on(store.set_preference(MONITOR_PRESETS_PREF_KEY, original));
        assert!(write_result.is_ok(), "test value should be written to redb");

        let test_store = Arc::new(crate::preferences::test_storage::SettingsTestStorage::new(
            store,
        ));
        let storage: Arc<dyn ramag_domain::traits::Storage> = test_store.clone();
        let loaded = futures::executor::block_on(storage.get_preference(MONITOR_PRESETS_PREF_KEY));
        assert_eq!(loaded.ok().flatten().as_deref(), Some(original));

        let mut load_result = None;
        cx.update(|app| {
            app.set_global(crate::StorageGlobal(storage.clone()));
            load_result = Some(init_monitor_preset_library(Some(original), app));
        });
        assert!(load_result.is_some_and(|result| result.is_err()));

        let mut candidate = MonitorPresetLibrary::default();
        let candidate_result = candidate.upsert("Work", MonitorPreset::default());
        assert!(candidate_result.is_ok());
        let mut save_result = None;
        cx.update(|app| {
            save_result = Some(save_monitor_preset_library(candidate, app));
        });
        assert!(save_result.is_some_and(|result| result.is_err()));

        let value_after_attempt =
            futures::executor::block_on(storage.get_preference(MONITOR_PRESETS_PREF_KEY));
        assert_eq!(
            value_after_attempt.ok().flatten().as_deref(),
            Some(original)
        );
        cx.update(|app| app.remove_global::<crate::StorageGlobal>());
        drop(storage);
        drop(test_store);

        let reopened_result = RedbStorage::open_with_key(&path, &[0x6d; 32]);
        assert!(reopened_result.is_ok(), "preference database should reopen");
        let Ok(reopened) = reopened_result else {
            let _ = std::fs::remove_file(&path);
            return;
        };
        let reopened_value =
            futures::executor::block_on(reopened.get_preference(MONITOR_PRESETS_PREF_KEY));
        assert_eq!(reopened_value.ok().flatten().as_deref(), Some(original));
        drop(reopened);
        let removed = std::fs::remove_file(&path);
        assert!(
            removed.is_ok(),
            "isolated preference database should be removed"
        );
    }

    #[gpui_kit::test]
    fn valid_library_reload_clears_the_load_error(cx: &mut gpui_kit::TestAppContext) {
        cx.update(gpui_kit::component::init);
        cx.update(|app| {
            assert!(init_monitor_preset_library(Some("{"), app).is_err());
            assert!(monitor_preset_library_load_error(app).is_some());
            assert!(
                init_monitor_preset_library(Some(r#"{"version":1,"presets":{}}"#), app).is_ok()
            );
            assert!(monitor_preset_library_load_error(app).is_none());
        });
    }
}
