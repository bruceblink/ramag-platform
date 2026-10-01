//! 系统监控工具偏好；独立存储，只影响采样频率，不影响应用主题或网络工具。

use std::collections::{BTreeMap, BTreeSet};

use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

pub const MONITOR_SETTINGS_PREF_KEY: &str = "monitor_settings";
pub const MONITOR_PRESENTATION_SETTINGS_PREF_KEY: &str = "monitor_presentation_settings";
const MAX_PRESENTATION_SETTINGS_BYTES: usize = 64 * 1024;
const MAX_PRESENTATION_ENTRIES: usize = 512;
const MAX_PRESENTATION_ID_BYTES: usize = 256;

/// 有界刷新档位。枚举拒绝零周期和过高采样频率，避免意外占用 CPU。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorRefreshRate {
    #[default]
    OneSecond,
    TwoSeconds,
    FiveSeconds,
}

impl MonitorRefreshRate {
    pub const ALL: [Self; 3] = [Self::OneSecond, Self::TwoSeconds, Self::FiveSeconds];
    pub fn label(self) -> &'static str {
        match self {
            Self::OneSecond => "1 秒",
            Self::TwoSeconds => "2 秒",
            Self::FiveSeconds => "5 秒",
        }
    }
}

/// 监控工具的持久偏好；不包含指标样本、进程信息或其他敏感数据。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorSettings {
    #[serde(default)]
    pub refresh_rate: MonitorRefreshRate,
}

/// 监控页面的展示偏好与刷新设置分开持久化，保持旧 `MonitorSettings` 字面量兼容。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorPresentationSettings {
    #[serde(default)]
    pub selected_devices: BTreeMap<String, String>,
    #[serde(default)]
    pub hidden_sensors: BTreeSet<String>,
}

impl MonitorPresentationSettings {
    /// 丢弃超长或过量的持久化 ID，限制用户偏好占用内存并保留稳定排序。
    pub fn bounded(mut self) -> Self {
        self.selected_devices.retain(|sensor, device| {
            valid_presentation_id(sensor) && valid_presentation_id(device)
        });
        self.hidden_sensors
            .retain(|sensor| valid_presentation_id(sensor));
        while self.selected_devices.len() > MAX_PRESENTATION_ENTRIES {
            if let Some(key) = self.selected_devices.keys().next_back().cloned() {
                self.selected_devices.remove(&key);
            }
        }
        while self.hidden_sensors.len() > MAX_PRESENTATION_ENTRIES {
            if let Some(key) = self.hidden_sensors.iter().next_back().cloned() {
                self.hidden_sensors.remove(&key);
            }
        }
        self
    }
}

fn valid_presentation_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_PRESENTATION_ID_BYTES
}

pub struct MonitorSettingsGlobal(MonitorSettings);
impl Global for MonitorSettingsGlobal {}

pub struct MonitorPresentationSettingsGlobal(MonitorPresentationSettings);
impl Global for MonitorPresentationSettingsGlobal {}

/// 首次使用或测试未初始化时使用兼容的 1 秒周期。
pub fn monitor_settings(cx: &App) -> MonitorSettings {
    cx.try_global::<MonitorSettingsGlobal>()
        .map(|global| global.0)
        .unwrap_or_default()
}

/// 返回传感器显隐和设备选择偏好；未初始化时保持所有传感器可见。
pub fn monitor_presentation_settings(cx: &App) -> MonitorPresentationSettings {
    cx.try_global::<MonitorPresentationSettingsGlobal>()
        .map(|global| global.0.clone())
        .unwrap_or_default()
}

/// 更新展示偏好并通知已打开的监控视图。
pub fn set_monitor_presentation_settings(settings: MonitorPresentationSettings, cx: &mut App) {
    cx.set_global(MonitorPresentationSettingsGlobal(settings.bounded()));
    cx.refresh_windows();
}

/// 读取展示偏好独立键；损坏或过量配置回退为空设置。
pub fn init_monitor_presentation_settings(
    preference: Option<&str>,
    cx: &mut App,
) -> Result<(), String> {
    let parsed = preference
        .filter(|raw| !raw.is_empty())
        .map(|raw| {
            if raw.len() > MAX_PRESENTATION_SETTINGS_BYTES {
                return Err("monitor presentation settings exceed the size limit".to_owned());
            }
            serde_json::from_str::<MonitorPresentationSettings>(raw)
                .map_err(|error| error.to_string())
        })
        .transpose();
    match parsed {
        Ok(settings) => {
            set_monitor_presentation_settings(settings.unwrap_or_default(), cx);
            Ok(())
        }
        Err(error) => {
            set_monitor_presentation_settings(MonitorPresentationSettings::default(), cx);
            Err(format!("系统监控展示设置格式无效：{error}"))
        }
    }
}

/// 保存有界展示偏好；写入失败由统一偏好存储记录。
pub fn save_monitor_presentation_settings(settings: MonitorPresentationSettings, cx: &mut App) {
    let settings = settings.bounded();
    match serde_json::to_string(&settings) {
        Ok(json) => {
            set_monitor_presentation_settings(settings, cx);
            crate::preferences::persist_preference_latest(
                MONITOR_PRESENTATION_SETTINGS_PREF_KEY,
                json,
                cx,
            );
        }
        Err(error) => {
            tracing::error!(operation = "monitor_presentation_settings_save", error = %error, "serialize monitor presentation settings failed")
        }
    }
}

/// 仅修改运行状态；观察者负责把新频率同步到采样器。
pub fn set_monitor_settings(settings: MonitorSettings, cx: &mut App) {
    cx.set_global(MonitorSettingsGlobal(settings));
    cx.refresh_windows();
}

/// 读取独立存储键，损坏配置报错并回退，不修改其他工具设置。
pub fn init_monitor_settings(preference: Option<&str>, cx: &mut App) -> Result<(), String> {
    let parsed = preference
        .filter(|raw| !raw.is_empty())
        .map(serde_json::from_str::<MonitorSettings>)
        .transpose();
    match parsed {
        Ok(settings) => {
            set_monitor_settings(settings.unwrap_or_default(), cx);
            Ok(())
        }
        Err(error) => {
            set_monitor_settings(MonitorSettings::default(), cx);
            Err(format!("系统监控设置格式无效：{error}"))
        }
    }
}

/// 保存最新选择并通知已打开的工具；写入失败由统一偏好存储记录。
pub fn save_monitor_settings(settings: MonitorSettings, cx: &mut App) {
    match serde_json::to_string(&settings) {
        Ok(json) => {
            set_monitor_settings(settings, cx);
            crate::preferences::persist_preference_latest(MONITOR_SETTINGS_PREF_KEY, json, cx);
        }
        Err(error) => {
            tracing::error!(operation = "monitor_settings_save", error = %error, "serialize monitor settings failed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_refresh_settings_shape_stays_compatible() {
        let parsed = serde_json::from_str::<MonitorSettings>(r#"{"refresh_rate":"five_seconds"}"#);
        assert!(parsed.is_ok(), "legacy refresh preference should parse");
        let Ok(settings) = parsed else { return };
        assert_eq!(settings.refresh_rate, MonitorRefreshRate::FiveSeconds);
        let serialized = serde_json::to_string(&settings);
        assert_eq!(
            serialized.ok().as_deref(),
            Some(r#"{"refresh_rate":"five_seconds"}"#)
        );
    }

    #[test]
    fn presentation_preferences_keep_only_bounded_stable_ids() {
        let settings = MonitorPresentationSettings {
            selected_devices: BTreeMap::from([
                ("cpu.load".to_owned(), "host-0".to_owned()),
                (
                    "x".repeat(MAX_PRESENTATION_ID_BYTES + 1),
                    "host-1".to_owned(),
                ),
            ]),
            hidden_sensors: BTreeSet::from(["gpu.temp".to_owned(), String::new()]),
        }
        .bounded();
        assert_eq!(settings.selected_devices.len(), 1);
        assert!(settings.hidden_sensors.contains("gpu.temp"));
        assert_eq!(settings.hidden_sensors.len(), 1);
    }
}
