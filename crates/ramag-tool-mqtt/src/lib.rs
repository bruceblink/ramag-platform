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

include!("mqtt_view/profile_state.rs");
include!("mqtt_view/mqtt_operations.rs");
include!("mqtt_view/dynamic_security_operations.rs");
include!("mqtt_view/management_editor_actions.rs");
include!("mqtt_view/configuration_view.rs");
include!("mqtt_view/message_operations_view.rs");
include!("mqtt_view/client_permissions_view.rs");
include!("mqtt_view/group_role_management_view.rs");
include!("mqtt_view/broker_configuration_view.rs");
include!("mqtt_view/render.rs");
include!("mqtt_view/helpers.rs");

#[cfg(test)]
mod visual_tests;

#[cfg(test)]
mod tests {
    include!("mqtt_view/tests.rs");
}
