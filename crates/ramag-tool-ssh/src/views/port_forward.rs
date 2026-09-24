//! SSH 独立端口转发进程的状态与回收。

use std::collections::HashMap;
use std::process::{Child, Command, Stdio};

use ramag_domain::entities::SshLaunchCommand;
use ramag_domain::entities::SshProfileId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PortForwardState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed(String),
}

struct PortForwardSession {
    state: PortForwardState,
    child: Option<Child>,
}

pub(super) struct PortForwardManager {
    sessions: HashMap<SshProfileId, PortForwardSession>,
}

impl PortForwardManager {
    pub(super) fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub(super) fn state(&self, profile_id: &SshProfileId) -> PortForwardState {
        self.sessions
            .get(profile_id)
            .map(|session| session.state.clone())
            .unwrap_or(PortForwardState::Stopped)
    }

    pub(super) fn begin_start(&mut self, profile_id: &SshProfileId) -> bool {
        let session =
            self.sessions
                .entry(profile_id.clone())
                .or_insert_with(|| PortForwardSession {
                    state: PortForwardState::Stopped,
                    child: None,
                });
        if matches!(
            session.state,
            PortForwardState::Starting | PortForwardState::Running | PortForwardState::Stopping
        ) {
            return false;
        }
        session.state = PortForwardState::Starting;
        session.child = None;
        true
    }

    pub(super) fn start_process(&mut self, command: SshLaunchCommand) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(&command.profile_id)
            .ok_or_else(|| "端口转发启动请求已失效".to_string())?;
        if session.state != PortForwardState::Starting {
            return Err("端口转发启动请求已取消".into());
        }
        let child = Command::new(&command.program)
            .args(&command.args)
            .envs(&command.env)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("启动端口转发失败：{error}"))?;
        session.child = Some(child);
        session.state = PortForwardState::Running;
        Ok(())
    }

    pub(super) fn fail_start(&mut self, profile_id: &SshProfileId, error: String) {
        let session =
            self.sessions
                .entry(profile_id.clone())
                .or_insert_with(|| PortForwardSession {
                    state: PortForwardState::Stopped,
                    child: None,
                });
        session.child = None;
        session.state = PortForwardState::Failed(error);
    }

    pub(super) fn stop(&mut self, profile_id: &SshProfileId) -> bool {
        let Some(session) = self.sessions.get_mut(profile_id) else {
            return false;
        };
        match session.state {
            PortForwardState::Starting => {
                session.state = PortForwardState::Stopped;
                true
            }
            PortForwardState::Running => {
                let Some(child) = session.child.as_mut() else {
                    session.state = PortForwardState::Stopped;
                    return true;
                };
                match child.kill() {
                    Ok(()) => {
                        session.state = PortForwardState::Stopping;
                        true
                    }
                    Err(error) => {
                        session.state =
                            PortForwardState::Failed(format!("停止端口转发失败：{error}"));
                        false
                    }
                }
            }
            PortForwardState::Stopping => true,
            PortForwardState::Stopped | PortForwardState::Failed(_) => false,
        }
    }

    pub(super) fn refresh(&mut self) -> bool {
        let mut changed = false;
        for session in self.sessions.values_mut() {
            let Some(child) = session.child.as_mut() else {
                continue;
            };
            let outcome = match child.try_wait() {
                Ok(None) => None,
                Ok(Some(status)) => Some(
                    if session.state == PortForwardState::Stopping || status.success() {
                        PortForwardState::Stopped
                    } else {
                        PortForwardState::Failed(format!(
                            "OpenSSH 端口转发已退出（状态码 {}）",
                            status
                                .code()
                                .map_or_else(|| "未知".into(), |code| code.to_string())
                        ))
                    },
                ),
                Err(error) => Some(PortForwardState::Failed(format!(
                    "读取端口转发状态失败：{error}"
                ))),
            };
            if let Some(state) = outcome {
                session.child.take();
                session.state = state;
                changed = true;
            }
        }
        changed
    }
}

impl Drop for PortForwardManager {
    fn drop(&mut self) {
        for session in self.sessions.values_mut() {
            if let Some(mut child) = session.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::SshProfileId;

    #[test]
    fn start_stop_state_is_bounded_and_idempotent() {
        let id = SshProfileId::new();
        let mut manager = PortForwardManager::new();

        assert_eq!(manager.state(&id), PortForwardState::Stopped);
        assert!(manager.begin_start(&id));
        assert_eq!(manager.state(&id), PortForwardState::Starting);
        assert!(!manager.begin_start(&id));
        assert!(manager.stop(&id));
        assert_eq!(manager.state(&id), PortForwardState::Stopped);
        assert!(!manager.stop(&id));
    }

    #[test]
    fn failed_start_is_visible_until_retried() {
        let id = SshProfileId::new();
        let mut manager = PortForwardManager::new();
        manager.fail_start(&id, "连接失败".into());
        assert_eq!(
            manager.state(&id),
            PortForwardState::Failed("连接失败".into())
        );
        assert!(manager.begin_start(&id));
        assert_eq!(manager.state(&id), PortForwardState::Starting);
    }

    #[cfg(unix)]
    #[test]
    fn running_process_is_killed_and_reaped() {
        let id = SshProfileId::new();
        let mut manager = PortForwardManager::new();
        assert!(manager.begin_start(&id));
        manager
            .start_process(SshLaunchCommand {
                profile_id: id.clone(),
                authorization_generation: 0,
                program: "/bin/sh".into(),
                args: vec!["-c".into(), "sleep 10".into()],
                env: Default::default(),
            })
            .unwrap();
        assert_eq!(manager.state(&id), PortForwardState::Running);
        assert!(manager.stop(&id));

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while manager.state(&id) != PortForwardState::Stopped
            && std::time::Instant::now() < deadline
        {
            manager.refresh();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(manager.state(&id), PortForwardState::Stopped);
    }
}
