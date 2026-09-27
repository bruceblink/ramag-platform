//! 系统监控工具偏好；独立存储，只影响采样频率，不影响应用主题或网络工具。

use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

pub const MONITOR_SETTINGS_PREF_KEY: &str = "monitor_settings";

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

pub struct MonitorSettingsGlobal(MonitorSettings);
impl Global for MonitorSettingsGlobal {}

/// 首次使用或测试未初始化时使用兼容的 1 秒周期。
pub fn monitor_settings(cx: &App) -> MonitorSettings {
    cx.try_global::<MonitorSettingsGlobal>()
        .map(|global| global.0)
        .unwrap_or_default()
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
