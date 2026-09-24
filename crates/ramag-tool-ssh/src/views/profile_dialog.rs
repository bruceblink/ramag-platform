//! SSH 配置弹窗状态、校验与异步操作。

use std::sync::Arc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{AppContext as _, Context, Entity, EventEmitter, Subscription, Window};
use ramag_app::SshService;
use ramag_domain::entities::{
    JumpServerRdpSession, RemoteCapabilityState, RemoteOperatingSystem, RemotePlatformPreference,
    RemoteShellKind, SftpNamespaceKind, SftpTransportKind, SshAuthMode, SshCapability,
    SshPortForward, SshProfile, SshProfileId, SshProfileOrigin, SshRemoteCapabilities,
};

use super::profile_form::ProfileForm;
use super::ssh_command::{MAX_SSH_COMMAND_BYTES, parse_ssh_command, profile_ssh_command};

#[derive(Debug, Clone)]
pub(super) enum ProfileFormEvent {
    SaveRequested(Box<SshProfile>),
    Cancelled,
    CapabilityChanged(Result<SshCapability, String>),
}

impl EventEmitter<ProfileFormEvent> for SshProfileFormPanel {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FormOperation {
    Saving,
    Testing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FeedbackKind {
    Info,
    Success,
    Error,
}

pub(super) struct FormFeedback {
    pub message: String,
    pub kind: FeedbackKind,
}

#[derive(PartialEq)]
struct FormSnapshot {
    values: Vec<String>,
    auth_mode: SshAuthMode,
    production: bool,
    remote_platform: RemotePlatformPreference,
    port_forwardings: Vec<SshPortForward>,
}

pub(super) struct SshProfileFormPanel {
    pub(super) service: Arc<SshService>,
    pub(super) form: ProfileForm,
    pub(super) command: Entity<InputState>,
    pub(super) editing_id: Option<SshProfileId>,
    origin: SshProfileOrigin,
    pub(super) auth_mode: SshAuthMode,
    pub(super) production: bool,
    pub(super) remote_platform: RemotePlatformPreference,
    pub(super) port_forwardings: Vec<SshPortForward>,
    rdp_web_enabled: Option<bool>,
    jumpserver_rdp_session: Option<JumpServerRdpSession>,
    pub(super) password_masked: bool,
    pub(super) operation: Option<FormOperation>,
    pub(super) feedback: Option<FormFeedback>,
    pub(super) default_capability: Option<Result<SshCapability, String>>,
    test_epoch: u64,
    capability_generation: u64,
    initial: FormSnapshot,
    _subscriptions: Vec<Subscription>,
}

impl SshProfileFormPanel {
    pub fn new(
        service: Arc<SshService>,
        profile: Option<SshProfile>,
        default_capability: Option<Result<SshCapability, String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = ProfileForm::new(window, cx);
        let command_value = profile
            .as_ref()
            .map(profile_ssh_command)
            .unwrap_or_default();
        let command = cx.new(|cx| {
            InputState::new(window, cx)
                .validate(|value, _| value.len() <= MAX_SSH_COMMAND_BYTES)
                .placeholder("ssh user@host -p 22 -i /path/to/key")
                .default_value(command_value)
        });
        form.set_profile(profile.as_ref(), window, cx);
        let editing_id = profile.as_ref().map(|profile| profile.id.clone());
        let origin = profile
            .as_ref()
            .map_or(SshProfileOrigin::Manual, |profile| profile.origin);
        let auth_mode = profile
            .as_ref()
            .map_or(SshAuthMode::Password, |profile| profile.auth_mode);
        let production = profile.as_ref().is_some_and(|profile| profile.production);
        let remote_platform = profile
            .as_ref()
            .map_or(RemotePlatformPreference::Auto, |profile| {
                profile.remote_platform
            });
        let port_forwardings = profile
            .as_ref()
            .map(|profile| profile.port_forwardings.clone())
            .unwrap_or_default();
        let rdp_web_enabled = profile.as_ref().and_then(|profile| profile.rdp_web_enabled);
        let jumpserver_rdp_session = profile
            .as_ref()
            .and_then(|profile| profile.jumpserver_rdp_session.clone());
        let mut subscriptions = Vec::new();
        for input in form.inputs() {
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.invalidate_test(cx);
                    }
                },
            ));
        }
        let initial = FormSnapshot {
            values: form.values(cx),
            auth_mode,
            production,
            remote_platform,
            port_forwardings: port_forwardings.clone(),
        };
        let mut this = Self {
            service,
            form,
            command,
            editing_id,
            origin,
            auth_mode,
            production,
            remote_platform,
            port_forwardings,
            rdp_web_enabled,
            jumpserver_rdp_session,
            password_masked: true,
            operation: None,
            feedback: None,
            default_capability,
            test_epoch: 0,
            capability_generation: 0,
            initial,
            _subscriptions: subscriptions,
        };
        if this.default_capability.is_none() {
            this.retry_openssh_probe(cx);
        }
        this
    }

    pub fn title(&self) -> &'static str {
        if self.editing_id.is_some() {
            "编辑"
        } else {
            "新建"
        }
    }

    pub fn is_busy(&self) -> bool {
        self.operation.is_some()
    }

    pub fn is_dirty(&self, cx: &gpui_kit::App) -> bool {
        self.snapshot(cx) != self.initial
    }

    fn snapshot(&self, cx: &gpui_kit::App) -> FormSnapshot {
        FormSnapshot {
            values: self.form.values(cx),
            auth_mode: self.auth_mode,
            production: self.production,
            remote_platform: self.remote_platform,
            port_forwardings: self.port_forwardings.clone(),
        }
    }

    pub(super) fn set_auth_mode(&mut self, mode: SshAuthMode, cx: &mut Context<Self>) {
        if self.is_busy() || self.auth_mode == mode {
            return;
        }
        self.auth_mode = mode;
        self.invalidate_test(cx);
    }

    pub(super) fn toggle_production(&mut self, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.production = !self.production;
        self.invalidate_test(cx);
    }

    pub(super) fn set_remote_platform(
        &mut self,
        platform: RemotePlatformPreference,
        cx: &mut Context<Self>,
    ) {
        if self.is_busy() || self.remote_platform == platform {
            return;
        }
        self.remote_platform = platform;
        self.invalidate_test(cx);
    }

    pub(super) fn parse_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        let command = self.command.read(cx).value().to_string();
        let user_home = current_user_home();
        let parsed = match parse_ssh_command(&command, user_home.as_deref()) {
            Ok(parsed) => parsed,
            Err(message) => {
                self.feedback = Some(FormFeedback {
                    message,
                    kind: FeedbackKind::Error,
                });
                cx.notify();
                return;
            }
        };

        if self.form.name.read(cx).value().trim().is_empty() {
            self.form
                .name
                .update(cx, |state, cx| state.set_value(&parsed.host, window, cx));
        }
        for (input, value) in [
            (&self.form.host, parsed.host),
            (
                &self.form.port,
                parsed.port.map(|port| port.to_string()).unwrap_or_default(),
            ),
            (&self.form.username, parsed.username),
            (
                &self.form.key_path,
                parsed.key_path.clone().unwrap_or_default(),
            ),
        ] {
            input.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        self.form
            .password
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.auth_mode = if parsed.key_path.is_some() {
            SshAuthMode::KeyFile
        } else {
            SshAuthMode::System
        };
        self.port_forwardings = parsed.port_forwardings;
        self.feedback = Some(FormFeedback {
            message: "已解析 SSH 命令".into(),
            kind: FeedbackKind::Success,
        });
        cx.notify();
    }

    fn invalidate_test(&mut self, cx: &mut Context<Self>) {
        self.test_epoch = self.test_epoch.wrapping_add(1);
        if self.operation != Some(FormOperation::Saving) {
            self.feedback = None;
        }
        cx.notify();
    }

    fn profile_from_form(&self, cx: &gpui_kit::App) -> Result<SshProfile, String> {
        let mut profile = self.form.to_profile(
            self.editing_id.clone(),
            self.origin,
            self.auth_mode,
            self.production,
            self.remote_platform,
            cx,
        )?;
        profile.rdp_web_enabled = self.rdp_web_enabled;
        profile.jumpserver_rdp_session = self.jumpserver_rdp_session.clone();
        profile.port_forwardings = self.port_forwardings.clone();
        profile.validate()?;
        Ok(profile)
    }

    pub(super) fn save(&mut self, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        let profile = match self.profile_from_form(cx) {
            Ok(profile) => profile,
            Err(error) => {
                self.feedback = Some(FormFeedback {
                    message: error,
                    kind: FeedbackKind::Error,
                });
                cx.notify();
                return;
            }
        };
        cx.emit(ProfileFormEvent::SaveRequested(Box::new(profile)));
    }

    pub(super) fn begin_save(&mut self, cx: &mut Context<Self>) {
        self.operation = Some(FormOperation::Saving);
        self.feedback = Some(FormFeedback {
            message: "保存中…".into(),
            kind: FeedbackKind::Info,
        });
        cx.notify();
    }

    pub(super) fn save_failed(&mut self, error: impl Into<String>, cx: &mut Context<Self>) {
        self.operation = None;
        self.feedback = Some(FormFeedback {
            message: format!("保存失败：{}", error.into()),
            kind: FeedbackKind::Error,
        });
        cx.notify();
    }

    pub(super) fn test_connection(&mut self, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        let profile = match self.profile_from_form(cx) {
            Ok(profile) => profile,
            Err(error) => {
                self.feedback = Some(FormFeedback {
                    message: error,
                    kind: FeedbackKind::Error,
                });
                cx.notify();
                return;
            }
        };
        self.operation = Some(FormOperation::Testing);
        self.feedback = Some(FormFeedback {
            message: "测试中…".into(),
            kind: FeedbackKind::Info,
        });
        let epoch = self.test_epoch;
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.test_connection(&profile).await;
            let _ = this.update(cx, |this, cx| {
                if this.test_epoch != epoch {
                    this.operation = None;
                    cx.notify();
                    return;
                }
                this.operation = None;
                this.feedback = Some(match result {
                    Ok(capabilities) => FormFeedback {
                        message: format_remote_capabilities(&capabilities),
                        kind: FeedbackKind::Success,
                    },
                    Err(error) => FormFeedback {
                        message: format!("测试失败：{error}"),
                        kind: FeedbackKind::Error,
                    },
                });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn retry_openssh_probe(&mut self, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.capability_generation = self.capability_generation.wrapping_add(1);
        let generation = self.capability_generation;
        self.default_capability = None;
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let capability = service.probe(None).await.map_err(|error| error.to_string());
            let _ = this.update(cx, |this, cx| {
                if this.capability_generation != generation {
                    return;
                }
                this.default_capability = Some(capability.clone());
                cx.emit(ProfileFormEvent::CapabilityChanged(capability));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn request_cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        if !self.is_dirty(cx) {
            cx.emit(ProfileFormEvent::Cancelled);
            return;
        }
        let form = cx.entity();
        ramag_ui::open_confirm(
            "放弃？",
            "未保存内容将丢失。",
            "放弃",
            true,
            move |_, app| {
                form.update(app, |_this, cx| cx.emit(ProfileFormEvent::Cancelled));
            },
            window,
            cx,
        );
    }

    pub(super) fn pick_local_path(
        &mut self,
        key_path: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_busy() {
            return;
        }
        cx.spawn_in(window, async move |this, async_cx| {
            let picked = rfd::AsyncFileDialog::new().pick_file().await;
            let _ = this.update_in(async_cx, |this, window, cx| {
                let Some(handle) = picked else {
                    return;
                };
                let Some(path) = handle.path().to_str().map(str::to_owned) else {
                    this.feedback = Some(FormFeedback {
                        message: "路径不是 UTF-8".into(),
                        kind: FeedbackKind::Error,
                    });
                    cx.notify();
                    return;
                };
                let input: &gpui_kit::Entity<InputState> = if key_path {
                    &this.form.key_path
                } else {
                    &this.form.ssh_path
                };
                input.update(cx, |state, cx| state.set_value(path, window, cx));
                cx.notify();
            });
        })
        .detach();
    }
}

fn current_user_home() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    let value = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let value = std::env::var_os("HOME");
    value.map(std::path::PathBuf::from)
}

fn format_remote_capabilities(capabilities: &SshRemoteCapabilities) -> String {
    format!(
        "测试完成 · OpenSSH {} · 认证 {} · 执行 {} · Terminal {} · SFTP {} · 通道 {} · 诊断 {} · 远端 {} · Shell {} · 路径 {}",
        remote_capability_state_label(capabilities.openssh_client),
        remote_capability_state_label(capabilities.ssh_authentication),
        remote_capability_state_label(capabilities.ssh_execution),
        remote_capability_state_label(capabilities.terminal),
        remote_capability_state_label(capabilities.sftp),
        sftp_transport_label(capabilities.sftp_transport),
        remote_capability_state_label(capabilities.diagnostic),
        remote_operating_system_label(capabilities.operating_system),
        remote_shell_label(capabilities.shell),
        sftp_namespace_label(capabilities.sftp_namespace),
    )
}

fn remote_capability_state_label(state: RemoteCapabilityState) -> &'static str {
    match state {
        RemoteCapabilityState::Available => "可用",
        RemoteCapabilityState::Unsupported => "不支持",
        RemoteCapabilityState::Failed => "失败",
        RemoteCapabilityState::BlockedByPolicy => "被策略阻止",
        RemoteCapabilityState::NotProbed => "未探测",
    }
}

fn sftp_transport_label(transport: Option<SftpTransportKind>) -> &'static str {
    match transport {
        Some(SftpTransportKind::StandardSubsystem) => "标准 SFTP",
        Some(SftpTransportKind::WindowsCompatibility) => "Windows 兼容 SFTP",
        None => "未确定",
    }
}

fn remote_operating_system_label(operating_system: RemoteOperatingSystem) -> &'static str {
    match operating_system {
        RemoteOperatingSystem::Linux => "Linux",
        RemoteOperatingSystem::Windows => "Windows",
        RemoteOperatingSystem::Unknown => "未识别",
    }
}

fn remote_shell_label(shell: RemoteShellKind) -> &'static str {
    match shell {
        RemoteShellKind::Posix => "POSIX",
        RemoteShellKind::Cmd => "Windows CMD",
        RemoteShellKind::WindowsPowerShell => "Windows PowerShell",
        RemoteShellKind::PowerShellCore => "PowerShell Core",
        RemoteShellKind::Unknown => "未识别",
    }
}

fn sftp_namespace_label(namespace: SftpNamespaceKind) -> &'static str {
    match namespace {
        SftpNamespaceKind::Posix => "POSIX",
        SftpNamespaceKind::WindowsDrive => "Windows 盘符",
        SftpNamespaceKind::Virtual => "虚拟路径",
        SftpNamespaceKind::Unknown => "未识别",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_capability_feedback_uses_readable_labels() {
        let capabilities = SshRemoteCapabilities {
            openssh_client: RemoteCapabilityState::Available,
            ssh_authentication: RemoteCapabilityState::BlockedByPolicy,
            operating_system: RemoteOperatingSystem::Windows,
            shell: RemoteShellKind::WindowsPowerShell,
            ssh_execution: RemoteCapabilityState::Failed,
            terminal: RemoteCapabilityState::Unsupported,
            sftp: RemoteCapabilityState::NotProbed,
            sftp_namespace: SftpNamespaceKind::WindowsDrive,
            sftp_transport: Some(SftpTransportKind::WindowsCompatibility),
            ..SshRemoteCapabilities::default()
        };

        let message = format_remote_capabilities(&capabilities);

        assert!(message.contains("OpenSSH 可用"));
        assert!(message.contains("认证 被策略阻止"));
        assert!(message.contains("执行 失败"));
        assert!(message.contains("Terminal 不支持"));
        assert!(message.contains("SFTP 未探测"));
        assert!(message.contains("通道 Windows 兼容 SFTP"));
        assert!(message.contains("远端 Windows"));
        assert!(message.contains("Shell Windows PowerShell"));
        assert!(message.contains("路径 Windows 盘符"));
        assert!(!message.contains("Available"));
        assert!(!message.contains("BlockedByPolicy"));
        assert!(!message.contains("WindowsPowerShell"));
    }
}
