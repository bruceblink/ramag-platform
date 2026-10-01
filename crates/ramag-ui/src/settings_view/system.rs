//! 公共显示与窗口偏好；所有入口读取同一 App Global，修改后立即刷新并保存。

use gpui_kit::component::{
    ActiveTheme, Sizable as _, Theme, button::ButtonVariants as _, h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, App, ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*,
};

use super::{SettingsView, pages::settings_card};
use crate::{
    InterfaceFont, InterfaceTextSize, NumericFont, ScrollbarVisibility, SystemSettings,
    theme::{Mode, current_mode, set_theme_preference},
};

impl SettingsView {
    pub(super) fn render_system_page(&self, cx: &mut Context<Self>) -> AnyElement {
        system_settings_panel(crate::system_settings(cx), current_mode(cx), cx.theme())
    }
}

#[cfg(test)]
#[path = "system_tests.rs"]
mod tests;

/// 渲染公共偏好；点击时读取最新全局值，避免其他窗口的修改被旧副本覆盖。
pub(super) fn system_settings_panel(
    settings: SystemSettings,
    mode: Mode,
    theme: &Theme,
) -> AnyElement {
    let themes = [(Mode::Light, "浅色"), (Mode::Dark, "深色")]
        .into_iter()
        .enumerate()
        .map(|(index, (choice, label))| {
            choice_button(
                format!("settings-theme-{index}"),
                label,
                mode == choice,
                move |_, _, cx| set_theme_preference(choice, cx),
            )
        })
        .collect();
    let sizes = InterfaceTextSize::ALL
        .into_iter()
        .enumerate()
        .map(|(index, choice)| {
            choice_button(
                format!("settings-text-size-{index}"),
                choice.label(),
                settings.text_size == choice,
                move |_, _, cx| {
                    let mut next = crate::system_settings(cx);
                    next.text_size = choice;
                    crate::save_system_settings(next, cx);
                },
            )
        })
        .collect();
    let scrollbars = ScrollbarVisibility::ALL
        .into_iter()
        .enumerate()
        .map(|(index, choice)| {
            choice_button(
                format!("settings-scrollbar-{index}"),
                choice.label(),
                settings.scrollbar_visibility == choice,
                move |_, _, cx| {
                    let mut next = crate::system_settings(cx);
                    next.scrollbar_visibility = choice;
                    crate::save_system_settings(next, cx);
                },
            )
        })
        .collect();
    let interface_fonts = InterfaceFont::ALL
        .into_iter()
        .enumerate()
        .map(|(index, choice)| {
            choice_button(
                format!("settings-interface-font-{index}"),
                choice.label(),
                settings.interface_font == choice,
                move |_, _, cx| {
                    let mut next = crate::system_settings(cx);
                    next.interface_font = choice;
                    crate::save_system_settings(next, cx);
                },
            )
        })
        .collect();
    let numeric_fonts = NumericFont::ALL
        .into_iter()
        .enumerate()
        .map(|(index, choice)| {
            choice_button(
                format!("settings-numeric-font-{index}"),
                choice.label(),
                settings.numeric_font == choice,
                move |_, _, cx| {
                    let mut next = crate::system_settings(cx);
                    next.numeric_font = choice;
                    crate::save_system_settings(next, cx);
                },
            )
        })
        .collect();
    let tray_description = if cfg!(target_os = "windows") {
        "关闭主窗口后应用继续运行，可从任务栏托盘重新打开。"
    } else {
        "当前平台尚未完成托盘驻留验证。"
    };
    v_flex()
        .w_full()
        .min_w_0()
        .gap_4()
        .child(
            settings_card("外观与交互", theme.border)
                .child(setting_row(
                    "settings-theme-row",
                    "主题",
                    "应用于全部工具和设置面板。",
                    themes,
                    theme,
                ))
                .child(setting_row(
                    "settings-text-size-row",
                    "界面字号",
                    "统一调整界面文字；代码和终端保留各自的等宽字体设置。",
                    sizes,
                    theme,
                ))
                .child(setting_row(
                    "settings-interface-font-row",
                    "界面字体",
                    "应用于页面标题、导航和普通文本。",
                    interface_fonts,
                    theme,
                ))
                .child(setting_row(
                    "settings-numeric-font-row",
                    "数值字体",
                    "应用于指标、单位和等宽数据，保持数字对齐。",
                    numeric_fonts,
                    theme,
                ))
                .child(setting_row(
                    "settings-scrollbar-row",
                    "滚动条",
                    "控制可滚动区域的显示方式，推荐始终显示。",
                    scrollbars,
                    theme,
                ))
                .child(
                    v_flex()
                        .id("settings-typography-preview")
                        .debug_selector(|| "settings-typography-preview".into())
                        .w_full()
                        .min_w_0()
                        .gap_2()
                        .p_3()
                        .border_l_2()
                        .border_color(theme.accent)
                        .bg(theme.background)
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("排版预览"),
                        )
                        .child(
                            div()
                                .font_family(theme.font_family.clone())
                                .child("The quick brown fox jumps over the lazy dog."),
                        )
                        .child(
                            div()
                                .font_family(theme.mono_font_family.clone())
                                .text_lg()
                                .text_color(theme.accent)
                                .child("0123456789 · 64.2 % · 8.5 GiB"),
                        ),
                ),
        )
        .child(settings_card("窗口行为", theme.border).child(setting_row(
            "settings-tray-row",
            "关闭时最小化到托盘",
            tray_description,
            vec![
                crate::clickable_switch("settings-system-minimize-to-tray")
                    .checked(settings.minimize_to_tray)
                    .on_click(|value, _, cx| {
                        let mut next = crate::system_settings(cx);
                        next.minimize_to_tray = *value;
                        crate::save_system_settings(next, cx);
                    }).into_any_element()
            ],
            theme,
        )))
        .into_any_element()
}

/// 设置行在窄窗口把选项换到下一行；说明和控件组均允许换行。
pub(super) fn setting_row(
    id: &'static str,
    title: &'static str,
    description: &'static str,
    controls: Vec<AnyElement>,
    theme: &Theme,
) -> impl IntoElement {
    h_flex()
        .id(id)
        .debug_selector(move || id.into())
        .w_full()
        .min_w_0()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(
            v_flex()
                .flex_1()
                .min_w(gpui_kit::px(160.0))
                .gap_1()
                .child(div().text_sm().child(title))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(description),
                ),
        )
        .child(
            h_flex()
                .min_w_0()
                .max_w_full()
                .flex_wrap()
                .gap_1()
                .children(controls),
        )
}

/// 呈现互斥选项，选中状态从真实配置读取，点击回调负责更新对应作用域。
pub(super) fn choice_button(
    id: String,
    label: &'static str,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let selector = id.clone();
    crate::clickable_button(SharedString::from(id))
        .debug_selector(move || selector.clone())
        .small()
        .label(label)
        .when(selected, |button| button.primary())
        .when(!selected, |button| button.outline())
        .on_click(on_click)
        .into_any_element()
}
