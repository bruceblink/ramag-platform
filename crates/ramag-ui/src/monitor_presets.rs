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

pub struct MonitorPresetLibraryGlobal(MonitorPresetLibrary);
impl Global for MonitorPresetLibraryGlobal {}

pub fn monitor_preset_library(cx: &App) -> MonitorPresetLibrary {
    cx.try_global::<MonitorPresetLibraryGlobal>()
        .map(|global| global.0.clone())
        .unwrap_or_default()
}

pub fn set_monitor_preset_library(library: MonitorPresetLibrary, cx: &mut App) {
    cx.set_global(MonitorPresetLibraryGlobal(library));
    cx.refresh_windows();
}

pub fn init_monitor_preset_library(preference: Option<&str>, cx: &mut App) -> Result<(), String> {
    match preference.map(MonitorPresetLibrary::parse).transpose() {
        Ok(library) => {
            set_monitor_preset_library(library.unwrap_or_default(), cx);
            Ok(())
        }
        Err(error) => {
            set_monitor_preset_library(MonitorPresetLibrary::default(), cx);
            Err(error)
        }
    }
}

/// Publishes a valid library immediately and lets the shared preference writer persist it.
pub fn save_monitor_preset_library(library: MonitorPresetLibrary, cx: &mut App) {
    match library.to_json() {
        Ok(json) => {
            set_monitor_preset_library(library, cx);
            crate::preferences::persist_preference_latest(MONITOR_PRESETS_PREF_KEY, json, cx);
        }
        Err(error) => tracing::error!(
            operation = "monitor_preset_library_save",
            error,
            "serialize monitor preset library failed"
        ),
    }
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
}
