use gpui_kit::{Context, Window};
use ramag_domain::entities::SshProfileId;

use super::super::SshView;
use super::super::port_forward::PortForwardState;

impl SshView {
    pub(crate) fn start_port_forwarding(
        &mut self,
        profile_id: SshProfileId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .workspaces
            .iter()
            .find(|workspace| workspace.profile_id() == &profile_id)
            .is_some_and(|workspace| !workspace.profile.port_forwardings.is_empty())
        {
            self.notice = Some(super::super::model::Notice::info(
                "当前 SSH 配置没有端口转发",
            ));
            cx.notify();
            return;
        }
        if !self.port_forward_manager.begin_start(&profile_id) {
            return;
        }
        let service = self.service.clone();
        let profile_id_for_task = profile_id.clone();
        cx.spawn_in(window, async move |this, async_cx| {
            let result = service.port_forward_command(&profile_id_for_task).await;
            let _ = this.update_in(async_cx, |this, _window, cx| {
                if this.port_forward_manager.state(&profile_id_for_task)
                    != PortForwardState::Starting
                {
                    return;
                }
                match result {
                    Ok(command) if service.terminal_launch_is_current(&command) => {
                        if let Err(error) = this.port_forward_manager.start_process(command) {
                            this.port_forward_manager
                                .fail_start(&profile_id_for_task, error.clone());
                            this.notice = Some(super::super::model::Notice::error(error));
                        }
                    }
                    Ok(_) => this
                        .port_forward_manager
                        .fail_start(&profile_id_for_task, "端口转发启动已取消".into()),
                    Err(error) => this
                        .port_forward_manager
                        .fail_start(&profile_id_for_task, error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn stop_port_forwarding(
        &mut self,
        profile_id: &SshProfileId,
        cx: &mut Context<Self>,
    ) {
        if self.port_forward_manager.stop(profile_id) {
            cx.notify();
        }
    }

    pub(crate) fn refresh_port_forwarding_states(&mut self) -> bool {
        self.port_forward_manager.refresh()
    }
}
