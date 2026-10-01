//! Stable, responsive navigation for the ten System Pulse monitor pages.

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, button::ButtonVariants as _, h_flex, v_flex,
};
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement, Styled, Window, div, px,
};

use super::{SystemSection, SystemView};

impl SystemView {
    /// Places the application identity, stable page tabs, and refresh action above content.
    pub(super) fn render_header(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tabs = SystemSection::ALL.map(|section| ramag_ui::pulse_ui::PulseTab {
            id: section.id().into(),
            title: section.title().into(),
        });
        let view = cx.entity().clone();
        let navigation = ramag_ui::pulse_ui::pulse_tabs(
            &tabs,
            self.section.id(),
            window,
            cx,
            move |selected, _, app| {
                if let Some(section) = SystemSection::ALL
                    .into_iter()
                    .find(|candidate| candidate.id() == selected.as_ref())
                {
                    view.update(app, |this, cx| this.select_section(section, cx));
                }
            },
        );
        let theme = cx.theme();
        v_flex()
            .debug_selector(|| "system-header".into())
            .w_full()
            .flex_none()
            .gap(px(10.0))
            .px(px(18.0))
            .pt(px(14.0))
            .pb(px(10.0))
            .border_b_1()
            .border_color(theme.border.opacity(0.75))
            .bg(theme.background)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .child("System Monitor"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("本机性能与设备状态"),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!(
                                        "刷新 {}",
                                        self.monitor.refresh_interval().label()
                                    )),
                            )
                            .child(
                                ramag_ui::clickable_button("system-open-settings")
                                    .debug_selector(|| "system-open-settings".into())
                                    .ghost()
                                    .small()
                                    .icon(ramag_ui::icons::settings())
                                    .tooltip("全局系统工具设置")
                                    .on_click(|_, window, cx| {
                                        window.dispatch_action(
                                            Box::new(ramag_ui::actions::OpenToolSettings {
                                                tool_id: "system".into(),
                                            }),
                                            cx,
                                        )
                                    }),
                            )
                            .child(
                                ramag_ui::clickable_button("system-refresh")
                                    .debug_selector(|| "system-refresh".into())
                                    .ghost()
                                    .small()
                                    .icon(ramag_ui::icons::refresh_cw())
                                    .tooltip("立即采样")
                                    .on_click(cx.listener(|this, _, _, cx| this.refresh_now(cx))),
                            ),
                    ),
            )
            .child(navigation)
    }
}
