//! Monitor preset CRUD controls for the fixed Ramag monitor workspace.

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _,
    button::ButtonVariants as _,
    h_flex,
    input::{Input, InputState},
    v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement, Render,
    SharedString, Styled, Window, div, px,
};

use super::pages::pulse_settings_card;

#[derive(Clone)]
enum Confirmation {
    Overwrite(String),
    Delete(String),
}

#[derive(Clone, Copy)]
enum BuiltinPreset {
    Default,
    Minimal,
    GpuFocus,
    Developer,
}

impl BuiltinPreset {
    const ALL: [Self; 4] = [
        Self::Default,
        Self::Minimal,
        Self::GpuFocus,
        Self::Developer,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Minimal => "Minimal",
            Self::GpuFocus => "GPU Focus",
            Self::Developer => "Developer",
        }
    }

    fn snapshot(self, current: crate::MonitorPresentationSettings) -> crate::MonitorPreset {
        let refresh_rate = match self {
            Self::Default | Self::GpuFocus => crate::MonitorRefreshRate::OneSecond,
            Self::Minimal => crate::MonitorRefreshRate::FiveSeconds,
            Self::Developer => crate::MonitorRefreshRate::HalfSecond,
        };
        let presentation = if matches!(self, Self::Default) {
            crate::MonitorPresentationSettings::default()
        } else {
            current
        };
        crate::MonitorPreset {
            monitor_settings: crate::MonitorSettings { refresh_rate },
            presentation,
        }
    }
}

/// Owns the editable name and confirmation state; saved preset data remains in the App Global.
pub(super) struct MonitorPresetManager {
    input: Entity<InputState>,
    confirmation: Option<Confirmation>,
    notice: Option<(String, bool)>,
}

impl MonitorPresetManager {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .validate(|value, _| value.len() <= 64)
                .placeholder("输入预设名称")
        });
        Self {
            input,
            confirmation: None,
            notice: None,
        }
    }

    fn input_name(&self, cx: &Context<Self>) -> String {
        self.input.read(cx).value().trim().to_owned()
    }

    fn clear_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input
            .update(cx, |state, cx| state.set_value("", window, cx));
    }

    fn save_library(
        &mut self,
        library: crate::MonitorPresetLibrary,
        message: String,
        cx: &mut Context<Self>,
    ) {
        match crate::save_monitor_preset_library(library, cx) {
            Ok(()) => self.notice = Some((message, false)),
            Err(error) => self.notice = Some((error, true)),
        }
        self.confirmation = None;
        cx.notify();
    }

    fn save_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.input_name(cx);
        if name.is_empty() {
            self.notice = Some(("请输入预设名称".into(), true));
            cx.notify();
            return;
        }
        let library = crate::monitor_preset_library(cx);
        if library.presets.contains_key(&name) {
            self.confirmation = Some(Confirmation::Overwrite(name));
            cx.notify();
            return;
        }
        let mut next = library;
        let result = next.upsert(
            &name,
            crate::MonitorPreset {
                monitor_settings: crate::monitor_settings(cx),
                presentation: crate::monitor_presentation_settings(cx),
            },
        );
        match result {
            Ok(()) => {
                self.save_library(next, format!("已保存预设：{name}"), cx);
                self.clear_input(window, cx);
            }
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
            }
        }
    }

    fn overwrite(&mut self, name: &str, cx: &mut Context<Self>) {
        let mut next = crate::monitor_preset_library(cx);
        match next.upsert(
            name,
            crate::MonitorPreset {
                monitor_settings: crate::monitor_settings(cx),
                presentation: crate::monitor_presentation_settings(cx),
            },
        ) {
            Ok(()) => self.save_library(next, format!("已覆盖预设：{name}"), cx),
            Err(error) => {
                self.notice = Some((error, true));
                self.confirmation = None;
                cx.notify();
            }
        }
    }

    fn apply(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(preset) = crate::monitor_preset_library(cx).presets.get(name).cloned() else {
            self.notice = Some(("预设不存在，未执行应用".into(), true));
            cx.notify();
            return;
        };
        // Preset application intentionally restores the application-wide cadence too.
        crate::save_monitor_settings(preset.monitor_settings, cx);
        crate::save_monitor_presentation_settings(preset.presentation, cx);
        self.notice = Some((format!("已应用预设：{name}"), false));
        cx.notify();
    }

    /// Applies a built-in reference preset without adding it to the persisted library.
    fn apply_builtin(&mut self, builtin: BuiltinPreset, cx: &mut Context<Self>) {
        let snapshot = builtin.snapshot(crate::monitor_presentation_settings(cx));
        crate::save_monitor_settings(snapshot.monitor_settings, cx);
        crate::save_monitor_presentation_settings(snapshot.presentation, cx);
        self.notice = Some((format!("已应用内置预设：{}", builtin.name()), false));
        cx.notify();
    }

    fn rename(&mut self, from: &str, window: &mut Window, cx: &mut Context<Self>) {
        let to = self.input_name(cx);
        let mut next = crate::monitor_preset_library(cx);
        match next.rename(from, &to) {
            Ok(()) => {
                self.save_library(next, format!("已重命名预设：{to}"), cx);
                self.clear_input(window, cx);
            }
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
            }
        }
    }

    fn delete(&mut self, name: &str, cx: &mut Context<Self>) {
        let mut next = crate::monitor_preset_library(cx);
        if !next.remove(name) {
            self.notice = Some(("预设不存在，未执行删除".into(), true));
            self.confirmation = None;
            cx.notify();
            return;
        }
        self.save_library(next, format!("已删除预设：{name}"), cx);
    }

    fn cancel_confirmation(&mut self, cx: &mut Context<Self>) {
        self.confirmation = None;
        cx.notify();
    }
}

impl Render for MonitorPresetManager {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let library = crate::monitor_preset_library(cx);
        let load_error = crate::monitor_preset_library_load_error(cx);
        let names = library.presets.keys().cloned().collect::<Vec<_>>();
        let mut rows = Vec::new();
        for (index, name) in names.iter().enumerate() {
            let apply_name = name.clone();
            let rename_name = name.clone();
            let overwrite_name = name.clone();
            let delete_name = name.clone();
            rows.push(
                h_flex()
                    .id(SharedString::from(format!("monitor-preset-row-{index}")))
                    .debug_selector(move || format!("monitor-preset-row-{index}"))
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.0))
                    .p(px(8.0))
                    .bg(theme.background)
                    .child(div().flex_1().min_w_0().child(name.clone()))
                    .child(
                        crate::clickable_button(format!("monitor-preset-apply-{index}"))
                            .debug_selector(move || format!("monitor-preset-apply-{index}"))
                            .small()
                            .label("应用")
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.apply(&apply_name, cx)),
                            ),
                    )
                    .child(
                        crate::clickable_button(format!("monitor-preset-rename-{index}"))
                            .debug_selector(move || format!("monitor-preset-rename-{index}"))
                            .small()
                            .label("重命名")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.rename(&rename_name, window, cx)
                            })),
                    )
                    .child(
                        crate::clickable_button(format!("monitor-preset-overwrite-{index}"))
                            .debug_selector(move || format!("monitor-preset-overwrite-{index}"))
                            .small()
                            .label("覆盖")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.confirmation =
                                    Some(Confirmation::Overwrite(overwrite_name.clone()));
                                cx.notify();
                            })),
                    )
                    .child(
                        crate::clickable_button(format!("monitor-preset-delete-{index}"))
                            .debug_selector(move || format!("monitor-preset-delete-{index}"))
                            .small()
                            .label("删除")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.confirmation = Some(Confirmation::Delete(delete_name.clone()));
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
            );
        }

        let mut card = pulse_settings_card("Presets", theme)
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("保存采样、设备和传感器展示偏好；不包含连接、凭据或历史样本。"),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap(px(4.0))
                    .child(div().text_sm().child("内置预设"))
                    .child(h_flex().w_full().flex_wrap().gap(px(6.0)).children(
                        BuiltinPreset::ALL.into_iter().map(|builtin| {
                            let name = builtin.name();
                            crate::clickable_button(format!("monitor-preset-builtin-{name}"))
                                .debug_selector(move || format!("monitor-preset-builtin-{name}"))
                                .small()
                                .label(name)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.apply_builtin(builtin, cx)
                                }))
                                .into_any_element()
                        }),
                    ))
                    .child(div().text_xs().text_color(theme.muted_foreground).child(
                        "内置预设只调整 Ramag 可迁移的采样和展示偏好；固定页面结构保持不变。",
                    )),
            );

        if let Some(error) = load_error {
            card = card.child(
                div()
                    .id("monitor-preset-load-error")
                    .debug_selector(|| "monitor-preset-load-error".into())
                    .text_xs()
                    .text_color(theme.danger)
                    .child(format!(
                        "本地预设库无法读取，原数据已保留；命名预设编辑已停用。修复或移除 monitor_presets 偏好后重启。原因：{error}"
                    )),
            );
        } else {
            card = card
                .child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .flex_wrap()
                        .gap(px(8.0))
                        .child(Input::new(&self.input).flex_1().min_w(px(180.0)).small())
                        .child(
                            crate::clickable_button("monitor-preset-save")
                                .debug_selector(|| "monitor-preset-save".into())
                                .small()
                                .primary()
                                .label("保存当前监控状态")
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.save_current(window, cx)
                                    }),
                                ),
                        ),
                )
                .child(v_flex().w_full().gap(px(4.0)).children(rows));

            if names.is_empty() {
                card = card.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("暂无命名预设"),
                );
            }
        }

        if let Some((message, error)) = &self.notice {
            card = card.child(
                div()
                    .id("monitor-preset-notice")
                    .debug_selector(|| "monitor-preset-notice".into())
                    .text_xs()
                    .text_color(if *error { theme.danger } else { theme.success })
                    .child(message.clone()),
            );
        }

        if let Some(confirmation) = self.confirmation.clone() {
            let (message, action) = match confirmation {
                Confirmation::Overwrite(name) => (
                    format!("覆盖“{name}”的当前监控状态？"),
                    crate::clickable_button("monitor-preset-confirm-overwrite")
                        .debug_selector(|| "monitor-preset-confirm-overwrite".into())
                        .small()
                        .primary()
                        .label("确认覆盖")
                        .on_click(cx.listener(move |this, _, _, cx| this.overwrite(&name, cx)))
                        .into_any_element(),
                ),
                Confirmation::Delete(name) => (
                    format!("删除预设“{name}”？当前监控状态不会改变。"),
                    crate::clickable_button("monitor-preset-confirm-delete")
                        .debug_selector(|| "monitor-preset-confirm-delete".into())
                        .small()
                        .danger()
                        .label("确认删除")
                        .on_click(cx.listener(move |this, _, _, cx| this.delete(&name, cx)))
                        .into_any_element(),
                ),
            };
            card = card.child(
                v_flex()
                    .id("monitor-preset-confirmation")
                    .debug_selector(|| "monitor-preset-confirmation".into())
                    .gap(px(8.0))
                    .p(px(10.0))
                    .border_1()
                    .border_color(theme.border)
                    .child(div().text_sm().child(message))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(
                                crate::clickable_button("monitor-preset-cancel")
                                    .debug_selector(|| "monitor-preset-cancel".into())
                                    .small()
                                    .ghost()
                                    .label("取消")
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.cancel_confirmation(cx)),
                                    ),
                            )
                            .child(action),
                    ),
            );
        }

        card.into_any_element()
    }
}

#[cfg(test)]
mod recovery_tests;

#[cfg(test)]
mod workflow_tests;
