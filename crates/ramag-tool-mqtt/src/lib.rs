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
        MosquittoDynamicSecuritySnapshot, MqttBrokerSnapshot, MqttMessage, MqttMessageSinkResult,
        MqttProfile, MqttProfileId, MqttProtocolVersion, MqttPublishRequest, MqttQos,
        MqttSubscribeRequest, MqttSubscription, MqttTlsConfig, MqttTransport as TransportKind,
        MqttTransportCapabilities,
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
            .with_icon("radio"),
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

/// MQTT 页面状态；列表和消息只在对应服务请求成功后写入。
pub struct MqttView {
    service: Arc<MqttService>,
    profiles: Vec<MqttProfile>,
    selected_profile_id: Option<MqttProfileId>,
    section: MqttSection,
    name: Entity<InputState>,
    host: Entity<InputState>,
    port: Entity<InputState>,
    client_id: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    ca_cert_path: Entity<InputState>,
    client_cert_path: Entity<InputState>,
    client_key_path: Entity<InputState>,
    keep_alive: Entity<InputState>,
    publish_topic: Entity<InputState>,
    publish_payload: Entity<InputState>,
    subscribe_filter: Entity<InputState>,
    search: Entity<InputState>,
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
    snapshot: Option<MqttBrokerSnapshot>,
    management_snapshot: Option<MosquittoDynamicSecuritySnapshot>,
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

        let fields = [
            &name,
            &host,
            &port,
            &client_id,
            &username,
            &password,
            &ca_cert_path,
            &client_cert_path,
            &client_key_path,
            &keep_alive,
            &publish_topic,
            &publish_payload,
            &subscribe_filter,
            &search,
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
            name,
            host,
            port,
            client_id,
            username,
            password,
            ca_cert_path,
            client_cert_path,
            client_key_path,
            keep_alive,
            publish_topic,
            publish_payload,
            subscribe_filter,
            search,
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
            snapshot: None,
            management_snapshot: None,
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
        self.password.update(cx, |state, cx| {
            state.set_placeholder("密码（可选）", window, cx);
        });
        self.clear_runtime_state();
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
            let _ = this.update_in(cx, |this, _, cx| {
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

    fn load_management(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.selected_profile().cloned() else {
            self.notice = Some(("请先保存并选择已启用 Mosquitto 管理的配置".into(), true));
            cx.notify();
            return;
        };
        self.loading_management = true;
        self.management_error = None;
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.dynamic_security_snapshot(&profile).await;
            let _ = this.update_in(cx, |this, _, cx| {
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

    fn select_section(&mut self, section: MqttSection, cx: &mut Context<Self>) {
        if self.section != section {
            self.section = section;
            cx.notify();
        }
    }

    fn render_sidebar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
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
            .w(px(250.0))
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
                        ramag_ui::clickable_button("mqtt-add-profile")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .tooltip("新建配置")
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.new_profile(window, cx)
                            })),
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
            .when(window.viewport_size().width < px(760.0), |sidebar| {
                sidebar.w(px(210.0))
            })
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
            );
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
        let mut protocol_buttons = h_flex().gap(px(4.0));
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
                    .child(field("协议版本", protocol_buttons))
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

    fn render_mosquitto(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut body = v_flex().w_full().max_w(px(920.0)).gap(px(12.0)).child(section_heading("Mosquitto 管理", "Dynamic Security 需要 Broker 开启对应插件；静态 password_file/acl_file 不能通过通用 MQTT 数据面猜测", &theme));
        if !self.management_enabled {
            body = body.child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("当前配置未启用 Mosquitto 管理能力。"),
            );
        } else {
            body = body.child(
                ramag_ui::clickable_button("mqtt-load-management")
                    .ghost()
                    .small()
                    .label("读取 Dynamic Security")
                    .loading(self.loading_management)
                    .disabled(self.loading_management || self.selected_profile_id.is_none())
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.load_management(window, cx)
                    })),
            );
            if let Some(error) = &self.management_error {
                body = body.child(
                    div()
                        .text_sm()
                        .text_color(theme.danger)
                        .child(error.clone()),
                );
            } else if let Some(snapshot) = &self.management_snapshot {
                body = body.child(h_flex().flex_wrap().gap(px(8.0)).children([
                    metric("客户端", snapshot.clients.len(), &theme),
                    metric("Group", snapshot.groups.len(), &theme),
                    metric("Role", snapshot.roles.len(), &theme),
                ]));
                let mut clients = v_flex().gap(px(4.0));
                for client in &snapshot.clients {
                    clients = clients.child(div().text_xs().child(format!(
                        "{} · {} 个 Group · {} 个 Role",
                        client.username,
                        client.groups.len(),
                        client.roles.len()
                    )));
                }
                body = body
                    .child(section_heading(
                        "客户端和权限绑定",
                        "以下内容来自 Dynamic Security 返回值",
                        &theme,
                    ))
                    .child(clients);
            } else {
                body = body.child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("尚未读取 Mosquitto 管理数据。"),
                );
            }
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
        let main = v_flex()
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
            .child(self.render_sidebar(window, cx))
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

fn field<E: IntoElement>(label: &'static str, input: E) -> gpui::Div {
    v_flex()
        .flex_1()
        .min_w_0()
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
    h_flex().w_full().min_w_0().items_end().gap(px(10.0))
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
mod tests {
    use super::*;
    use ramag_domain::entities::MqttTransportBackend;

    #[test]
    fn tool_metadata_exposes_mqtt_entry() {
        let tool = MqttTool::new();
        assert_eq!(tool.meta().id, MqttTool::ID);
        assert_eq!(tool.meta().name, "MQTT");
        assert!(tool.meta().icon.is_some());
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
