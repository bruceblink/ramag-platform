//! 应用级系统设置与 GPUI 全局状态。

use gpui_kit::component::{Theme, scroll::ScrollbarMode};
use gpui_kit::{App, Global, px};
use serde::{Deserialize, Serialize};

/// 系统设置在本地偏好存储中的键名。
pub const SYSTEM_SETTINGS_PREF_KEY: &str = "system_settings";

/// 公共界面字号档位；有界枚举防止损坏配置把全部控件缩成不可操作的尺寸。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceTextSize {
    Compact,
    #[default]
    Standard,
    Large,
}

/// 应用级界面字体；只允许随应用发布的字体，避免配置引用不存在的系统字体。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceFont {
    #[default]
    Inter,
    IbmPlexSans,
}

impl InterfaceFont {
    pub const ALL: [Self; 2] = [Self::Inter, Self::IbmPlexSans];

    pub fn family(self) -> &'static str {
        match self {
            Self::Inter => "Inter Variable",
            Self::IbmPlexSans => "IBM Plex Sans",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Inter => "Inter",
            Self::IbmPlexSans => "IBM Plex Sans",
        }
    }
}

/// 应用级数字和指标字体；与界面字体分开保存，保证数据读数保持等宽可比较。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumericFont {
    #[default]
    JetbrainsMono,
    IbmPlexMono,
}

impl NumericFont {
    pub const ALL: [Self; 2] = [Self::JetbrainsMono, Self::IbmPlexMono];

    pub fn family(self) -> &'static str {
        match self {
            Self::JetbrainsMono => "JetBrains Mono",
            Self::IbmPlexMono => "IBM Plex Mono",
        }
    }

    pub fn label(self) -> &'static str {
        self.family()
    }
}

impl InterfaceTextSize {
    pub const ALL: [Self; 3] = [Self::Compact, Self::Standard, Self::Large];

    pub fn pixels(self) -> f32 {
        match self {
            Self::Compact => 14.0,
            Self::Standard => 16.0,
            Self::Large => 18.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => "紧凑",
            Self::Standard => "标准",
            Self::Large => "较大",
        }
    }
}

/// 所有工具共用的滚动条可见策略；默认常显以保证长内容的可发现性。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollbarVisibility {
    #[default]
    Always,
    Hover,
    Scrolling,
}

impl ScrollbarVisibility {
    pub const ALL: [Self; 3] = [Self::Always, Self::Hover, Self::Scrolling];

    pub fn label(self) -> &'static str {
        match self {
            Self::Always => "始终显示",
            Self::Hover => "悬停显示",
            Self::Scrolling => "滚动时显示",
        }
    }

    fn mode(self) -> ScrollbarMode {
        match self {
            Self::Always => ScrollbarMode::Always,
            Self::Hover => ScrollbarMode::Hover,
            Self::Scrolling => ScrollbarMode::Scrolling,
        }
    }
}

/// 应用级窗口与显示偏好，不包含连接、凭据或特定工具参数。
/// 缺失字段使用兼容默认值；非法字段由启动路径报告并恢复默认。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemSettings {
    /// Windows 上关闭主窗口后是否保留进程并由任务栏托盘重新打开。
    #[serde(default)]
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub text_size: InterfaceTextSize,
    #[serde(default)]
    pub scrollbar_visibility: ScrollbarVisibility,
    #[serde(default)]
    pub interface_font: InterfaceFont,
    #[serde(default)]
    pub numeric_font: NumericFont,
}

impl SystemSettings {
    /// 解析本地偏好；空值表示尚未保存，格式错误交给启动逻辑回退为默认值。
    pub fn parse(raw: &str) -> Result<Self, String> {
        if raw.is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_str(raw).map_err(|error| format!("系统设置格式无效：{error}"))
    }

    /// 将系统设置编码为可写入本地偏好存储的 JSON。
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|error| format!("序列化系统设置失败：{error}"))
    }
}

pub struct SystemSettingsGlobal(SystemSettings);
impl Global for SystemSettingsGlobal {}

/// 从 GPUI 全局状态读取系统设置；启动前未初始化时保持默认关闭。
pub fn system_settings(cx: &App) -> SystemSettings {
    cx.try_global::<SystemSettingsGlobal>()
        .map(|global| global.0)
        .unwrap_or_default()
}

/// 更新 GPUI 全局状态，供窗口生命周期和设置页面共享最新值。
pub fn set_system_settings(settings: SystemSettings, cx: &mut App) {
    cx.set_global(SystemSettingsGlobal(settings));
    apply_display_settings(cx);
    cx.refresh_windows();
}

/// 应用显示偏好，主题切换和启动初始化共用此路径，不写入持久存储。
pub(crate) fn apply_display_settings(cx: &mut App) {
    let settings = system_settings(cx);
    let theme = Theme::global_mut(cx);
    theme.font_size = px(settings.text_size.pixels());
    theme.font_family = settings.interface_font.family().into();
    theme.mono_font_family = settings.numeric_font.family().into();
    Theme::set_scrollbar_mode(settings.scrollbar_visibility.mode(), cx);
    // 字号和主题切换都要更新 Base 的副本，保持文字、滚动条与组件主题一致。
    Theme::sync_base(cx);
}

/// 更新公共设置并异步保存最后一次选择；设置页显示保存结果并允许重试失败值。
pub fn save_system_settings(settings: SystemSettings, cx: &mut App) {
    match settings.to_json() {
        Ok(json) => {
            set_system_settings(settings, cx);
            crate::preferences::persist_preference_latest(SYSTEM_SETTINGS_PREF_KEY, json, cx);
        }
        Err(error) => tracing::error!(
            operation = "system_settings_save",
            error,
            "serialize system settings failed"
        ),
    }
}

/// 初始化系统设置；损坏配置不会意外启用后台驻留。
pub fn init_system_settings(preference: Option<&str>, cx: &mut App) -> Result<(), String> {
    match preference.map(SystemSettings::parse).transpose() {
        Ok(settings) => {
            set_system_settings(settings.unwrap_or_default(), cx);
            Ok(())
        }
        Err(error) => {
            set_system_settings(SystemSettings::default(), cx);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_setting_keeps_tray_minimization_disabled() -> Result<(), String> {
        assert_eq!(SystemSettings::parse("{}")?, SystemSettings::default());
        assert!(!SystemSettings::default().minimize_to_tray);
        Ok(())
    }

    #[test]
    fn enabled_setting_round_trips() -> Result<(), String> {
        let settings = SystemSettings {
            minimize_to_tray: true,
            ..Default::default()
        };
        assert_eq!(SystemSettings::parse(&settings.to_json()?)?, settings);
        Ok(())
    }

    #[test]
    fn invalid_setting_is_rejected() {
        assert!(SystemSettings::parse(r#"{"minimize_to_tray":"yes"}"#).is_err());
    }

    #[test]
    fn legacy_settings_and_new_display_preferences_round_trip() -> Result<(), String> {
        let old = SystemSettings::parse(r#"{"minimize_to_tray":true}"#)?;
        assert!(old.minimize_to_tray);
        assert_eq!(old.scrollbar_visibility, ScrollbarVisibility::Always);
        assert_eq!(old.text_size, InterfaceTextSize::Standard);
        assert_eq!(old.interface_font, InterfaceFont::Inter);
        assert_eq!(old.numeric_font, NumericFont::JetbrainsMono);
        for text_size in InterfaceTextSize::ALL {
            for scrollbar_visibility in ScrollbarVisibility::ALL {
                for interface_font in InterfaceFont::ALL {
                    for numeric_font in NumericFont::ALL {
                        let next = SystemSettings {
                            text_size,
                            scrollbar_visibility,
                            interface_font,
                            numeric_font,
                            ..old
                        };
                        assert_eq!(SystemSettings::parse(&next.to_json()?)?, next);
                    }
                }
            }
        }
        assert!(SystemSettings::parse(r#"{"text_size":99999}"#).is_err());
        assert!(SystemSettings::parse(r#"{"scrollbar_visibility":"missing"}"#).is_err());
        Ok(())
    }

    /// 主题切换不能丢失用户显示偏好；损坏配置必须恢复可操作默认值。
    #[gpui_kit::test]
    fn display_preferences_survive_theme_changes(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            gpui_kit::component::init(cx);
            let settings = SystemSettings {
                text_size: InterfaceTextSize::Large,
                scrollbar_visibility: ScrollbarVisibility::Hover,
                ..Default::default()
            };
            set_system_settings(settings, cx);
            for mode in [crate::Mode::Dark, crate::Mode::Light, crate::Mode::Dark] {
                crate::apply_theme(mode, cx);
                let theme = Theme::global(cx);
                assert_eq!(theme.font_size, px(18.0));
                assert_eq!(theme.font_family.as_ref(), "Inter Variable");
                assert_eq!(theme.mono_font_family.as_ref(), "JetBrains Mono");
                assert_eq!(theme.scrollbar_mode, ScrollbarMode::Hover);
                assert_eq!(theme.tab_bar, theme.secondary);
                assert_eq!(theme.sidebar_foreground, theme.foreground);
                assert_eq!(theme.table_head, theme.secondary);
                assert_eq!(theme.ring, theme.accent);
            }
            assert!(init_system_settings(Some("broken"), cx).is_err());
            assert_eq!(system_settings(cx), SystemSettings::default());
            assert_eq!(Theme::global(cx).scrollbar_mode, ScrollbarMode::Always);
        });
    }

    /// 字体选择必须在主题切换中保持，并在损坏配置回退时恢复默认字体。
    #[gpui_kit::test]
    fn font_preferences_survive_theme_changes(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            gpui_kit::component::init(cx);
            set_system_settings(
                SystemSettings {
                    interface_font: InterfaceFont::IbmPlexSans,
                    numeric_font: NumericFont::IbmPlexMono,
                    ..Default::default()
                },
                cx,
            );
            for mode in [crate::Mode::Light, crate::Mode::Dark] {
                crate::apply_theme(mode, cx);
                let theme = Theme::global(cx);
                assert_eq!(theme.font_family.as_ref(), "IBM Plex Sans");
                assert_eq!(theme.mono_font_family.as_ref(), "IBM Plex Mono");
            }
            assert!(init_system_settings(Some(r#"{"interface_font":"missing"}"#), cx).is_err());
            assert_eq!(system_settings(cx), SystemSettings::default());
            assert_eq!(Theme::global(cx).font_family.as_ref(), "Inter Variable");
            assert_eq!(
                Theme::global(cx).mono_font_family.as_ref(),
                "JetBrains Mono"
            );
        });
    }
}
