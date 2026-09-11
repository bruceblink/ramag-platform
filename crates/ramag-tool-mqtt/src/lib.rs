#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

//! MQTT 工作区 UI。
//!
//! 所有连接、发布、订阅和 Mosquitto 管理请求都通过 `MqttService` 执行。视图只保存用户
//! 输入和真实返回结果，不在客户端生成 Topic、在线客户端或权限样例。

use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use async_channel::{TrySendError, bounded};
use gpui::{
    App, AppContext as _, ClickEvent, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, Window, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    button::ButtonVariants as _,
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use ramag_app::MqttService;
use ramag_domain::{
    entities::{
        MosquittoAcl, MosquittoAclDecision, MosquittoAclType, MosquittoClient,
        MosquittoConfigTarget, MosquittoDynamicSecuritySnapshot, MosquittoGroup,
        MosquittoGroupBinding, MosquittoRole, MosquittoRoleBinding, MosquittoStaticConfig,
        MosquittoStaticFile, MosquittoStaticFileKind, MqttBrokerSnapshot, MqttMessage,
        MqttMessageSinkResult, MqttProfile, MqttProfileId, MqttProtocolVersion, MqttPublishRequest,
        MqttQos, MqttSubscribeRequest, MqttSubscription, MqttTlsConfig,
        MqttTransport as TransportKind, MqttTransportCapabilities,
    },
    traits::{Tool, ToolMeta},
};

const MAX_MESSAGES: usize = 500;
const MAX_PROFILE_NAME_BYTES: usize = 256;
const MAX_HOST_BYTES: usize = 1024;
const MAX_CLIENT_ID_BYTES: usize = 256;
const MAX_USERNAME_BYTES: usize = 1024;
const MAX_PASSWORD_BYTES: usize = 64 * 1024;
const MAX_PATH_BYTES: usize = 32 * 1024;
const MAX_TOPIC_BYTES: usize = u16::MAX as usize;
const MAX_MOSQUITTO_DESCRIPTION_BYTES: usize = 16 * 1024;
const MAX_MOSQUITTO_BINDINGS_BYTES: usize = 16 * 1024;
const MAX_MOSQUITTO_ACL_EDITOR_BYTES: usize = 4 * 1024 * 1024;
const MAX_MOSQUITTO_STATIC_FILE_BYTES: usize = 4 * 1024 * 1024;
const MQTT_SIDEBAR_WIDTH: f32 = 250.0;
const MQTT_SIDEBAR_COLLAPSE_BREAKPOINT: f32 = 480.0;

/// 创建 MQTT 工具主视图。
pub fn create_mqtt_view(
    service: Arc<MqttService>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<MqttView> {
    cx.new(|cx| MqttView::new(service, window, cx))
}

/// MQTT 工具在 Activity Bar 中的注册信息。
pub struct MqttTool {
    meta: ToolMeta,
}

impl MqttTool {
    pub const ID: &'static str = "mqtt";

    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(
                Self::ID,
                "MQTT",
                "连接 Broker、发布订阅消息并管理 Mosquitto",
            )
            .with_icon("mqtt"),
        }
    }
}

impl Default for MqttTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for MqttTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MqttSection {
    Config,
    Overview,
    Publish,
    Subscribe,
    Mosquitto,
}

impl MqttSection {
    const ALL: [Self; 5] = [
        Self::Config,
        Self::Overview,
        Self::Publish,
        Self::Subscribe,
        Self::Mosquitto,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Config => "配置",
            Self::Overview => "状态",
            Self::Publish => "发布",
            Self::Subscribe => "订阅",
            Self::Mosquitto => "Mosquitto",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MosquittoManagementSection {
    Clients,
    Groups,
    Roles,
    StaticFiles,
}

impl MosquittoManagementSection {
    const ALL: [Self; 4] = [Self::Clients, Self::Groups, Self::Roles, Self::StaticFiles];

    const fn label(self) -> &'static str {
        match self {
            Self::Clients => "用户",
            Self::Groups => "组",
            Self::Roles => "角色与 ACL",
            Self::StaticFiles => "静态文件",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ClientPermissionRow {
    source: String,
    role_name: String,
    binding_priority: i32,
    acl: MosquittoAcl,
}

/// MQTT 页面状态；列表和消息只在对应服务请求成功后写入。
pub struct MqttView {
    service: Arc<MqttService>,
    profiles: Vec<MqttProfile>,
    selected_profile_id: Option<MqttProfileId>,
    section: MqttSection,
    /// Narrow windows hide the profile list by default so the active form keeps usable width.
    sidebar_visible: bool,
    name: Entity<InputState>,
    host: Entity<InputState>,
    port: Entity<InputState>,
    client_id: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    management_admin_username: Entity<InputState>,
    management_admin_password: Entity<InputState>,
    password_file_path: Entity<InputState>,
    acl_file_path: Entity<InputState>,
    ca_cert_path: Entity<InputState>,
    client_cert_path: Entity<InputState>,
    client_key_path: Entity<InputState>,
    keep_alive: Entity<InputState>,
    publish_topic: Entity<InputState>,
    publish_payload: Entity<InputState>,
    subscribe_filter: Entity<InputState>,
    search: Entity<InputState>,
    client_username: Entity<InputState>,
    client_id_editor: Entity<InputState>,
    client_password: Entity<InputState>,
    client_text_name: Entity<InputState>,
    client_text_description: Entity<InputState>,
    client_groups: Entity<InputState>,
    client_roles: Entity<InputState>,
    client_disabled: bool,
    group_name_editor: Entity<InputState>,
    group_text_name: Entity<InputState>,
    group_text_description: Entity<InputState>,
    group_roles: Entity<InputState>,
    role_name_editor: Entity<InputState>,
    role_text_name: Entity<InputState>,
    role_text_description: Entity<InputState>,
    role_acls: Entity<InputState>,
    role_allow_wildcards: bool,
    static_content: Entity<InputState>,
    transport: TransportKind,
    protocol: MqttProtocolVersion,
    clean_start: bool,
    management_enabled: bool,
    loading_profiles: bool,
    saving: bool,
    testing: bool,
    deleting: bool,
    publishing: bool,
    loading_snapshot: bool,
    loading_management: bool,
    saving_management: bool,
    deleting_management: bool,
    loading_static_file: bool,
    saving_static_file: bool,
    snapshot: Option<MqttBrokerSnapshot>,
    management_snapshot: Option<MosquittoDynamicSecuritySnapshot>,
    management_section: MosquittoManagementSection,
    selected_client_username: Option<String>,
    selected_group_name: Option<String>,
    selected_role_name: Option<String>,
    static_file_kind: MosquittoStaticFileKind,
    static_file: Option<MosquittoStaticFile>,
    messages: VecDeque<MqttMessage>,
    subscription_running: bool,
    subscription_cancelled: Option<Arc<AtomicBool>>,
    profile_request_id: u64,
    operation_id: u64,
    snapshot_request_id: u64,
    subscription_request_id: u64,
    notice: Option<(String, bool)>,
    snapshot_error: Option<String>,
    management_error: Option<String>,
    management_operation_id: u64,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Drop for MqttView {
    fn drop(&mut self) {
        if let Some(cancelled) = self.subscription_cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
    }
}

impl Focusable for MqttView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl MqttView {
    pub fn new(service: Arc<MqttService>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = input(window, cx, MAX_PROFILE_NAME_BYTES, "配置名称", false, "");
        let host = input(
            window,
            cx,
            MAX_HOST_BYTES,
            "Broker 地址，例如 127.0.0.1",
            false,
            "",
        );
        let port = input(window, cx, 5, "1883", false, "1883");
        let client_id = input(
            window,
            cx,
            MAX_CLIENT_ID_BYTES,
            "Client ID（可选）",
            false,
            "",
        );
        let username = input(window, cx, MAX_USERNAME_BYTES, "用户名（可选）", false, "");
        let password = input(
            window,
            cx,
            MAX_PASSWORD_BYTES,
            "密码（留空保持已保存密码）",
            true,
            "",
        );
        let management_admin_username = input(
            window,
            cx,
            MAX_USERNAME_BYTES,
            "管理用户名（可选）",
            false,
            "",
        );
        let management_admin_password =
            input(window, cx, MAX_PASSWORD_BYTES, "管理密码（可选）", true, "");
        let password_file_path = input(
            window,
            cx,
            MAX_PATH_BYTES,
            "password_file 绝对路径",
            false,
            "",
        );
        let acl_file_path = input(window, cx, MAX_PATH_BYTES, "acl_file 绝对路径", false, "");
        let ca_cert_path = input(window, cx, MAX_PATH_BYTES, "CA 证书路径（可选）", false, "");
        let client_cert_path = input(
            window,
            cx,
            MAX_PATH_BYTES,
            "客户端证书路径（可选）",
            false,
            "",
        );
        let client_key_path = input(
            window,
            cx,
            MAX_PATH_BYTES,
            "客户端密钥路径（可选）",
            false,
            "",
        );
        let keep_alive = input(window, cx, 5, "Keep Alive 秒数", false, "60");
        let publish_topic = input(window, cx, MAX_TOPIC_BYTES, "发布 Topic", false, "");
        let publish_payload = input(window, cx, 16 * 1024 * 1024, "消息内容", false, "");
        let subscribe_filter = input(
            window,
            cx,
            MAX_TOPIC_BYTES,
            "Topic Filter，例如 sensors/#",
            false,
            "",
        );
        let search = input(window, cx, MAX_PROFILE_NAME_BYTES, "筛选配置…", false, "");
        let client_username = input(window, cx, MAX_USERNAME_BYTES, "用户名", false, "");
        let client_id_editor = input(
            window,
            cx,
            MAX_CLIENT_ID_BYTES,
            "Client ID（可选）",
            false,
            "",
        );
        let client_password = input(
            window,
            cx,
            MAX_PASSWORD_BYTES,
            "新密码（留空不修改）",
            true,
            "",
        );
        let client_text_name = input(window, cx, MAX_PROFILE_NAME_BYTES, "显示名称", false, "");
        let client_text_description = input(
            window,
            cx,
            MAX_MOSQUITTO_DESCRIPTION_BYTES,
            "描述",
            false,
            "",
        );
        let client_groups = input(
            window,
            cx,
            MAX_MOSQUITTO_BINDINGS_BYTES,
            "Group 名称，逗号分隔",
            false,
            "",
        );
        let client_roles = input(
            window,
            cx,
            MAX_MOSQUITTO_BINDINGS_BYTES,
            "Role 名称，逗号分隔",
            false,
            "",
        );
        let group_name_editor = input(window, cx, MAX_PROFILE_NAME_BYTES, "Group 名称", false, "");
        let group_text_name = input(window, cx, MAX_PROFILE_NAME_BYTES, "显示名称", false, "");
        let group_text_description = input(
            window,
            cx,
            MAX_MOSQUITTO_DESCRIPTION_BYTES,
            "描述",
            false,
            "",
        );
        let group_roles = input(
            window,
            cx,
            MAX_MOSQUITTO_BINDINGS_BYTES,
            "Role 名称，逗号分隔",
            false,
            "",
        );
        let role_name_editor = input(window, cx, MAX_PROFILE_NAME_BYTES, "Role 名称", false, "");
        let role_text_name = input(window, cx, MAX_PROFILE_NAME_BYTES, "显示名称", false, "");
        let role_text_description = input(
            window,
            cx,
            MAX_MOSQUITTO_DESCRIPTION_BYTES,
            "描述",
            false,
            "",
        );
        let role_acls = input(
            window,
            cx,
            MAX_MOSQUITTO_ACL_EDITOR_BYTES,
            "ACL 编辑器",
            false,
            "",
        );
        let static_content = input(
            window,
            cx,
            MAX_MOSQUITTO_STATIC_FILE_BYTES,
            "静态文件内容",
            false,
            "",
        );

        let fields = [
            &name,
            &host,
            &port,
            &client_id,
            &username,
            &password,
            &management_admin_username,
            &management_admin_password,
            &password_file_path,
            &acl_file_path,
            &ca_cert_path,
            &client_cert_path,
            &client_key_path,
            &keep_alive,
            &publish_topic,
            &publish_payload,
            &subscribe_filter,
            &search,
            &client_username,
            &client_id_editor,
            &client_password,
            &client_text_name,
            &client_text_description,
            &client_groups,
            &client_roles,
            &group_name_editor,
            &group_text_name,
            &group_text_description,
            &group_roles,
            &role_name_editor,
            &role_text_name,
            &role_text_description,
            &role_acls,
            &static_content,
        ];
        let mut subscriptions = Vec::with_capacity(fields.len());
        for field in fields {
            subscriptions.push(cx.subscribe(field, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.notice = None;
                    cx.notify();
                }
            }));
        }

        let mut view = Self {
            service,
            profiles: Vec::new(),
            selected_profile_id: None,
            section: MqttSection::Config,
            sidebar_visible: false,
            name,
            host,
            port,
            client_id,
            username,
            password,
            management_admin_username,
            management_admin_password,
            password_file_path,
            acl_file_path,
            ca_cert_path,
            client_cert_path,
            client_key_path,
            keep_alive,
            publish_topic,
            publish_payload,
            subscribe_filter,
            search,
            client_username,
            client_id_editor,
            client_password,
            client_text_name,
            client_text_description,
            client_groups,
            client_roles,
            client_disabled: false,
            group_name_editor,
            group_text_name,
            group_text_description,
            group_roles,
            role_name_editor,
            role_text_name,
            role_text_description,
            role_acls,
            role_allow_wildcards: false,
            static_content,
            transport: TransportKind::Tcp,
            protocol: MqttProtocolVersion::V5,
            clean_start: true,
            management_enabled: false,
            loading_profiles: true,
            saving: false,
            testing: false,
            deleting: false,
            publishing: false,
            loading_snapshot: false,
            loading_management: false,
            saving_management: false,
            deleting_management: false,
            loading_static_file: false,
            saving_static_file: false,
            snapshot: None,
            management_snapshot: None,
            management_section: MosquittoManagementSection::Clients,
            selected_client_username: None,
            selected_group_name: None,
            selected_role_name: None,
            static_file_kind: MosquittoStaticFileKind::Password,
            static_file: None,
            messages: VecDeque::new(),
            subscription_running: false,
            subscription_cancelled: None,
            profile_request_id: 0,
            operation_id: 0,
            snapshot_request_id: 0,
            subscription_request_id: 0,
            notice: None,
            snapshot_error: None,
            management_error: None,
            management_operation_id: 0,
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        view.load_profiles(window, cx);
        view
    }

    fn selected_profile(&self) -> Option<&MqttProfile> {
        self.selected_profile_id
            .as_ref()
            .and_then(|id| self.profiles.iter().find(|profile| &profile.id == id))
    }

    fn is_busy(&self) -> bool {
        self.saving || self.testing || self.deleting || self.publishing || self.loading_profiles
    }

    /// Detect the width where the profile list would leave the active MQTT workbench unusable.
    fn sidebar_is_narrow(window: &Window) -> bool {
        window.viewport_size().width < px(MQTT_SIDEBAR_COLLAPSE_BREAKPOINT)
    }

    fn load_profiles(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.profile_request_id = self.profile_request_id.wrapping_add(1);
        let request_id = self.profile_request_id;
        self.loading_profiles = true;
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.list_profiles().await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.profile_request_id != request_id {
                    return;
                }
                this.loading_profiles = false;
                match result {
                    Ok(profiles) => {
                        let selected = this
                            .selected_profile_id
                            .clone()
                            .filter(|id| profiles.iter().any(|profile| &profile.id == id));
                        this.profiles = profiles;
                        if let Some(id) =
                            selected.or_else(|| this.profiles.first().map(|p| p.id.clone()))
                        {
                            if let Some(profile) =
                                this.profiles.iter().find(|p| p.id == id).cloned()
                            {
                                this.selected_profile_id = Some(id);
                                this.set_form_from_profile(&profile, window, cx);
                            }
                        } else {
                            this.reset_form(window, cx);
                        }
                    }
                    Err(error) => {
                        this.notice = Some((
                            format!("加载 MQTT 配置失败：{}", error.user_message()),
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn reset_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_subscription();
        self.selected_profile_id = None;
        self.transport = TransportKind::Tcp;
        self.protocol = MqttProtocolVersion::V5;
        self.clean_start = true;
        self.management_enabled = false;
        for field in [
            &self.name,
            &self.host,
            &self.client_id,
            &self.username,
            &self.password,
            &self.management_admin_username,
            &self.management_admin_password,
            &self.password_file_path,
            &self.acl_file_path,
            &self.ca_cert_path,
            &self.client_cert_path,
            &self.client_key_path,
            &self.publish_topic,
            &self.publish_payload,
            &self.subscribe_filter,
        ] {
            set_value(field, "", window, cx);
        }
        set_value(&self.port, "1883", window, cx);
        set_value(&self.keep_alive, "60", window, cx);
        for field in [
            &self.client_username,
            &self.client_id_editor,
            &self.client_password,
            &self.client_text_name,
            &self.client_text_description,
            &self.client_groups,
            &self.client_roles,
            &self.group_name_editor,
            &self.group_text_name,
            &self.group_text_description,
            &self.group_roles,
            &self.role_name_editor,
            &self.role_text_name,
            &self.role_text_description,
            &self.role_acls,
            &self.static_content,
        ] {
            set_value(field, "", window, cx);
        }
        self.management_section = MosquittoManagementSection::Clients;
        self.selected_client_username = None;
        self.selected_group_name = None;
        self.selected_role_name = None;
        self.client_disabled = false;
        self.role_allow_wildcards = false;
        self.static_file_kind = MosquittoStaticFileKind::Password;
        self.static_file = None;
        self.password.update(cx, |state, cx| {
            state.set_placeholder("密码（可选）", window, cx);
        });
        self.clear_runtime_state();
        self.client_disabled = false;
        self.role_allow_wildcards = false;
        self.notice = None;
    }

    fn set_form_from_profile(
        &mut self,
        profile: &MqttProfile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_subscription();
        self.transport = profile.transport;
        self.protocol = profile.protocol_version;
        self.clean_start = profile.clean_start;
        self.management_enabled = profile.management.enabled;
        set_value(&self.name, profile.name.clone(), window, cx);
        set_value(&self.host, profile.host.clone(), window, cx);
        set_value(&self.port, profile.port.to_string(), window, cx);
        set_value(
            &self.client_id,
            profile.client_id.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.username,
            profile.username.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.password, "", window, cx);
        set_value(
            &self.management_admin_username,
            profile
                .management
                .admin_username
                .clone()
                .unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.management_admin_password, "", window, cx);
        set_value(
            &self.password_file_path,
            profile
                .management
                .static_config
                .as_ref()
                .and_then(|config| config.password_file.clone())
                .unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.acl_file_path,
            profile
                .management
                .static_config
                .as_ref()
                .and_then(|config| config.acl_file.clone())
                .unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.ca_cert_path,
            profile.tls.ca_cert_path.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.client_cert_path,
            profile.tls.client_cert_path.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.client_key_path,
            profile.tls.client_key_path.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.keep_alive,
            profile.keep_alive_seconds.to_string(),
            window,
            cx,
        );
        self.password.update(cx, |state, cx| {
            state.set_placeholder(
                if profile.password.is_some() {
                    "已保存密码，留空保持；输入新值可替换"
                } else {
                    "密码（可选）"
                },
                window,
                cx,
            );
        });
        self.publish_topic.update(cx, |state, cx| {
            state.set_placeholder("发布 Topic", window, cx);
        });
        self.clear_runtime_state();
        self.notice = None;
    }

    fn clear_runtime_state(&mut self) {
        self.snapshot = None;
        self.management_snapshot = None;
        self.snapshot_error = None;
        self.management_error = None;
        self.selected_client_username = None;
        self.selected_group_name = None;
        self.selected_role_name = None;
        self.static_file = None;
        self.messages.clear();
    }

    fn select_profile(&mut self, id: MqttProfileId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
        else {
            return;
        };
        self.selected_profile_id = Some(id);
        self.section = MqttSection::Overview;
        self.set_form_from_profile(&profile, window, cx);
        cx.notify();
    }

    fn new_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.section = MqttSection::Config;
        self.reset_form(window, cx);
        cx.notify();
    }

    fn form_profile(&self, cx: &App) -> Result<MqttProfile, String> {
        let name = value(&self.name, cx);
        let host = value(&self.host, cx);
        let port = value(&self.port, cx)
            .parse::<u16>()
            .map_err(|_| "端口必须是 1 - 65535 的整数".to_string())?;
        let keep_alive_seconds = value(&self.keep_alive, cx)
            .parse::<u16>()
            .map_err(|_| "Keep Alive 必须是 0 - 65535 的整数".to_string())?;
        let mut profile = self
            .selected_profile()
            .cloned()
            .unwrap_or_else(|| MqttProfile::new(name.clone(), host.clone(), port));
        profile.name = name;
        profile.host = host;
        profile.port = port;
        profile.transport = self.transport;
        profile.protocol_version = self.protocol;
        profile.client_id = optional_value(&self.client_id, cx);
        profile.username = optional_value(&self.username, cx);
        if let Some(password) = optional_value(&self.password, cx) {
            profile.password = Some(password);
        }
        profile.tls = MqttTlsConfig {
            verify: profile.tls.verify,
            ca_cert_path: optional_value(&self.ca_cert_path, cx),
            client_cert_path: optional_value(&self.client_cert_path, cx),
            client_key_path: optional_value(&self.client_key_path, cx),
        };
        profile.keep_alive_seconds = keep_alive_seconds;
        profile.clean_start = self.clean_start;
        profile.management.enabled = self.management_enabled;
        if !self.management_enabled {
            profile.management = Default::default();
        } else {
            if let Some(username) = optional_value(&self.management_admin_username, cx) {
                profile.management.admin_username = Some(username);
            }
            if let Some(password) = optional_value(&self.management_admin_password, cx) {
                profile.management.admin_password = Some(password);
            }
            let password_file = optional_value(&self.password_file_path, cx);
            let acl_file = optional_value(&self.acl_file_path, cx);
            profile.management.static_config = match (password_file, acl_file) {
                (None, None) => None,
                (password_file, acl_file) => Some(MosquittoStaticConfig {
                    target: profile
                        .management
                        .static_config
                        .as_ref()
                        .map(|config| config.target.clone())
                        .unwrap_or(MosquittoConfigTarget::Local),
                    password_file,
                    acl_file,
                }),
            };
        }
        profile.validate()?;
        Ok(profile)
    }

    fn save_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy() || self.subscription_running {
            return;
        }
        let profile = match self.form_profile(cx) {
            Ok(profile) => profile,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let service = self.service.clone();
        let id = profile.id.clone();
        let name = profile.name.clone();
        self.operation_id = self.operation_id.wrapping_add(1);
        let operation_id = self.operation_id;
        self.saving = true;
        self.notice = Some(("正在保存本机加密配置…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service.save_profile(&profile).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.operation_id != operation_id {
                    return;
                }
                this.saving = false;
                match result {
                    Ok(()) => {
                        if let Some(existing) = this.profiles.iter_mut().find(|item| item.id == id)
                        {
                            *existing = profile.clone();
                        } else {
                            this.profiles.push(profile.clone());
                        }
                        this.selected_profile_id = Some(id);
                        this.set_form_from_profile(&profile, window, cx);
                        this.notice = Some((format!("已保存「{name}」"), false));
                    }
                    Err(error) => {
                        this.notice = Some((format!("保存失败：{}", error.user_message()), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn delete_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy() || self.subscription_running {
            return;
        }
        let Some(id) = self.selected_profile_id.clone() else {
            self.notice = Some(("请先选择要删除的 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let service = self.service.clone();
        self.deleting = true;
        self.notice = Some(("正在删除本机配置…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service.delete_profile(&id).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.deleting = false;
                match result {
                    Ok(()) => {
                        this.profiles.retain(|profile| profile.id != id);
                        this.reset_form(window, cx);
                        this.notice = Some(("配置已删除".into(), false));
                    }
                    Err(error) => {
                        this.notice = Some((format!("删除失败：{}", error.user_message()), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn test_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy() || self.subscription_running {
            return;
        }
        let profile = match self.form_profile(cx) {
            Ok(profile) => profile,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let service = self.service.clone();
        self.testing = true;
        self.notice = Some(("正在连接 MQTT Broker…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service.test_connection(&profile).await;
            let _ = this.update(cx, |this, cx| {
                this.testing = false;
                this.notice = Some(match result {
                    Ok(()) => ("MQTT 连接测试成功".into(), false),
                    Err(error) => (format!("连接测试失败：{}", error.user_message()), true),
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn load_snapshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        self.snapshot_request_id = self.snapshot_request_id.wrapping_add(1);
        let request_id = self.snapshot_request_id;
        let service = self.service.clone();
        self.loading_snapshot = true;
        self.snapshot_error = None;
        self.notice = Some(("正在读取 Broker 状态…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service.broker_snapshot(&profile).await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.snapshot_request_id != request_id {
                    return;
                }
                this.loading_snapshot = false;
                match result {
                    Ok(snapshot) => {
                        this.snapshot = Some(snapshot);
                        this.notice = Some(("Broker 状态读取完成".into(), false));
                    }
                    Err(error) => {
                        let message = error.user_message();
                        this.snapshot_error = Some(message.clone());
                        this.notice = Some((format!("读取 Broker 状态失败：{message}"), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn publish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.publishing || self.subscription_running {
            return;
        }
        let Some(profile) = self
            .selected_profile()
            .cloned()
            .or_else(|| self.form_profile(cx).ok())
        else {
            self.notice = Some(("请先填写有效的 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let request = MqttPublishRequest {
            topic: value(&self.publish_topic, cx),
            payload: value(&self.publish_payload, cx).into_bytes(),
            qos: MqttQos::AtMostOnce,
            retain: false,
            user_properties: Vec::new(),
        };
        if let Err(error) = request.validate() {
            self.notice = Some((error, true));
            cx.notify();
            return;
        }
        let service = self.service.clone();
        self.publishing = true;
        self.notice = Some(("正在发布 MQTT 消息…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service.publish(&profile, &request).await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.publishing = false;
                this.notice = Some(match result {
                    Ok(result) => (
                        format!("已发布到 {}（QoS {}）", result.topic, result.qos.as_u8()),
                        false,
                    ),
                    Err(error) => (format!("发布失败：{}", error.user_message()), true),
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn start_subscription(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.subscription_running {
            return;
        }
        let Some(profile) = self
            .selected_profile()
            .cloned()
            .or_else(|| self.form_profile(cx).ok())
        else {
            self.notice = Some(("请先填写有效的 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let request = MqttSubscribeRequest {
            subscriptions: vec![MqttSubscription {
                filter: value(&self.subscribe_filter, cx),
                qos: MqttQos::AtLeastOnce,
            }],
        };
        if let Err(error) = request.validate() {
            self.notice = Some((error, true));
            cx.notify();
            return;
        }
        self.subscription_request_id = self.subscription_request_id.wrapping_add(1);
        let request_id = self.subscription_request_id;
        let cancelled = Arc::new(AtomicBool::new(false));
        self.subscription_cancelled = Some(cancelled.clone());
        self.subscription_running = true;
        self.messages.clear();
        self.notice = Some(("已启动订阅，等待 Broker 消息…".into(), false));
        let (sender, receiver) = bounded(32);
        let sink = Arc::new(move |message: MqttMessage| match sender.try_send(message) {
            Ok(()) => MqttMessageSinkResult::Accepted,
            Err(TrySendError::Full(_)) => MqttMessageSinkResult::Backpressured,
            Err(TrySendError::Closed(_)) => MqttMessageSinkResult::Closed,
        });
        let receiver_cancelled = cancelled.clone();
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(message) = receiver.recv().await {
                if this
                    .update_in(cx, |this, _, cx| {
                        if this.subscription_request_id != request_id {
                            return;
                        }
                        if this.messages.len() >= MAX_MESSAGES {
                            this.messages.pop_front();
                        }
                        this.messages.push_back(message);
                        cx.notify();
                    })
                    .is_err()
                {
                    receiver_cancelled.store(true, Ordering::Release);
                    break;
                }
            }
        })
        .detach();
        let service = self.service.clone();
        let operation_cancelled = cancelled;
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .subscribe(&profile, &request, sink, operation_cancelled)
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.subscription_request_id != request_id {
                    return;
                }
                this.subscription_running = false;
                this.subscription_cancelled = None;
                if let Err(error) = result {
                    this.notice = Some((format!("订阅结束：{}", error.user_message()), true));
                } else {
                    this.notice = Some(("订阅已结束".into(), false));
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn stop_subscription(&mut self) {
        self.subscription_request_id = self.subscription_request_id.wrapping_add(1);
        if let Some(cancelled) = self.subscription_cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
        self.subscription_running = false;
    }

    fn load_management(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择已启用 Mosquitto 管理的配置".into(), true));
            cx.notify();
            return;
        };
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.loading_management = true;
        self.management_error = None;
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.dynamic_security_snapshot(&profile).await;
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.loading_management = false;
                match result {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.notice = Some(("Mosquitto Dynamic Security 读取完成".into(), false));
                    }
                    Err(error) => {
                        let message = error.user_message();
                        this.management_error = Some(message.clone());
                        this.notice = Some((format!("Mosquitto 管理读取失败：{message}"), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn select_management_section(
        &mut self,
        section: MosquittoManagementSection,
        cx: &mut Context<Self>,
    ) {
        self.management_section = section;
        cx.notify();
    }

    fn select_client(&mut self, username: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(client) = self
            .management_snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .clients
                    .iter()
                    .find(|client| client.username == username)
            })
            .cloned()
        else {
            return;
        };
        self.management_section = MosquittoManagementSection::Clients;
        self.selected_client_username = Some(client.username.clone());
        set_value(&self.client_username, client.username, window, cx);
        set_value(
            &self.client_id_editor,
            client.client_id.unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.client_password, "", window, cx);
        set_value(
            &self.client_text_name,
            client.text_name.unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.client_text_description,
            client.text_description.unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.client_groups,
            join_group_bindings(&client.groups),
            window,
            cx,
        );
        set_value(
            &self.client_roles,
            join_role_bindings(&client.roles),
            window,
            cx,
        );
        self.client_disabled = client.disabled;
        cx.notify();
    }

    fn select_group(&mut self, group_name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(group) = self
            .management_snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .groups
                    .iter()
                    .find(|group| group.group_name == group_name)
            })
            .cloned()
        else {
            return;
        };
        self.management_section = MosquittoManagementSection::Groups;
        self.selected_group_name = Some(group.group_name.clone());
        set_value(&self.group_name_editor, group.group_name, window, cx);
        set_value(
            &self.group_text_name,
            group.text_name.unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.group_text_description,
            group.text_description.unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.group_roles,
            join_role_bindings(&group.roles),
            window,
            cx,
        );
        cx.notify();
    }

    fn select_role(&mut self, role_name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(role) = self
            .management_snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .roles
                    .iter()
                    .find(|role| role.role_name == role_name)
            })
            .cloned()
        else {
            return;
        };
        self.management_section = MosquittoManagementSection::Roles;
        self.selected_role_name = Some(role.role_name.clone());
        set_value(&self.role_name_editor, role.role_name, window, cx);
        set_value(
            &self.role_text_name,
            role.text_name.unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.role_text_description,
            role.text_description.unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.role_acls, serialize_acls(&role.acls), window, cx);
        self.role_allow_wildcards = role.allow_wildcards_subscriptions;
        cx.notify();
    }

    fn client_from_editor(&self, cx: &App) -> std::result::Result<MosquittoClient, String> {
        let client = MosquittoClient {
            username: value(&self.client_username, cx),
            client_id: optional_value(&self.client_id_editor, cx),
            password_configured: self
                .selected_client_username
                .as_ref()
                .and_then(|username| {
                    self.management_snapshot.as_ref().and_then(|snapshot| {
                        snapshot
                            .clients
                            .iter()
                            .find(|client| &client.username == username)
                            .map(|client| client.password_configured)
                    })
                })
                .unwrap_or(false),
            password: optional_value(&self.client_password, cx),
            disabled: self.client_disabled,
            text_name: optional_value(&self.client_text_name, cx),
            text_description: optional_value(&self.client_text_description, cx),
            groups: parse_group_bindings(&value(&self.client_groups, cx))?,
            roles: parse_role_bindings(&value(&self.client_roles, cx))?,
        };
        client.validate()?;
        Ok(client)
    }

    fn group_from_editor(&self, cx: &App) -> std::result::Result<MosquittoGroup, String> {
        let group = MosquittoGroup {
            group_name: value(&self.group_name_editor, cx),
            text_name: optional_value(&self.group_text_name, cx),
            text_description: optional_value(&self.group_text_description, cx),
            roles: parse_role_bindings(&value(&self.group_roles, cx))?,
        };
        group.validate()?;
        Ok(group)
    }

    fn role_from_editor(&self, cx: &App) -> std::result::Result<MosquittoRole, String> {
        let role = MosquittoRole {
            role_name: value(&self.role_name_editor, cx),
            text_name: optional_value(&self.role_text_name, cx),
            text_description: optional_value(&self.role_text_description, cx),
            allow_wildcards_subscriptions: self.role_allow_wildcards,
            acls: parse_acls(&value(&self.role_acls, cx))?,
        };
        role.validate()?;
        Ok(role)
    }

    fn save_client(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let client = match self.client_from_editor(cx) {
            Ok(client) => client,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let service = self.service.clone();
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.saving_management = true;
        self.notice = Some(("正在保存 Mosquitto 用户…".into(), false));
        cx.spawn(async move |this, cx| {
            let outcome = match service.save_client(&profile, &client).await {
                Err(error) => Err((false, error)),
                Ok(()) => match service.dynamic_security_snapshot(&profile).await {
                    Ok(snapshot) => Ok(snapshot),
                    Err(error) => Err((true, error)),
                },
            };
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.saving_management = false;
                match outcome {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.notice = Some(("Mosquitto 用户已保存".into(), false));
                    }
                    Err((saved, error)) => {
                        let message = error.user_message().to_string();
                        this.management_error = Some(message.clone());
                        this.notice = Some((
                            if saved {
                                format!("用户已保存，但刷新列表失败：{message}")
                            } else {
                                format!("保存用户失败：{message}")
                            },
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save_group(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let group = match self.group_from_editor(cx) {
            Ok(group) => group,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let service = self.service.clone();
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.saving_management = true;
        self.notice = Some(("正在保存 Mosquitto Group…".into(), false));
        cx.spawn(async move |this, cx| {
            let outcome = match service.save_group(&profile, &group).await {
                Err(error) => Err((false, error)),
                Ok(()) => match service.dynamic_security_snapshot(&profile).await {
                    Ok(snapshot) => Ok(snapshot),
                    Err(error) => Err((true, error)),
                },
            };
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.saving_management = false;
                match outcome {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.notice = Some(("Mosquitto Group 已保存".into(), false));
                    }
                    Err((saved, error)) => {
                        let message = error.user_message().to_string();
                        this.notice = Some((
                            if saved {
                                format!("Group 已保存，但刷新列表失败：{message}")
                            } else {
                                format!("保存 Group 失败：{message}")
                            },
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save_role(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let role = match self.role_from_editor(cx) {
            Ok(role) => role,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let service = self.service.clone();
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.saving_management = true;
        self.notice = Some(("正在保存 Mosquitto Role…".into(), false));
        cx.spawn(async move |this, cx| {
            let outcome = match service.save_role(&profile, &role).await {
                Err(error) => Err((false, error)),
                Ok(()) => match service.dynamic_security_snapshot(&profile).await {
                    Ok(snapshot) => Ok(snapshot),
                    Err(error) => Err((true, error)),
                },
            };
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.saving_management = false;
                match outcome {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.notice = Some(("Mosquitto Role 已保存".into(), false));
                    }
                    Err((saved, error)) => {
                        let message = error.user_message().to_string();
                        this.notice = Some((
                            if saved {
                                format!("Role 已保存，但刷新列表失败：{message}")
                            } else {
                                format!("保存 Role 失败：{message}")
                            },
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn confirm_delete_client(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(username) = self.selected_client_username.clone() else {
            self.notice = Some(("请先选择要删除的用户".into(), true));
            cx.notify();
            return;
        };
        let entity = cx.entity().clone();
        ramag_ui::open_confirm(
            "删除 Mosquitto 用户",
            format!("将从 Dynamic Security 删除用户「{username}」，此操作不可撤销。"),
            "删除用户",
            true,
            move |_, app| {
                entity.update(app, |this, cx| this.delete_client(username, cx));
            },
            window,
            cx,
        );
    }

    fn delete_client(&mut self, username: String, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            return;
        };
        let service = self.service.clone();
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.deleting_management = true;
        self.notice = Some(("正在删除 Mosquitto 用户…".into(), false));
        cx.spawn(async move |this, cx| {
            let outcome = match service.delete_client(&profile, &username).await {
                Err(error) => Err((false, error)),
                Ok(()) => match service.dynamic_security_snapshot(&profile).await {
                    Ok(snapshot) => Ok(snapshot),
                    Err(error) => Err((true, error)),
                },
            };
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.deleting_management = false;
                match outcome {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.selected_client_username = None;
                        this.notice = Some(("Mosquitto 用户已删除".into(), false));
                    }
                    Err((saved, error)) => {
                        let message = error.user_message().to_string();
                        this.notice = Some((
                            if saved {
                                format!("用户已删除，但刷新列表失败：{message}")
                            } else {
                                format!("删除用户失败：{message}")
                            },
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn confirm_delete_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(group_name) = self.selected_group_name.clone() else {
            self.notice = Some(("请先选择要删除的 Group".into(), true));
            cx.notify();
            return;
        };
        let entity = cx.entity().clone();
        ramag_ui::open_confirm(
            "删除 Mosquitto Group",
            format!("将从 Dynamic Security 删除 Group「{group_name}」，此操作不可撤销。"),
            "删除 Group",
            true,
            move |_, app| {
                entity.update(app, |this, cx| this.delete_group(group_name, cx));
            },
            window,
            cx,
        );
    }

    fn delete_group(&mut self, group_name: String, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            return;
        };
        let service = self.service.clone();
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.deleting_management = true;
        self.notice = Some(("正在删除 Mosquitto Group…".into(), false));
        cx.spawn(async move |this, cx| {
            let outcome = match service.delete_group(&profile, &group_name).await {
                Err(error) => Err((false, error)),
                Ok(()) => match service.dynamic_security_snapshot(&profile).await {
                    Ok(snapshot) => Ok(snapshot),
                    Err(error) => Err((true, error)),
                },
            };
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.deleting_management = false;
                match outcome {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.selected_group_name = None;
                        this.notice = Some(("Mosquitto Group 已删除".into(), false));
                    }
                    Err((saved, error)) => {
                        let message = error.user_message().to_string();
                        this.notice = Some((
                            if saved {
                                format!("Group 已删除，但刷新列表失败：{message}")
                            } else {
                                format!("删除 Group 失败：{message}")
                            },
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn confirm_delete_role(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(role_name) = self.selected_role_name.clone() else {
            self.notice = Some(("请先选择要删除的 Role".into(), true));
            cx.notify();
            return;
        };
        let entity = cx.entity().clone();
        ramag_ui::open_confirm(
            "删除 Mosquitto Role",
            format!("将从 Dynamic Security 删除 Role「{role_name}」及其 ACL，此操作不可撤销。"),
            "删除 Role",
            true,
            move |_, app| {
                entity.update(app, |this, cx| this.delete_role(role_name, cx));
            },
            window,
            cx,
        );
    }

    fn delete_role(&mut self, role_name: String, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            return;
        };
        let service = self.service.clone();
        self.management_operation_id = self.management_operation_id.wrapping_add(1);
        let operation_id = self.management_operation_id;
        self.deleting_management = true;
        self.notice = Some(("正在删除 Mosquitto Role…".into(), false));
        cx.spawn(async move |this, cx| {
            let outcome = match service.delete_role(&profile, &role_name).await {
                Err(error) => Err((false, error)),
                Ok(()) => match service.dynamic_security_snapshot(&profile).await {
                    Ok(snapshot) => Ok(snapshot),
                    Err(error) => Err((true, error)),
                },
            };
            let _ = this.update(cx, |this, cx| {
                if this.management_operation_id != operation_id {
                    return;
                }
                this.deleting_management = false;
                match outcome {
                    Ok(snapshot) => {
                        this.management_snapshot = Some(snapshot);
                        this.selected_role_name = None;
                        this.notice = Some(("Mosquitto Role 已删除".into(), false));
                    }
                    Err((saved, error)) => {
                        let message = error.user_message().to_string();
                        this.notice = Some((
                            if saved {
                                format!("Role 已删除，但刷新列表失败：{message}")
                            } else {
                                format!("删除 Role 失败：{message}")
                            },
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_static_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let kind = self.static_file_kind;
        self.loading_static_file = true;
        self.static_file = None;
        self.notice = Some((format!("正在读取 Mosquitto {}…", kind.label()), false));
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.read_static_file(&profile, kind).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.loading_static_file = false;
                match result {
                    Ok(file) => {
                        set_value(&this.static_content, file.content.clone(), window, cx);
                        this.static_file = Some(file);
                        this.notice = Some(("Mosquitto 静态文件读取完成".into(), false));
                    }
                    Err(error) => {
                        this.notice = Some((
                            format!("读取 Mosquitto 静态文件失败：{}", error.user_message()),
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save_static_file(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择 MQTT 配置".into(), true));
            cx.notify();
            return;
        };
        let Some(mut file) = self.static_file.clone() else {
            self.notice = Some(("请先读取要保存的 Mosquitto 静态文件".into(), true));
            cx.notify();
            return;
        };
        file.content = value(&self.static_content, cx);
        if let Err(error) = file.validate() {
            self.notice = Some((error, true));
            cx.notify();
            return;
        }
        let service = self.service.clone();
        self.saving_static_file = true;
        self.notice = Some(("正在保存 Mosquitto 静态文件…".into(), false));
        cx.spawn(async move |this, cx| {
            let result = service.write_static_file(&profile, &file).await;
            let _ = this.update(cx, |this, cx| {
                this.saving_static_file = false;
                match result {
                    Ok(()) => {
                        this.static_file = Some(file);
                        this.notice = Some((
                            "Mosquitto 静态文件已保存；Broker 可能需要 reload 或重启才能使用新内容"
                                .into(),
                            false,
                        ));
                    }
                    Err(error) => {
                        this.notice = Some((
                            format!("保存 Mosquitto 静态文件失败：{}", error.user_message()),
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn new_client(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_client_username = None;
        self.client_disabled = false;
        for field in [
            &self.client_username,
            &self.client_id_editor,
            &self.client_password,
            &self.client_text_name,
            &self.client_text_description,
            &self.client_groups,
            &self.client_roles,
        ] {
            set_value(field, "", window, cx);
        }
        cx.notify();
    }

    fn new_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_group_name = None;
        for field in [
            &self.group_name_editor,
            &self.group_text_name,
            &self.group_text_description,
            &self.group_roles,
        ] {
            set_value(field, "", window, cx);
        }
        cx.notify();
    }

    fn new_role(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_role_name = None;
        self.role_allow_wildcards = false;
        for field in [
            &self.role_name_editor,
            &self.role_text_name,
            &self.role_text_description,
            &self.role_acls,
        ] {
            set_value(field, "", window, cx);
        }
        cx.notify();
    }

    fn select_section(&mut self, section: MqttSection, cx: &mut Context<Self>) {
        if self.section != section {
            self.section = section;
            cx.notify();
        }
    }

    fn render_sidebar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let narrow = Self::sidebar_is_narrow(window);
        let search = value(&self.search, cx);
        let mut rows = v_flex().w_full().min_h_0();
        for profile in self.profiles.iter().filter(|profile| {
            search.is_empty() || profile.name.contains(&search) || profile.host.contains(&search)
        }) {
            let selected = self.selected_profile_id.as_ref() == Some(&profile.id);
            let id = profile.id.clone();
            rows = rows.child(
                h_flex()
                    .id(SharedString::from(format!("mqtt-profile-{}", profile.id)))
                    .w_full()
                    .min_w_0()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(10.0))
                    .when(selected, |row| row.bg(theme.accent.opacity(0.12)))
                    .when(!selected, |row| {
                        row.hover(|row| row.bg(theme.muted.opacity(0.45)))
                    })
                    .when(!self.is_busy(), |row| {
                        row.cursor_pointer().on_click(cx.listener(
                            move |this, _: &ClickEvent, window, cx| {
                                this.select_profile(id.clone(), window, cx);
                            },
                        ))
                    })
                    .child(div().size(px(8.0)).rounded_full().bg(if selected {
                        theme.accent
                    } else {
                        theme.muted_foreground
                    }))
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(div().text_sm().truncate().child(profile.name.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(format!("{}:{}", profile.host, profile.port)),
                            ),
                    ),
            );
        }
        v_flex()
            .id("mqtt-sidebar")
            .debug_selector(|| "mqtt-sidebar".into())
            .w(px(MQTT_SIDEBAR_WIDTH))
            .min_w(px(210.0))
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.secondary.opacity(0.45))
            .child(
                h_flex()
                    .w_full()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .px(px(14.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(Icon::new(IconName::Network).text_color(theme.accent))
                            .child(
                                v_flex()
                                    .gap(px(1.0))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child("MQTT"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child("Broker 配置"),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap(px(4.0))
                            .when(narrow, |actions| {
                                actions.child(
                                    ramag_ui::clickable_button("mqtt-hide-sidebar")
                                        .debug_selector(|| "mqtt-hide-sidebar".into())
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::PanelLeft)
                                        .tooltip("隐藏配置栏")
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.sidebar_visible = false;
                                            cx.notify();
                                        })),
                                )
                            })
                            .child(
                                ramag_ui::clickable_button("mqtt-add-profile")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Plus)
                                    .tooltip("新建配置")
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.new_profile(window, cx)
                                    })),
                            ),
                    ),
            )
            .child(
                div().w_full().flex_none().px(px(12.0)).py(px(10.0)).child(
                    ramag_ui::cleanable_input(&self.search, "mqtt-search-clear", false, cx)
                        .small()
                        .prefix(
                            Icon::new(IconName::Search)
                                .small()
                                .text_color(theme.muted_foreground),
                        ),
                ),
            )
            .child(
                div()
                    .id("mqtt-profile-list-scroll")
                    .w_full()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(rows),
            )
            .child(
                div()
                    .w_full()
                    .flex_none()
                    .px(px(12.0))
                    .py(px(12.0))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(div().text_xs().text_color(theme.muted_foreground).child(
                        if self.loading_profiles {
                            "正在加载本地配置…".to_string()
                        } else {
                            format!("{} 个本地配置", self.profiles.len())
                        },
                    )),
            )
            .when(narrow, |sidebar| {
                sidebar
                    .w_full()
                    .min_w_0()
                    .h(px(220.0))
                    .border_r_0()
                    .border_b_1()
            })
            .when(
                !narrow && window.viewport_size().width < px(760.0),
                |sidebar| sidebar.w(px(210.0)),
            )
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tabs = h_flex().flex_wrap().gap(px(4.0));
        for section in MqttSection::ALL {
            let mut button =
                ramag_ui::clickable_button(SharedString::from(format!("mqtt-tab-{:?}", section)))
                    .xsmall()
                    .label(section.label());
            button = if self.section == section {
                button.primary()
            } else {
                button.ghost()
            };
            tabs = tabs.child(button.on_click(
                cx.listener(move |this, _: &ClickEvent, _, cx| this.select_section(section, cx)),
            ));
        }
        tabs
    }

    fn render_header(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let narrow = Self::sidebar_is_narrow(window);
        let selected_name = self.selected_profile().map_or_else(
            || "新建 MQTT 配置".to_string(),
            |profile| profile.name.clone(),
        );
        let mut actions = h_flex()
            .flex_wrap()
            .items_center()
            .justify_end()
            .gap(px(6.0));
        if self.selected_profile_id.is_some() {
            actions = actions.child(
                ramag_ui::clickable_button("mqtt-delete-profile")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Delete)
                    .tooltip("删除配置")
                    .disabled(self.is_busy() || self.subscription_running)
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.delete_profile(window, cx)
                    })),
            );
        }
        actions = actions
            .child(
                ramag_ui::clickable_button("mqtt-test-connection")
                    .ghost()
                    .small()
                    .label("测试连接")
                    .loading(self.testing)
                    .disabled(self.is_busy() || self.subscription_running)
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.test_connection(window, cx)
                    })),
            )
            .child(
                ramag_ui::clickable_button("mqtt-save-profile")
                    .primary()
                    .small()
                    .label("保存")
                    .loading(self.saving)
                    .disabled(self.is_busy() || self.subscription_running)
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.save_profile(window, cx)
                    })),
            )
            .when(narrow && !self.sidebar_visible, |actions| {
                actions.child(
                    ramag_ui::clickable_button("mqtt-show-sidebar")
                        .debug_selector(|| "mqtt-show-sidebar".into())
                        .ghost()
                        .small()
                        .icon(IconName::PanelLeft)
                        .tooltip("显示配置栏")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.sidebar_visible = true;
                            cx.notify();
                        })),
                )
            });
        v_flex()
            .w_full()
            .flex_none()
            .gap(px(10.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.secondary.opacity(0.35))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .when(window.viewport_size().width < px(900.0), |row| {
                        row.flex_col().items_stretch()
                    })
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(selected_name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("真实 Broker 数据由 Native MQTT 驱动返回"),
                            ),
                    )
                    .child(actions),
            )
            .child(self.render_tabs(cx))
    }

    fn render_config(&self, _window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut protocol_buttons = h_flex()
            .debug_selector(|| "mqtt-protocol-buttons".into())
            .gap(px(4.0));
        for (protocol, label) in [
            (MqttProtocolVersion::V5, "MQTT 5.0"),
            (MqttProtocolVersion::V311, "MQTT 3.1.1"),
        ] {
            let mut button =
                ramag_ui::clickable_button(SharedString::from(format!("mqtt-protocol-{label}")))
                    .xsmall()
                    .label(label);
            button = if self.protocol == protocol {
                button.primary()
            } else {
                button.ghost()
            };
            protocol_buttons = protocol_buttons.child(button.on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.protocol = protocol;
                    cx.notify();
                },
            )));
        }
        let mut transport_buttons = h_flex().gap(px(4.0));
        for (transport, label) in [(TransportKind::Tcp, "TCP"), (TransportKind::Tls, "TLS")] {
            let mut button =
                ramag_ui::clickable_button(SharedString::from(format!("mqtt-transport-{label}")))
                    .xsmall()
                    .label(label);
            button = if self.transport == transport {
                button.primary()
            } else {
                button.ghost()
            };
            transport_buttons = transport_buttons.child(button.on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.transport = transport;
                    cx.notify();
                },
            )));
        }
        let content = v_flex()
            .w_full()
            .max_w(px(920.0))
            .gap(px(14.0))
            .child(section_heading(
                "连接配置",
                "凭据和证书只通过本机加密存储保存",
                &theme,
            ))
            .child(
                row()
                    .child(field("名称", Input::new(&self.name).small()))
                    .child(field("Broker 地址", Input::new(&self.host).small()))
                    .child(field("端口", Input::new(&self.port).small())),
            )
            .child(
                row()
                    .child(
                        field("协议版本", protocol_buttons)
                            .debug_selector(|| "mqtt-protocol-field".into()),
                    )
                    .child(field("传输", transport_buttons))
                    .child(field("Keep Alive", Input::new(&self.keep_alive).small())),
            )
            .child(
                row()
                    .child(field("Client ID", Input::new(&self.client_id).small()))
                    .child(field("用户名", Input::new(&self.username).small()))
                    .child(field(
                        "密码",
                        Input::new(&self.password).small().mask_toggle(),
                    )),
            )
            .child(section_heading(
                "TLS 文件",
                "只有 TLS 传输会读取这些路径；客户端证书和密钥必须成对配置",
                &theme,
            ))
            .child(
                row()
                    .child(field("CA 证书", Input::new(&self.ca_cert_path).small()))
                    .child(field(
                        "客户端证书",
                        Input::new(&self.client_cert_path).small(),
                    ))
                    .child(field(
                        "客户端密钥",
                        Input::new(&self.client_key_path).small(),
                    )),
            )
            .when(self.management_enabled, |content| {
                content
                    .child(section_heading(
                        "Mosquitto 管理凭据",
                        "Dynamic Security 请求优先使用这里的账号；密码不会从 Broker 读取或回显",
                        &theme,
                    ))
                    .child(
                        row()
                            .child(field(
                                "管理用户名",
                                Input::new(&self.management_admin_username).small(),
                            ))
                            .child(field(
                                "管理密码",
                                Input::new(&self.management_admin_password)
                                    .small()
                                    .mask_toggle(),
                            )),
                    )
                    .child(section_heading(
                        "静态配置文件",
                        "配置绝对路径后，Mosquitto 页面才能读取或保存本机文件",
                        &theme,
                    ))
                    .child(
                        row()
                            .child(field(
                                "password_file",
                                Input::new(&self.password_file_path).small(),
                            ))
                            .child(field("acl_file", Input::new(&self.acl_file_path).small())),
                    )
            })
            .child(
                row()
                    .child(toggle_button(
                        "mqtt-clean-start",
                        "Clean Start",
                        self.clean_start,
                        self.is_busy(),
                        cx,
                        |this| this.clean_start = !this.clean_start,
                    ))
                    .child(toggle_button(
                        "mqtt-management",
                        "启用 Mosquitto 管理",
                        self.management_enabled,
                        self.is_busy(),
                        cx,
                        |this| this.management_enabled = !this.management_enabled,
                    ))
                    .child(div().flex_1().min_w_0()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("保存后才能在状态、发布、订阅和 Mosquitto 页面使用此配置。"),
            );
        div()
            .id("mqtt-config-scroll")
            .w_full()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p(px(16.0))
            .child(content)
    }

    fn render_overview(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let capabilities = self.service.transport_capabilities();
        let mut capabilities_view = v_flex().gap(px(5.0));
        for (label, enabled) in capability_items(capabilities) {
            capabilities_view = capabilities_view.child(
                h_flex()
                    .gap(px(8.0))
                    .child(
                        Icon::new(if enabled {
                            IconName::CircleCheck
                        } else {
                            IconName::CircleX
                        })
                        .small()
                        .text_color(if enabled {
                            theme.accent
                        } else {
                            theme.muted_foreground
                        }),
                    )
                    .child(div().text_xs().child(label)),
            );
        }
        let snapshot_body = if self.loading_snapshot {
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("正在读取 Broker 状态…")
                .into_any_element()
        } else if let Some(error) = &self.snapshot_error {
            div()
                .text_sm()
                .text_color(theme.danger)
                .child(error.clone())
                .into_any_element()
        } else if let Some(snapshot) = &self.snapshot {
            v_flex()
                .gap(px(6.0))
                .children([
                    div().text_sm().child(format!(
                        "观察到 {} 个 Topic，{} 个在线客户端",
                        snapshot.topics.len(),
                        snapshot.online_clients.len()
                    )),
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "Topic 数据{}完整；在线客户端数据{}完整。",
                            if snapshot.topics_complete { "" } else { "不" },
                            if snapshot.online_clients_complete {
                                ""
                            } else {
                                "不"
                            }
                        )),
                ])
                .into_any_element()
        } else {
            div().text_sm().text_color(theme.muted_foreground).child("尚未读取 Broker 状态；标准 MQTT 不提供完整 Topic 目录，驱动会明确标记数据是否完整。").into_any_element()
        };
        let mut body = v_flex().w_full().max_w(px(920.0)).gap(px(14.0));
        body = body
            .child(section_heading(
                "传输能力",
                "能力来自当前编译的 Native 驱动，不代表远端 Broker 已连接",
                &theme,
            ))
            .child(capabilities_view);
        body = body
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(section_heading(
                        "Broker 状态",
                        "只展示本次真实请求返回的观察结果",
                        &theme,
                    ))
                    .child(
                        ramag_ui::clickable_button("mqtt-refresh-snapshot")
                            .ghost()
                            .small()
                            .icon(ramag_ui::icons::refresh_cw())
                            .tooltip("读取 Broker 状态")
                            .disabled(self.loading_snapshot || self.selected_profile_id.is_none())
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.load_snapshot(window, cx)
                            })),
                    ),
            )
            .child(snapshot_body);
        if let Some(snapshot) = &self.snapshot {
            body = body.child(section_heading(
                "观察到的 Topic",
                "这些 Topic 来自驱动返回，不是 Broker 的完整目录",
                &theme,
            ));
            if snapshot.topics.is_empty() {
                body = body.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("本次没有观察到 Topic。"),
                );
            } else {
                let mut topics = v_flex().gap(px(3.0));
                for topic in &snapshot.topics {
                    topics = topics.child(
                        h_flex()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_xs()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(topic.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{:?}", topic.source)),
                            ),
                    );
                }
                body = body.child(topics);
            }
        }
        div()
            .id("mqtt-overview-scroll")
            .w_full()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p(px(16.0))
            .child(body)
            .when(window.viewport_size().width < px(760.0), |view| {
                view.p(px(10.0))
            })
    }

    fn render_publish(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        v_flex()
            .w_full()
            .max_w(px(920.0))
            .gap(px(12.0))
            .child(section_heading(
                "发布消息",
                "消息通过当前配置连接远端 Broker；没有成功返回就不会显示为已发布",
                &theme,
            ))
            .child(field("Topic", Input::new(&self.publish_topic).small()))
            .child(field(
                "Payload（UTF-8）",
                Input::new(&self.publish_payload).h(px(140.0)).small(),
            ))
            .child(
                ramag_ui::clickable_button("mqtt-publish")
                    .primary()
                    .small()
                    .label("发布消息")
                    .loading(self.publishing)
                    .disabled(self.publishing || self.subscription_running)
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| this.publish(window, cx)),
                    ),
            )
    }

    fn render_subscribe(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut body = v_flex()
            .w_full()
            .max_w(px(920.0))
            .gap(px(12.0))
            .child(section_heading(
                "订阅消息",
                "订阅使用有界缓冲；缓冲满时驱动会报告背压，不会无限堆积内存",
                &theme,
            ))
            .child(field(
                "Topic Filter",
                Input::new(&self.subscribe_filter).small(),
            ));
        let action = if self.subscription_running {
            ramag_ui::clickable_button("mqtt-stop-subscription")
                .danger()
                .small()
                .label("停止订阅")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.stop_subscription();
                    this.notice = Some(("正在停止订阅…".into(), false));
                    cx.notify();
                }))
        } else {
            ramag_ui::clickable_button("mqtt-start-subscription")
                .primary()
                .small()
                .label("开始订阅")
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.start_subscription(window, cx)
                }))
        };
        body = body.child(action);
        if self.messages.is_empty() {
            body = body.child(div().text_sm().text_color(theme.muted_foreground).child(
                if self.subscription_running {
                    "等待消息…"
                } else {
                    "尚未收到消息。"
                },
            ));
        } else {
            let mut messages = v_flex().gap(px(6.0));
            for message in self.messages.iter().rev() {
                let payload = String::from_utf8_lossy(&message.payload);
                messages = messages.child(
                    v_flex()
                        .gap(px(3.0))
                        .p(px(10.0))
                        .border_1()
                        .border_color(theme.border)
                        .rounded(px(5.0))
                        .child(
                            h_flex()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .truncate()
                                        .child(message.topic.clone()),
                                )
                                .child(div().text_xs().text_color(theme.muted_foreground).child(
                                    format!(
                                        "QoS {} · {}",
                                        message.qos.as_u8(),
                                        message.received_at
                                    ),
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .whitespace_normal()
                                .child(payload.to_string()),
                        ),
                );
            }
            body = body.child(messages);
        }
        v_flex()
            .id("mqtt-subscribe-scroll")
            .w_full()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p(px(16.0))
            .child(body)
    }

    fn render_management_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tabs = h_flex().flex_wrap().gap(px(4.0));
        for section in MosquittoManagementSection::ALL {
            let mut button = ramag_ui::clickable_button(SharedString::from(format!(
                "mqtt-management-tab-{section:?}"
            )))
            .xsmall()
            .label(section.label());
            button = if self.management_section == section {
                button.primary()
            } else {
                button.ghost()
            };
            tabs = tabs.child(
                button.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.select_management_section(section, cx)
                })),
            );
        }
        tabs
    }

    fn render_clients(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme().clone();
        let mut list = v_flex().gap(px(4.0));
        if let Some(snapshot) = &self.management_snapshot {
            for client in &snapshot.clients {
                let username = client.username.clone();
                let selected = self.selected_client_username.as_ref() == Some(&username);
                list = list.child(
                    h_flex()
                        .id(SharedString::from(format!("mqtt-client-{username}")))
                        .w_full()
                        .gap(px(8.0))
                        .p(px(8.0))
                        .rounded(px(4.0))
                        .when(selected, |row| row.bg(theme.accent.opacity(0.12)))
                        .when(!selected, |row| {
                            row.hover(|row| row.bg(theme.muted.opacity(0.45)))
                        })
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.select_client(username.clone(), window, cx)
                        }))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_xs().truncate().child(client.username.clone()))
                                .child(div().text_xs().text_color(theme.muted_foreground).child(
                                    format!(
                                        "{} 个 Group · {} 个 Role · 密码{}",
                                        client.groups.len(),
                                        client.roles.len(),
                                        if client.password_configured {
                                            "已配置"
                                        } else {
                                            "未配置"
                                        }
                                    ),
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(if client.disabled {
                                    theme.danger
                                } else {
                                    theme.muted_foreground
                                })
                                .child(if client.disabled {
                                    "已禁用"
                                } else {
                                    "启用"
                                }),
                        ),
                );
            }
        }
        let editor = v_flex()
            .w_full()
            .gap(px(8.0))
            .child(
                h_flex()
                    .justify_between()
                    .child(section_heading(
                        "用户编辑",
                        "Group 和 Role 使用逗号分隔；密码只在提交时发送",
                        &theme,
                    ))
                    .child(
                        ramag_ui::clickable_button("mqtt-client-new")
                            .ghost()
                            .xsmall()
                            .label("新建用户")
                            .disabled(self.saving_management || self.deleting_management)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.new_client(window, cx)
                            })),
                    ),
            )
            .child(
                row()
                    .child(field("用户名", Input::new(&self.client_username).small()))
                    .child(field(
                        "Client ID",
                        Input::new(&self.client_id_editor).small(),
                    ))
                    .child(field(
                        "新密码",
                        Input::new(&self.client_password).small().mask_toggle(),
                    )),
            )
            .child(
                row()
                    .child(field(
                        "显示名称",
                        Input::new(&self.client_text_name).small(),
                    ))
                    .child(field(
                        "描述",
                        Input::new(&self.client_text_description).small(),
                    )),
            )
            .child(
                row()
                    .child(field("Group", Input::new(&self.client_groups).small()))
                    .child(field("Role", Input::new(&self.client_roles).small())),
            )
            .child(toggle_button(
                "mqtt-client-disabled",
                "用户状态",
                self.client_disabled,
                self.saving_management || self.deleting_management,
                cx,
                |this| this.client_disabled = !this.client_disabled,
            ))
            .child(
                h_flex()
                    .gap(px(6.0))
                    .child(
                        ramag_ui::clickable_button("mqtt-client-save")
                            .primary()
                            .small()
                            .label("保存用户")
                            .loading(self.saving_management)
                            .disabled(self.saving_management || self.deleting_management)
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| this.save_client(cx)),
                            ),
                    )
                    .child(
                        ramag_ui::clickable_button("mqtt-client-delete")
                            .danger()
                            .small()
                            .label("删除用户")
                            .disabled(
                                self.selected_client_username.is_none()
                                    || self.saving_management
                                    || self.deleting_management,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.confirm_delete_client(window, cx)
                            })),
                    ),
            );
        let has_clients = self
            .management_snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.clients.is_empty());
        v_flex()
            .w_full()
            .gap(px(12.0))
            .child(section_heading(
                "用户列表",
                "来自 Dynamic Security 的真实用户和绑定关系",
                &theme,
            ))
            .child(if !has_clients {
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("暂无用户或尚未读取列表")
                    .into_any_element()
            } else {
                list.into_any_element()
            })
            .child(editor)
            .child(self.render_client_permissions(cx))
            .into_any_element()
    }

    fn render_client_permissions(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme().clone();
        let Some(snapshot) = self.management_snapshot.as_ref() else {
            return div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("读取 Dynamic Security 后才能展开用户权限")
                .into_any_element();
        };
        let Some(username) = self.selected_client_username.as_deref() else {
            return div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("选择用户后显示直接 Role 和 Group 继承的 ACL")
                .into_any_element();
        };
        let Some(client) = snapshot
            .clients
            .iter()
            .find(|client| client.username == username)
        else {
            return div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("当前用户已不在最近一次 Broker 返回的列表中")
                .into_any_element();
        };

        let (rows, missing_roles) = client_permission_rows(snapshot, client);
        let mut body = v_flex().w_full().gap(px(4.0));
        if rows.is_empty() {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("该用户当前没有可展开的 ACL"),
            );
        } else {
            body = body.child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap(px(8.0))
                    .child(div().w(px(130.0)).text_xs().child("来源"))
                    .child(div().w(px(130.0)).text_xs().child("Role"))
                    .child(div().flex_1().min_w_0().text_xs().child("Topic"))
                    .child(div().w(px(180.0)).text_xs().child("权限 / 优先级")),
            );
            for row in rows {
                let decision = match row.acl.decision {
                    MosquittoAclDecision::Allow => "允许",
                    MosquittoAclDecision::Deny => "拒绝",
                };
                body = body.child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .gap(px(8.0))
                        .child(
                            div()
                                .w(px(130.0))
                                .text_xs()
                                .truncate()
                                .text_color(theme.muted_foreground)
                                .child(row.source),
                        )
                        .child(div().w(px(130.0)).text_xs().truncate().child(row.role_name))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_xs()
                                .truncate()
                                .child(row.acl.topic),
                        )
                        .child(
                            div()
                                .w(px(180.0))
                                .text_xs()
                                .truncate()
                                .text_color(
                                    if matches!(row.acl.decision, MosquittoAclDecision::Allow) {
                                        theme.accent
                                    } else {
                                        theme.danger
                                    },
                                )
                                .child(format!(
                                    "{} · {} · ACL {} / 绑定 {}",
                                    row.acl.acl_type.as_str(),
                                    decision,
                                    row.acl.priority,
                                    row.binding_priority
                                )),
                        ),
                );
            }
        }
        if !missing_roles.is_empty() {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(theme.danger)
                    .child(format!("未找到绑定的 Role：{}", missing_roles.join("、"))),
            );
        }
        v_flex()
            .w_full()
            .gap(px(8.0))
            .child(section_heading(
                "用户权限预览",
                "按用户直接绑定和 Group 继承的 Role 展开；最终判定仍由 Mosquitto Broker 执行",
                &theme,
            ))
            .child(body)
            .into_any_element()
    }

    fn render_groups(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme().clone();
        let mut list = v_flex().gap(px(4.0));
        if let Some(snapshot) = &self.management_snapshot {
            for group in &snapshot.groups {
                let group_name = group.group_name.clone();
                let selected = self.selected_group_name.as_ref() == Some(&group_name);
                list = list.child(
                    h_flex()
                        .id(SharedString::from(format!("mqtt-group-{group_name}")))
                        .w_full()
                        .gap(px(8.0))
                        .p(px(8.0))
                        .rounded(px(4.0))
                        .when(selected, |row| row.bg(theme.accent.opacity(0.12)))
                        .when(!selected, |row| {
                            row.hover(|row| row.bg(theme.muted.opacity(0.45)))
                        })
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.select_group(group_name.clone(), window, cx)
                        }))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_xs().truncate().child(group.group_name.clone()))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(format!("{} 个 Role", group.roles.len())),
                                ),
                        ),
                );
            }
        }
        let has_groups = self
            .management_snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.groups.is_empty());
        v_flex()
            .w_full()
            .gap(px(12.0))
            .child(section_heading(
                "Group 列表",
                "Group 只保存 Role 绑定，用户绑定由用户编辑器维护",
                &theme,
            ))
            .child(if !has_groups {
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("暂无 Group 或尚未读取列表")
                    .into_any_element()
            } else {
                list.into_any_element()
            })
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(
                        h_flex()
                            .justify_between()
                            .child(section_heading(
                                "Group 编辑",
                                "Role 名称使用逗号分隔",
                                &theme,
                            ))
                            .child(
                                ramag_ui::clickable_button("mqtt-group-new")
                                    .ghost()
                                    .xsmall()
                                    .label("新建 Group")
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.new_group(window, cx)
                                    })),
                            ),
                    )
                    .child(
                        row()
                            .child(field(
                                "Group 名称",
                                Input::new(&self.group_name_editor).small(),
                            ))
                            .child(field("显示名称", Input::new(&self.group_text_name).small()))
                            .child(field("Role", Input::new(&self.group_roles).small())),
                    )
                    .child(field(
                        "描述",
                        Input::new(&self.group_text_description).small(),
                    ))
                    .child(
                        h_flex()
                            .gap(px(6.0))
                            .child(
                                ramag_ui::clickable_button("mqtt-group-save")
                                    .primary()
                                    .small()
                                    .label("保存 Group")
                                    .loading(self.saving_management)
                                    .disabled(self.saving_management || self.deleting_management)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.save_group(cx)
                                    })),
                            )
                            .child(
                                ramag_ui::clickable_button("mqtt-group-delete")
                                    .danger()
                                    .small()
                                    .label("删除 Group")
                                    .disabled(
                                        self.selected_group_name.is_none()
                                            || self.saving_management
                                            || self.deleting_management,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.confirm_delete_group(window, cx)
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_roles(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme().clone();
        let mut list = v_flex().gap(px(4.0));
        if let Some(snapshot) = &self.management_snapshot {
            for role in &snapshot.roles {
                let role_name = role.role_name.clone();
                let selected = self.selected_role_name.as_ref() == Some(&role_name);
                list = list.child(
                    h_flex()
                        .id(SharedString::from(format!("mqtt-role-{role_name}")))
                        .w_full()
                        .gap(px(8.0))
                        .p(px(8.0))
                        .rounded(px(4.0))
                        .when(selected, |row| row.bg(theme.accent.opacity(0.12)))
                        .when(!selected, |row| {
                            row.hover(|row| row.bg(theme.muted.opacity(0.45)))
                        })
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.select_role(role_name.clone(), window, cx)
                        }))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_xs().truncate().child(role.role_name.clone()))
                                .child(div().text_xs().text_color(theme.muted_foreground).child(
                                    format!(
                                        "{} 条 ACL · 通配符订阅{}",
                                        role.acls.len(),
                                        if role.allow_wildcards_subscriptions {
                                            "允许"
                                        } else {
                                            "禁止"
                                        }
                                    ),
                                )),
                        ),
                );
            }
        }
        let has_roles = self
            .management_snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.roles.is_empty());
        v_flex()
            .w_full()
            .gap(px(12.0))
            .child(section_heading("Role 列表", "ACL 来自 Dynamic Security；每行格式为 acltype|topic|allow/deny|priority", &theme))
            .child(if !has_roles {
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("暂无 Role 或尚未读取列表")
                    .into_any_element()
            } else {
                list.into_any_element()
            })
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(
                        h_flex()
                            .justify_between()
                            .child(section_heading(
                                "Role 与 ACL 编辑",
                                "支持 publishClientSend、publishClientReceive、subscribeLiteral、subscribePattern、unsubscribeLiteral、unsubscribePattern",
                                &theme,
                            ))
                            .child(
                                ramag_ui::clickable_button("mqtt-role-new")
                                    .ghost()
                                    .xsmall()
                                    .label("新建 Role")
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.new_role(window, cx)
                                    })),
                            ),
                    )
                    .child(
                        row()
                            .child(field("Role 名称", Input::new(&self.role_name_editor).small()))
                            .child(field("显示名称", Input::new(&self.role_text_name).small())),
                    )
                    .child(field(
                        "描述",
                        Input::new(&self.role_text_description).small(),
                    ))
                    .child(field(
                        "ACL",
                        Input::new(&self.role_acls).h(px(180.0)).small(),
                    ))
                    .child(toggle_button(
                        "mqtt-role-wildcards",
                        "通配符订阅",
                        self.role_allow_wildcards,
                        self.saving_management || self.deleting_management,
                        cx,
                        |this| this.role_allow_wildcards = !this.role_allow_wildcards,
                    ))
                    .child(
                        h_flex()
                            .gap(px(6.0))
                            .child(
                                ramag_ui::clickable_button("mqtt-role-save")
                                    .primary()
                                    .small()
                                    .label("保存 Role")
                                    .loading(self.saving_management)
                                    .disabled(self.saving_management || self.deleting_management)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.save_role(cx)
                                    })),
                            )
                            .child(
                                ramag_ui::clickable_button("mqtt-role-delete")
                                    .danger()
                                    .small()
                                    .label("删除 Role")
                                    .disabled(
                                        self.selected_role_name.is_none()
                                            || self.saving_management
                                            || self.deleting_management,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.confirm_delete_role(window, cx)
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_static_files(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme().clone();
        let mut kind_buttons = h_flex().gap(px(4.0));
        for kind in [
            MosquittoStaticFileKind::Password,
            MosquittoStaticFileKind::Acl,
        ] {
            let mut button = ramag_ui::clickable_button(SharedString::from(format!(
                "mqtt-static-kind-{:?}",
                kind
            )))
            .xsmall()
            .label(kind.label());
            button = if self.static_file_kind == kind {
                button.primary()
            } else {
                button.ghost()
            };
            kind_buttons = kind_buttons.child(button.on_click(cx.listener(
                move |this, _: &ClickEvent, window, cx| {
                    this.static_file_kind = kind;
                    this.static_file = None;
                    set_value(&this.static_content, "", window, cx);
                    cx.notify();
                },
            )));
        }
        let configured_path = self
            .selected_profile()
            .and_then(|profile| self.static_file_kind.configured_path(profile))
            .unwrap_or("未配置");
        let ssh_target = self.selected_profile().is_some_and(|profile| {
            matches!(
                profile
                    .management
                    .static_config
                    .as_ref()
                    .map(|config| &config.target),
                Some(MosquittoConfigTarget::Ssh { .. })
            )
        });
        v_flex()
            .w_full()
            .gap(px(10.0))
            .child(section_heading(
                "静态配置文件",
                "文件内容来自本机文件驱动；保存成功后仍需 Broker reload 或重启",
                &theme,
            ))
            .child(kind_buttons)
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!(
                        "当前 {} 路径：{}",
                        self.static_file_kind.label(),
                        configured_path
                    )),
            )
            .when(ssh_target, |body| {
                body.child(
                    div()
                        .text_xs()
                        .text_color(theme.danger)
                        .child("当前配置是 SSH 目标；本机文件驱动不会伪装成远程文件管理。"),
                )
            })
            .child(
                h_flex()
                    .gap(px(6.0))
                    .child(
                        ramag_ui::clickable_button("mqtt-static-load")
                            .ghost()
                            .small()
                            .label("读取文件")
                            .loading(self.loading_static_file)
                            .disabled(self.loading_static_file || self.saving_static_file)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.load_static_file(window, cx)
                            })),
                    )
                    .child(
                        ramag_ui::clickable_button("mqtt-static-save")
                            .primary()
                            .small()
                            .label("保存文件")
                            .loading(self.saving_static_file)
                            .disabled(
                                self.static_file.is_none()
                                    || self.loading_static_file
                                    || self.saving_static_file,
                            )
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.save_static_file(cx)
                                }),
                            ),
                    ),
            )
            .child(field(
                "文件内容",
                Input::new(&self.static_content).h(px(300.0)).small(),
            ))
            .into_any_element()
    }

    fn render_mosquitto(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut body = v_flex()
            .w_full()
            .max_w(px(980.0))
            .gap(px(12.0))
            .child(section_heading(
                "Mosquitto 管理",
                "Dynamic Security 和静态文件分别通过明确的管理接口读取，不生成演示数据",
                &theme,
            ));
        if !self.management_enabled {
            body = body.child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("当前配置未启用 Mosquitto 管理能力。"),
            );
        } else {
            body = body
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap(px(6.0))
                        .child(
                            ramag_ui::clickable_button("mqtt-load-management")
                                .ghost()
                                .small()
                                .label("读取 Dynamic Security")
                                .loading(self.loading_management)
                                .disabled(
                                    self.loading_management
                                        || self.saving_management
                                        || self.deleting_management
                                        || self.selected_profile_id.is_none(),
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.load_management(cx)
                                })),
                        )
                        .child(self.render_management_tabs(cx)),
                )
                .child(if let Some(error) = &self.management_error {
                    div()
                        .text_sm()
                        .text_color(theme.danger)
                        .child(error.clone())
                        .into_any_element()
                } else if self.loading_management && self.management_snapshot.is_none() {
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("正在读取 Dynamic Security…")
                        .into_any_element()
                } else if let Some(snapshot) = &self.management_snapshot {
                    let metrics = h_flex().flex_wrap().gap(px(8.0)).children([
                        metric("用户", snapshot.clients.len(), &theme),
                        metric("Group", snapshot.groups.len(), &theme),
                        metric("Role", snapshot.roles.len(), &theme),
                    ]);
                    let panel = match self.management_section {
                        MosquittoManagementSection::Clients => self.render_clients(cx),
                        MosquittoManagementSection::Groups => self.render_groups(cx),
                        MosquittoManagementSection::Roles => self.render_roles(cx),
                        MosquittoManagementSection::StaticFiles => self.render_static_files(cx),
                    };
                    v_flex()
                        .w_full()
                        .gap(px(12.0))
                        .child(metrics)
                        .child(panel)
                        .into_any_element()
                } else {
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("尚未读取 Mosquitto 管理数据。")
                        .into_any_element()
                });
        }
        v_flex()
            .id("mqtt-mosquitto-scroll")
            .w_full()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p(px(16.0))
            .child(body)
    }
}

impl Render for MqttView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let narrow = Self::sidebar_is_narrow(window);
        let main = v_flex()
            .id("mqtt-main")
            .debug_selector(|| "mqtt-main".into())
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(theme.background)
            .child(self.render_header(window, cx))
            .child(match self.section {
                MqttSection::Config => self.render_config(window, cx).into_any_element(),
                MqttSection::Overview => self.render_overview(window, cx).into_any_element(),
                MqttSection::Publish => self.render_publish(cx).into_any_element(),
                MqttSection::Subscribe => self.render_subscribe(cx).into_any_element(),
                MqttSection::Mosquitto => self.render_mosquitto(cx).into_any_element(),
            });
        h_flex()
            .id("mqtt-root")
            .debug_selector(|| "mqtt-root".into())
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(theme.background)
            .when(narrow && self.sidebar_visible, |root| {
                root.flex_col().items_stretch()
            })
            .when(!narrow || self.sidebar_visible, |root| {
                root.child(self.render_sidebar(window, cx))
            })
            .child(main)
    }
}

fn input(
    window: &mut Window,
    cx: &mut Context<MqttView>,
    max_bytes: usize,
    placeholder: &'static str,
    masked: bool,
    default_value: &'static str,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .validate(move |value, _| value.len() <= max_bytes)
            .placeholder(placeholder)
            .masked(masked)
            .default_value(default_value)
    })
}

fn set_value(
    field: &Entity<InputState>,
    value: impl Into<String>,
    window: &mut Window,
    cx: &mut Context<MqttView>,
) {
    field.update(cx, |state, cx| state.set_value(value.into(), window, cx));
}

fn value(field: &Entity<InputState>, cx: &App) -> String {
    field.read(cx).value().trim().to_string()
}

fn optional_value(field: &Entity<InputState>, cx: &App) -> Option<String> {
    let value = value(field, cx);
    (!value.is_empty()).then_some(value)
}

fn parse_group_bindings(text: &str) -> std::result::Result<Vec<MosquittoGroupBinding>, String> {
    parse_binding_names(text, "Group").map(|names| {
        names
            .into_iter()
            .map(|group_name| MosquittoGroupBinding {
                group_name,
                priority: -1,
            })
            .collect()
    })
}

fn client_permission_rows(
    snapshot: &MosquittoDynamicSecuritySnapshot,
    client: &MosquittoClient,
) -> (Vec<ClientPermissionRow>, Vec<String>) {
    let mut rows = Vec::new();
    let mut missing_roles = Vec::new();
    for binding in &client.roles {
        append_role_permissions(
            snapshot,
            &mut rows,
            &mut missing_roles,
            &binding.role_name,
            "直接 Role".into(),
            binding.priority,
        );
    }
    for group_binding in &client.groups {
        let Some(group) = snapshot
            .groups
            .iter()
            .find(|group| group.group_name == group_binding.group_name)
        else {
            continue;
        };
        for role_binding in &group.roles {
            append_role_permissions(
                snapshot,
                &mut rows,
                &mut missing_roles,
                &role_binding.role_name,
                format!("Group {}", group.group_name),
                role_binding.priority,
            );
        }
    }
    (rows, missing_roles)
}

fn append_role_permissions(
    snapshot: &MosquittoDynamicSecuritySnapshot,
    rows: &mut Vec<ClientPermissionRow>,
    missing_roles: &mut Vec<String>,
    role_name: &str,
    source: String,
    binding_priority: i32,
) {
    let Some(role) = snapshot
        .roles
        .iter()
        .find(|role| role.role_name == role_name)
    else {
        if !missing_roles.iter().any(|missing| missing == role_name) {
            missing_roles.push(role_name.to_string());
        }
        return;
    };
    rows.extend(role.acls.iter().cloned().map(|acl| ClientPermissionRow {
        source: source.clone(),
        role_name: role.role_name.clone(),
        binding_priority,
        acl,
    }));
}

fn parse_role_bindings(text: &str) -> std::result::Result<Vec<MosquittoRoleBinding>, String> {
    parse_binding_names(text, "Role").map(|names| {
        names
            .into_iter()
            .map(|role_name| MosquittoRoleBinding {
                role_name,
                priority: -1,
            })
            .collect()
    })
}

fn parse_binding_names(text: &str, label: &str) -> std::result::Result<Vec<String>, String> {
    let mut names = Vec::new();
    for name in text.split([',', '\n']) {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        if names.iter().any(|existing| existing == name) {
            return Err(format!("{label} 名称不能重复：{name}"));
        }
        names.push(name.to_string());
    }
    Ok(names)
}

fn join_group_bindings(bindings: &[MosquittoGroupBinding]) -> String {
    bindings
        .iter()
        .map(|binding| binding.group_name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn join_role_bindings(bindings: &[MosquittoRoleBinding]) -> String {
    bindings
        .iter()
        .map(|binding| binding.role_name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_acls(text: &str) -> std::result::Result<Vec<MosquittoAcl>, String> {
    let mut acls = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts = line.split('|').map(str::trim).collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(format!(
                "ACL 第 {} 行必须使用 acltype|topic|allow/deny|priority 格式",
                line_number + 1
            ));
        }
        let acl_type = parse_acl_type(parts[0])
            .ok_or_else(|| format!("ACL 第 {} 行的类型不受支持：{}", line_number + 1, parts[0]))?;
        let decision = match parts[2].to_ascii_lowercase().as_str() {
            "allow" | "true" => MosquittoAclDecision::Allow,
            "deny" | "false" => MosquittoAclDecision::Deny,
            _ => {
                return Err(format!(
                    "ACL 第 {} 行的决定必须是 allow 或 deny",
                    line_number + 1
                ));
            }
        };
        let priority = parts[3]
            .parse::<i32>()
            .map_err(|_| format!("ACL 第 {} 行的优先级必须是整数", line_number + 1))?;
        acls.push(MosquittoAcl {
            acl_type,
            topic: parts[1].to_string(),
            decision,
            priority,
        });
    }
    Ok(acls)
}

fn parse_acl_type(value: &str) -> Option<MosquittoAclType> {
    Some(match value {
        "publishClientSend" => MosquittoAclType::PublishClientSend,
        "publishClientReceive" => MosquittoAclType::PublishClientReceive,
        "subscribeLiteral" => MosquittoAclType::SubscribeLiteral,
        "subscribePattern" => MosquittoAclType::SubscribePattern,
        "unsubscribeLiteral" => MosquittoAclType::UnsubscribeLiteral,
        "unsubscribePattern" => MosquittoAclType::UnsubscribePattern,
        _ => return None,
    })
}

fn serialize_acls(acls: &[MosquittoAcl]) -> String {
    acls.iter()
        .map(|acl| {
            format!(
                "{}|{}|{}|{}",
                acl.acl_type.as_str(),
                acl.topic,
                match acl.decision {
                    MosquittoAclDecision::Allow => "allow",
                    MosquittoAclDecision::Deny => "deny",
                },
                acl.priority
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn field<E: IntoElement>(label: &'static str, input: E) -> gpui::Div {
    v_flex()
        .flex_1()
        .min_w(px(180.0))
        .gap(px(5.0))
        .child(
            div()
                .text_xs()
                .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                .child(label),
        )
        .child(div().w_full().min_w_0().child(input))
}

fn row() -> gpui::Div {
    h_flex()
        .w_full()
        .min_w_0()
        .flex_wrap()
        .items_end()
        .gap(px(10.0))
}

fn section_heading(
    title: &'static str,
    subtitle: &'static str,
    theme: &gpui_component::Theme,
) -> gpui::Div {
    v_flex()
        .w_full()
        .gap(px(2.0))
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(subtitle),
        )
}

fn toggle_button<F>(
    id: &'static str,
    label: &'static str,
    selected: bool,
    disabled: bool,
    cx: &mut Context<MqttView>,
    handler: F,
) -> gpui_component::button::Button
where
    F: Fn(&mut MqttView) + 'static,
{
    let mut button = ramag_ui::clickable_button(id).xsmall().label(if selected {
        format!("{}：开", label)
    } else {
        format!("{}：关", label)
    });
    button = if selected {
        button.primary()
    } else {
        button.ghost()
    };
    button
        .disabled(disabled)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            handler(this);
            cx.notify();
        }))
}

fn metric(label: &'static str, value: usize, theme: &gpui_component::Theme) -> gpui::Div {
    v_flex()
        .gap(px(3.0))
        .p(px(10.0))
        .min_w(px(110.0))
        .border_1()
        .border_color(theme.border)
        .rounded(px(5.0))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(value.to_string()),
        )
}

fn capability_items(capabilities: MqttTransportCapabilities) -> [(&'static str, bool); 8] {
    [
        ("Native 构建可用", capabilities.build_available),
        ("MQTT 3.1.1", capabilities.mqtt311),
        ("MQTT 5.0", capabilities.mqtt5),
        ("TCP", capabilities.tcp),
        ("TLS", capabilities.tls),
        ("发布", capabilities.publish),
        ("订阅", capabilities.subscribe),
        ("完整在线客户端目录", capabilities.online_clients),
    ]
}

#[cfg(test)]
mod visual_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::MqttTransportBackend;

    #[test]
    fn tool_metadata_exposes_mqtt_entry() {
        let tool = MqttTool::new();
        assert_eq!(tool.meta().id, MqttTool::ID);
        assert_eq!(tool.meta().name, "MQTT");
        assert_eq!(tool.meta().icon.as_deref(), Some("mqtt"));
    }

    #[test]
    fn client_permission_rows_expand_direct_and_group_roles() {
        let client = MosquittoClient {
            username: "operator".into(),
            client_id: None,
            password_configured: true,
            password: None,
            disabled: false,
            text_name: None,
            text_description: None,
            groups: vec![MosquittoGroupBinding {
                group_name: "operators".into(),
                priority: 20,
            }],
            roles: vec![MosquittoRoleBinding {
                role_name: "direct-reader".into(),
                priority: 10,
            }],
        };
        let snapshot = MosquittoDynamicSecuritySnapshot {
            clients: vec![client.clone()],
            groups: vec![MosquittoGroup {
                group_name: "operators".into(),
                text_name: None,
                text_description: None,
                roles: vec![MosquittoRoleBinding {
                    role_name: "group-writer".into(),
                    priority: 30,
                }],
            }],
            roles: vec![
                MosquittoRole {
                    role_name: "direct-reader".into(),
                    text_name: None,
                    text_description: None,
                    allow_wildcards_subscriptions: false,
                    acls: vec![MosquittoAcl {
                        acl_type: MosquittoAclType::SubscribeLiteral,
                        topic: "devices/operator/state".into(),
                        decision: MosquittoAclDecision::Allow,
                        priority: 1,
                    }],
                },
                MosquittoRole {
                    role_name: "group-writer".into(),
                    text_name: None,
                    text_description: None,
                    allow_wildcards_subscriptions: false,
                    acls: vec![MosquittoAcl {
                        acl_type: MosquittoAclType::PublishClientSend,
                        topic: "devices/operator/command".into(),
                        decision: MosquittoAclDecision::Deny,
                        priority: 2,
                    }],
                },
            ],
        };

        let (rows, missing_roles) = client_permission_rows(&snapshot, &client);
        assert_eq!(rows.len(), 2);
        assert!(missing_roles.is_empty());
        assert_eq!(rows[0].source, "直接 Role");
        assert_eq!(rows[0].role_name, "direct-reader");
        assert_eq!(rows[1].source, "Group operators");
        assert_eq!(rows[1].role_name, "group-writer");
    }

    #[test]
    fn capability_items_do_not_claim_management_or_topic_discovery() {
        let capabilities = MqttTransportCapabilities {
            backend: MqttTransportBackend::Native,
            build_available: true,
            mqtt311: true,
            mqtt5: true,
            tcp: true,
            tls: true,
            subscribe: true,
            publish: true,
            dynamic_security: false,
            static_config: false,
            metrics: false,
            retained_topics: false,
            online_clients: false,
        };
        let labels = capability_items(capabilities);
        assert!(
            !labels
                .iter()
                .any(|(label, enabled)| *enabled && label.contains("完整在线"))
        );
    }
}
