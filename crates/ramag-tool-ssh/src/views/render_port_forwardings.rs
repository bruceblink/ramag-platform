//! SSH 配置中的端口转发摘要。

use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::{Context, IntoElement, ParentElement, Styled, div, prelude::*, px};
use ramag_domain::entities::SshPortForwardDirection;

use super::profile_dialog::SshProfileFormPanel;

impl SshProfileFormPanel {
    pub(super) fn render_port_forwardings(
        &self,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let summary = self
            .port_forwardings
            .iter()
            .filter_map(|forwarding| {
                let option = match forwarding.direction() {
                    SshPortForwardDirection::Local => "-L",
                    SshPortForwardDirection::Remote => "-R",
                    SshPortForwardDirection::Dynamic => "-D",
                };
                forwarding
                    .open_ssh_argument()
                    .ok()
                    .map(|argument| format!("{option} {argument}"))
            })
            .collect::<Vec<_>>()
            .join(" · ");

        v_flex()
            .id("ssh-profile-port-forwardings")
            .debug_selector(|| "ssh-profile-port-forwardings".into())
            .w_full()
            .min_w_0()
            .gap(px(6.0))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(8.0))
                    .when(compact, |row| row.flex_wrap())
                    .child(div().text_sm().child("端口转发"))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} 条", self.port_forwardings.len())),
                    ),
            )
            .child(
                div()
                    .id("ssh-profile-port-forwardings-summary")
                    .debug_selector(|| "ssh-profile-port-forwardings-summary".into())
                    .w_full()
                    .min_w_0()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(if summary.is_empty() {
                        "未配置（可在 SSH 命令中使用 -L、-R 或 -D 后点击解析）".into()
                    } else {
                        summary
                    }),
            )
    }
}
