use std::sync::Arc;

use gpui_kit::{Entity, SharedString};
use ramag_domain::entities::{
    RemoteEntry, SshProfile, SshProfileId, SshRemoteCapabilities, SshSessionState,
};
use ramag_terminal::{TerminalCore, TerminalExit, TerminalView};

pub(super) fn can_close_terminal(terminal_count: usize) -> bool {
    terminal_count > 1
}

pub(super) fn terminal_index_after_close(
    closed_index: usize,
    remaining_count: usize,
) -> Option<usize> {
    (remaining_count > 0).then(|| closed_index.saturating_sub(1).min(remaining_count - 1))
}

pub(super) fn terminal_has_exited(core: &TerminalCore) -> bool {
    core.is_closed() || core.exit_status().is_some()
}

pub(super) fn session_state_text(state: SshSessionState) -> &'static str {
    match state {
        SshSessionState::Disconnected => "未连接",
        SshSessionState::Connecting => "连接中",
        SshSessionState::Connected => "已连接",
        SshSessionState::Reconnecting => "重连中",
        SshSessionState::Exited => "已退出",
        SshSessionState::Failed => "连接失败",
    }
}

pub(super) fn session_pulse_status(state: SshSessionState) -> ramag_ui::pulse_ui::PulseStatus {
    match state {
        SshSessionState::Disconnected | SshSessionState::Exited => {
            ramag_ui::pulse_ui::PulseStatus::Unavailable
        }
        SshSessionState::Connecting | SshSessionState::Reconnecting => {
            ramag_ui::pulse_ui::PulseStatus::Warming
        }
        SshSessionState::Connected => ramag_ui::pulse_ui::PulseStatus::Current,
        SshSessionState::Failed => ramag_ui::pulse_ui::PulseStatus::Failed,
    }
}

pub(super) fn terminal_session_state(
    exit_status: Option<&TerminalExit>,
    finished: bool,
) -> SshSessionState {
    match exit_status {
        Some(status) if !status.success => SshSessionState::Failed,
        Some(_) => SshSessionState::Exited,
        None if finished => SshSessionState::Exited,
        None => SshSessionState::Connected,
    }
}

/// Pending launches retain their state; otherwise a live terminal takes
/// precedence and failed exits must not be reported as normal completion.
pub(super) fn workspace_session_state(
    current: SshSessionState,
    terminal_loading: bool,
    terminal_states: impl IntoIterator<Item = SshSessionState>,
) -> SshSessionState {
    if terminal_loading {
        return current;
    }
    let mut has_terminal = false;
    let mut has_failed_terminal = false;
    for state in terminal_states {
        has_terminal = true;
        if state == SshSessionState::Connected {
            return SshSessionState::Connected;
        }
        has_failed_terminal |= state == SshSessionState::Failed;
    }
    if has_failed_terminal {
        SshSessionState::Failed
    } else if has_terminal {
        SshSessionState::Exited
    } else {
        current
    }
}

pub(super) fn terminal_pulse_status(
    exit_status: Option<&TerminalExit>,
    finished: bool,
) -> (ramag_ui::pulse_ui::PulseStatus, &'static str) {
    use ramag_ui::pulse_ui::PulseStatus;

    match exit_status {
        Some(status) if status.success => (PulseStatus::Unavailable, "已完成"),
        Some(_) => (PulseStatus::Failed, "异常退出"),
        None if finished => (PulseStatus::Unavailable, "已关闭"),
        None => (PulseStatus::Current, "运行中"),
    }
}

pub(super) fn terminal_tab_label(
    label: &str,
    exit_status: Option<&TerminalExit>,
    finished: bool,
) -> String {
    match exit_status {
        Some(status) => format!(
            "{label} [退出{}]",
            status
                .code
                .map_or_else(String::new, |code| format!(": {code}"))
        ),
        None if finished => format!("{label} [已关闭]"),
        None => label.to_owned(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ViewMode {
    Manager,
    Workspace,
}

pub(super) struct Notice {
    pub message: String,
    pub error: bool,
}

impl Notice {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error: false,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error: true,
        }
    }
}

pub(super) struct TerminalTab {
    pub id: u64,
    pub label: SharedString,
    pub view: Entity<TerminalView>,
}

pub(super) struct SshWorkspace {
    pub profile: SshProfile,
    pub path: String,
    pub directory_query: String,
    pub entries: Arc<Vec<RemoteEntry>>,
    pub selected_path: Option<String>,
    pub terminals: Vec<TerminalTab>,
    pub active_terminal_id: Option<u64>,
    pub terminal_loading: bool,
    pub connection_started: bool,
    pub session_state: SshSessionState,
    pub sftp_loading: bool,
    pub directory_loaded: bool,
    pub directory_loading_path: Option<String>,
    pub sftp_error: Option<String>,
    pub operation_busy: bool,
    pub file_preview_loading: bool,
    pub transfers_visible: bool,
    pub next_terminal_ordinal: u64,
    pub directory_generation: u64,
    pub file_preview_generation: u64,
    pub terminal_generation: u64,
    pub capabilities: Option<SshRemoteCapabilities>,
    pub capability_error: Option<String>,
    pub capability_loading: bool,
    pub capability_generation: u64,
}

impl SshWorkspace {
    pub fn placeholder(profile: SshProfile, path: String) -> Self {
        Self {
            profile,
            path,
            directory_query: String::new(),
            entries: Arc::new(Vec::new()),
            selected_path: None,
            terminals: Vec::new(),
            active_terminal_id: None,
            terminal_loading: false,
            connection_started: false,
            session_state: SshSessionState::Disconnected,
            sftp_loading: false,
            directory_loaded: false,
            directory_loading_path: None,
            sftp_error: None,
            operation_busy: false,
            file_preview_loading: false,
            transfers_visible: false,
            next_terminal_ordinal: 1,
            directory_generation: 0,
            file_preview_generation: 0,
            terminal_generation: 0,
            capabilities: None,
            capability_error: None,
            capability_loading: false,
            capability_generation: 0,
        }
    }

    pub fn profile_id(&self) -> &SshProfileId {
        &self.profile.id
    }

    pub fn next_terminal_label(&mut self) -> SharedString {
        let ordinal = self.next_terminal_ordinal;
        self.next_terminal_ordinal = self.next_terminal_ordinal.wrapping_add(1).max(1);
        format!("终端 {ordinal}").into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_close_keeps_at_least_one_tab() {
        assert!(!can_close_terminal(0));
        assert!(!can_close_terminal(1));
        assert!(can_close_terminal(2));
    }

    #[test]
    fn closing_active_terminal_selects_the_previous_tab() {
        assert_eq!(terminal_index_after_close(9, 9), Some(8));
        assert_eq!(terminal_index_after_close(5, 9), Some(4));
        assert_eq!(terminal_index_after_close(0, 9), Some(0));
        assert_eq!(terminal_index_after_close(0, 0), None);
    }

    #[test]
    fn terminal_labels_remain_unique_when_tabs_are_removed() {
        let mut workspace =
            SshWorkspace::placeholder(SshProfile::new("server", "host"), "/".into());

        assert_eq!(workspace.next_terminal_label().as_ref(), "终端 1");
        assert_eq!(workspace.next_terminal_label().as_ref(), "终端 2");
    }

    #[test]
    fn session_state_text_describes_each_runtime_state() {
        assert_eq!(session_state_text(SshSessionState::Disconnected), "未连接");
        assert_eq!(session_state_text(SshSessionState::Connecting), "连接中");
        assert_eq!(session_state_text(SshSessionState::Connected), "已连接");
        assert_eq!(session_state_text(SshSessionState::Reconnecting), "重连中");
        assert_eq!(session_state_text(SshSessionState::Exited), "已退出");
        assert_eq!(session_state_text(SshSessionState::Failed), "连接失败");
    }

    #[test]
    fn session_state_maps_to_pulse_status_semantics() {
        assert_eq!(
            session_pulse_status(SshSessionState::Disconnected),
            ramag_ui::pulse_ui::PulseStatus::Unavailable
        );
        assert_eq!(
            session_pulse_status(SshSessionState::Connecting),
            ramag_ui::pulse_ui::PulseStatus::Warming
        );
        assert_eq!(
            session_pulse_status(SshSessionState::Connected),
            ramag_ui::pulse_ui::PulseStatus::Current
        );
        assert_eq!(
            session_pulse_status(SshSessionState::Failed),
            ramag_ui::pulse_ui::PulseStatus::Failed
        );
        assert_eq!(
            session_pulse_status(SshSessionState::Reconnecting),
            ramag_ui::pulse_ui::PulseStatus::Warming
        );
        assert_eq!(
            session_pulse_status(SshSessionState::Exited),
            ramag_ui::pulse_ui::PulseStatus::Unavailable
        );
    }

    #[test]
    fn terminal_exit_status_keeps_success_and_failure_distinct() {
        let success = TerminalExit {
            code: Some(0),
            success: true,
        };
        let failure = TerminalExit {
            code: Some(7),
            success: false,
        };

        assert_eq!(
            terminal_pulse_status(Some(&success), true),
            (ramag_ui::pulse_ui::PulseStatus::Unavailable, "已完成")
        );
        assert_eq!(
            terminal_pulse_status(Some(&failure), true),
            (ramag_ui::pulse_ui::PulseStatus::Failed, "异常退出")
        );
        assert_eq!(
            terminal_pulse_status(None, true),
            (ramag_ui::pulse_ui::PulseStatus::Unavailable, "已关闭")
        );
        assert_eq!(
            terminal_pulse_status(None, false),
            (ramag_ui::pulse_ui::PulseStatus::Current, "运行中")
        );
        assert_eq!(
            terminal_session_state(Some(&failure), true),
            SshSessionState::Failed
        );
        assert_eq!(
            terminal_session_state(Some(&success), true),
            SshSessionState::Exited
        );
        assert_eq!(terminal_session_state(None, true), SshSessionState::Exited);
        assert_eq!(
            terminal_session_state(None, false),
            SshSessionState::Connected
        );
    }

    #[test]
    fn workspace_status_preserves_failures_live_siblings_and_pending_launches() {
        use SshSessionState::{Connected, Connecting, Disconnected, Exited, Failed, Reconnecting};

        assert_eq!(workspace_session_state(Connected, false, [Failed]), Failed);
        assert_eq!(
            workspace_session_state(Connected, false, [Exited, Failed]),
            Failed
        );
        assert_eq!(
            workspace_session_state(Connected, false, [Failed, Connected]),
            Connected
        );
        assert_eq!(workspace_session_state(Connected, false, [Exited]), Exited);
        assert_eq!(
            workspace_session_state(Disconnected, false, []),
            Disconnected
        );
        assert_eq!(workspace_session_state(Failed, false, []), Failed);
        assert_eq!(
            workspace_session_state(Connecting, true, [Connected]),
            Connecting
        );
        assert_eq!(
            workspace_session_state(Reconnecting, true, [Failed]),
            Reconnecting
        );
    }

    #[test]
    fn terminal_tab_label_keeps_name_and_exit_code() {
        let success = TerminalExit {
            code: Some(0),
            success: true,
        };
        let failure = TerminalExit {
            code: Some(7),
            success: false,
        };

        assert_eq!(
            terminal_tab_label("终端 1", Some(&success), true),
            "终端 1 [退出: 0]"
        );
        assert_eq!(
            terminal_tab_label("终端 1", Some(&failure), true),
            "终端 1 [退出: 7]"
        );
        assert_eq!(terminal_tab_label("终端 1", None, true), "终端 1 [已关闭]");
        assert_eq!(terminal_tab_label("终端 1", None, false), "终端 1");
    }
}
