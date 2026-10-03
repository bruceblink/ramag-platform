//! Redirects monitor settings to Ramag's single application settings surface.

use gpui_kit::base::Button;
use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    ScrollHandle, Styled, Window, div,
};

pub(crate) struct SettingsPanel {
    shared: crate::workspace::Shared,
    scroll: ScrollHandle,
    pub(crate) presets: Option<Entity<crate::workspace::presets::PresetManager>>,
}

impl SettingsPanel {
    pub(crate) fn new(shared: crate::workspace::Shared, _: &mut gpui_kit::App) -> Self {
        Self {
            shared,
            scroll: ScrollHandle::default(),
            presets: None,
        }
    }

    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }
}

impl Render for SettingsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.presets.is_none() {
            self.presets = Some(cx.new(|_| {
                crate::workspace::presets::PresetManager::new(
                    self.shared.clone(),
                    self.scroll.clone(),
                )
            }));
        }
        let theme = cx.theme();
        div()
            .id("system-monitor-settings")
            .debug_selector(|| "system-monitor-settings".into())
            .size_full()
            .flex()
            .flex_col()
            .items_start()
            .gap_3()
            .p_4()
            .text_color(theme.foreground)
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("外观和采样设置统一由 Ramag 系统设置管理。"),
            )
            .child(
                Button::new("open-ramag-settings")
                    .debug_selector(|| "open-ramag-settings".into())
                    .child("打开 Ramag 系统设置")
                    .on_click(|_, window, cx| {
                        window.dispatch_action(Box::new(ramag_ui::actions::OpenSystemSettings), cx);
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("主题、字体和刷新频率使用 Ramag 系统设置；此处预设保存监控页面布局。"),
            )
            .when_some(self.presets.clone(), |view, presets| view.child(presets))
    }
}
