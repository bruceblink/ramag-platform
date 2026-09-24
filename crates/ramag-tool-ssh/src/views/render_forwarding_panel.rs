use gpui_kit::component::{ActiveTheme, Disableable as _, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::{ClickEvent, Context, IntoElement, ParentElement, Styled, div, prelude::*, px};
use ramag_domain::entities::{SshPortForward, SshPortForwardDirection, SshProfileId};

use super::SshView;
use super::port_forward::PortForwardState;

impl SshView {
    pub(super) fn render_port_forwarding_panel(
        &self,
        workspace_id: SshProfileId,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(workspace) = self
            .workspaces
            .iter()
            .find(|workspace| workspace.profile_id() == &workspace_id)
        else {
            return div().into_any_element();
        };
        if workspace.profile.port_forwardings.is_empty() {
            return div().into_any_element();
        }
        let state = self.port_forward_manager.state(&workspace_id);
        let (status, status_color) = forwarding_state_text(&state, cx);
        let link = cx.theme().link;
        let warning = cx.theme().warning;
        let accent = cx.theme().accent;
        let muted = cx.theme().muted_foreground;
        let can_start = matches!(
            state,
            PortForwardState::Stopped | PortForwardState::Failed(_)
        );
        let can_stop = matches!(
            state,
            PortForwardState::Starting | PortForwardState::Running
        );
        let action_id = if can_start {
            "start-ssh-port-forwarding"
        } else {
            "stop-ssh-port-forwarding"
        };
        let workspace_for_action = workspace_id.clone();
        let action = if can_start {
            ramag_ui::clickable_button("start-ssh-port-forwarding")
                .small()
                .icon(IconName::Play)
                .label("启动转发")
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.start_port_forwarding(workspace_for_action.clone(), window, cx);
                }))
                .into_any_element()
        } else {
            let workspace_for_stop = workspace_id.clone();
            ramag_ui::clickable_button("stop-ssh-port-forwarding")
                .small()
                .icon(IconName::Close)
                .label("停止转发")
                .disabled(!can_stop)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.stop_port_forwarding(&workspace_for_stop, cx);
                }))
                .into_any_element()
        };
        let rows = workspace
            .profile
            .port_forwardings
            .iter()
            .enumerate()
            .map(|(index, forwarding)| {
                render_forwarding_row(index, forwarding, link, warning, accent, muted)
            })
            .collect::<Vec<_>>();
        let error = match state {
            PortForwardState::Failed(message) => Some(message),
            _ => None,
        };
        v_flex()
            .id("ssh-port-forwarding-panel")
            .debug_selector(|| "ssh-port-forwarding-panel".into())
            .w_full()
            .flex_none()
            .gap(px(6.0))
            .px(px(8.0))
            .py(px(6.0))
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().text_xs().child("端口转发"))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(status_color)
                            .child(status),
                    )
                    .child(
                        div()
                            .id(action_id)
                            .debug_selector(move || action_id.into())
                            .child(action),
                    ),
            )
            .child(
                v_flex()
                    .id("ssh-port-forwarding-list")
                    .debug_selector(|| "ssh-port-forwarding-list".into())
                    .w_full()
                    .min_w_0()
                    .gap(px(3.0))
                    .children(rows),
            )
            .when_some(error, |panel, message| {
                panel.child(
                    div()
                        .id("ssh-port-forwarding-error")
                        .debug_selector(|| "ssh-port-forwarding-error".into())
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(format!("失败：{message}")),
                )
            })
            .into_any_element()
    }
}

fn render_forwarding_row(
    index: usize,
    forwarding: &SshPortForward,
    link: gpui_kit::Hsla,
    warning: gpui_kit::Hsla,
    accent: gpui_kit::Hsla,
    muted: gpui_kit::Hsla,
) -> impl IntoElement {
    let (direction, direction_color) = match forwarding.direction() {
        SshPortForwardDirection::Local => ("-L", link),
        SshPortForwardDirection::Remote => ("-R", warning),
        SshPortForwardDirection::Dynamic => ("-D", accent),
    };
    let argument = forwarding
        .open_ssh_argument()
        .unwrap_or_else(|error| format!("无效配置：{error}"));
    h_flex()
        .id(("ssh-port-forwarding-row", index))
        .debug_selector(move || format!("ssh-port-forwarding-row-{index}"))
        .w_full()
        .min_w_0()
        .flex_wrap()
        .gap(px(8.0))
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(direction_color)
                .child(direction),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_xs()
                .text_color(muted)
                .overflow_hidden()
                .text_ellipsis()
                .child(argument),
        )
}

fn forwarding_state_text(
    state: &PortForwardState,
    cx: &Context<SshView>,
) -> (String, gpui_kit::Hsla) {
    match state {
        PortForwardState::Stopped => ("已停止".into(), cx.theme().muted_foreground),
        PortForwardState::Starting => ("启动中".into(), cx.theme().warning),
        PortForwardState::Running => ("运行中".into(), cx.theme().success),
        PortForwardState::Stopping => ("停止中".into(), cx.theme().warning),
        PortForwardState::Failed(_) => ("失败".into(), cx.theme().danger),
    }
}
