//! Root render composition and the explicit force-quit confirmation surface.

use gpui_kit::component::{
    ActiveTheme as _, FocusTrapElement as _, Sizable as _, button::ButtonVariants as _, h_flex,
    scroll::ScrollableElement as _, v_flex,
};
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement, Render, Styled,
    Window, div, px,
};

use super::SystemView;

impl SystemView {
    fn render_termination_confirmation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(request) = self.termination_request.as_ref() else {
            return div().into_any_element();
        };
        let title = "强制结束进程";
        let description = format!(
            "强制结束 {}（PID {}）？确认时会重新核对进程启动身份。",
            request.name, request.identity.pid
        );
        let mut shade = gpui_kit::black();
        shade.a = 0.58;
        let theme = cx.theme();
        div()
            .id("system-termination-overlay")
            .debug_selector(|| "system-termination-overlay".into())
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .p(px(12.0))
            .bg(shade)
            .occlude()
            .track_focus(&self.termination_focus)
            .key_context("SystemTerminationConfirmation")
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key.eq_ignore_ascii_case("escape") {
                    this.cancel_termination(cx);
                } else {
                    cx.stop_propagation();
                }
            }))
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation()
            })
            .child(
                v_flex()
                    .id("system-termination-card")
                    .debug_selector(|| "system-termination-card".into())
                    .w_full()
                    .max_w(px(420.0))
                    .max_h_full()
                    .gap(px(12.0))
                    .p(px(16.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.background)
                    .text_color(theme.foreground)
                    .child(div().text_lg().child(title))
                    .child(
                        div()
                            .min_w_0()
                            .text_sm()
                            .whitespace_normal()
                            .child(description),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .justify_end()
                            .gap(px(8.0))
                            .child(
                                ramag_ui::clickable_button("system-kill-cancel")
                                    .debug_selector(|| "system-kill-cancel".into())
                                    .ghost()
                                    .small()
                                    .label("取消")
                                    .on_key_down(cx.listener(
                                        |this, event: &KeyDownEvent, _, cx| {
                                            if matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            ) {
                                                this.cancel_termination(cx);
                                                cx.stop_propagation();
                                            }
                                        },
                                    ))
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.cancel_termination(cx)),
                                    ),
                            )
                            .child(
                                ramag_ui::clickable_button("system-kill-confirm")
                                    .debug_selector(|| "system-kill-confirm".into())
                                    .danger()
                                    .small()
                                    .label("强制结束")
                                    .on_key_down(cx.listener(
                                        |this, event: &KeyDownEvent, _, cx| {
                                            if matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            ) {
                                                this.confirm_termination(cx);
                                                cx.stop_propagation();
                                            }
                                        },
                                    ))
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.confirm_termination(cx)),
                                    ),
                            ),
                    ),
            )
            .focus_trap("system-termination-focus-trap", &self.termination_focus)
            .into_any_element()
    }

    fn render_notice(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        let notice = self.notice.as_ref()?;
        Some(ramag_ui::pulse_ui::pulse_status_notice(
            if notice.error {
                ramag_ui::pulse_ui::PulseStatus::Failed
            } else {
                ramag_ui::pulse_ui::PulseStatus::Current
            },
            notice.message.clone(),
            cx,
        ))
    }
}

impl Render for SystemView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = v_flex()
            .debug_selector(|| "system-root".into())
            .size_full()
            .relative()
            .items_stretch()
            .min_w_0()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_header(window, cx));
        if let Some(notice) = self.render_notice(cx) {
            root = root.child(notice);
        }
        root = root.child(
            div()
                .id("system-content")
                .debug_selector(|| "system-content".into())
                .w_full()
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .child(self.render_page(window, cx)),
        );
        if self.termination_request.is_some() {
            if !self.termination_focus_requested {
                self.termination_focus_requested = true;
                window.focus(&self.termination_focus, cx);
            }
            root = root.child(self.render_termination_confirmation(cx));
        }
        root
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
