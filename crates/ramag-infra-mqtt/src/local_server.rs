use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;
use std::time::Duration;

use async_channel::{Receiver, Sender, TryRecvError, TrySendError};
use async_trait::async_trait;
use bytes::Bytes;
use oximqtt::Result as OxiResult;
use oximqtt::codec::types::{Publish as CodecPublish, QoS};
use oximqtt::codec::v5::PublishProperties;
use oximqtt::context::ServerContext;
use oximqtt::hook::{Handler, HookResult, Parameter, ReturnType, Type};
use oximqtt::net::Builder;
use oximqtt::retain::{DefaultRetainStorage, RetainStorage};
use oximqtt::server::MqttServer;
use oximqtt::session::Session;
use oximqtt::types::{AuthResult, From, Id, Publish, Retain, TopicFilter, TopicName};
use ramag_domain::entities::{
    MqttBrokerSnapshot, MqttLocalServerConfig, MqttLocalServerEvent, MqttLocalServerEventSink,
    MqttLocalServerEventSinkResult, MqttLocalServerStatus, MqttLocalServerUser, MqttMessage,
    MqttOnlineClient, MqttPublishRequest, MqttPublishResult, MqttQos, MqttSubscription,
    MqttUserProperty,
};
use ramag_domain::error::{DomainError, MqttError, MqttErrorCategory, Result};
use ramag_domain::traits::MqttLocalServerDriver;
use tokio::sync::{Mutex, mpsc, oneshot};

const LOCAL_SERVER_MAX_CONNECTIONS: usize = 1024;
const LOCAL_SERVER_MAX_PACKET_BYTES: u32 = 16 * 1024 * 1024;
const LOCAL_SERVER_TASK_WORKERS: usize = 4;
const LOCAL_SERVER_TASK_QUEUE: usize = 2048;
const LOCAL_SERVER_COMMAND_QUEUE: usize = 32;

#[derive(Clone)]
pub struct NativeMqttLocalServer {
    state: Arc<Mutex<LocalServerState>>,
}

struct LocalServerState {
    status: MqttLocalServerStatus,
    config: Option<MqttLocalServerConfig>,
    stop: Option<oneshot::Sender<()>>,
    commands: Option<mpsc::Sender<LocalServerCommand>>,
    events: Option<Receiver<MqttLocalServerEvent>>,
    task: Option<std::thread::JoinHandle<()>>,
}

impl NativeMqttLocalServer {
    pub fn new() -> Self {
        let config = MqttLocalServerConfig::default();
        Self {
            state: Arc::new(Mutex::new(LocalServerState {
                status: MqttLocalServerStatus::stopped(&config),
                config: None,
                stop: None,
                commands: None,
                events: None,
                task: None,
            })),
        }
    }
}

impl Default for NativeMqttLocalServer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MqttLocalServerDriver for NativeMqttLocalServer {
    /// 启动本地 Broker；相同运行配置可幂等调用，冲突配置必须先停止当前实例。
    async fn start(&self, config: &MqttLocalServerConfig) -> Result<MqttLocalServerStatus> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        let ip = config
            .bind_host
            .parse::<IpAddr>()
            .map_err(|_| DomainError::InvalidConfig("本地 MQTT Broker 监听地址无效".into()))?;
        let address = SocketAddr::new(ip, config.port);

        let mut state = self.state.lock().await;
        if let Some(task) = state.task.take() {
            if !task.is_finished() {
                state.task = Some(task);
                if state
                    .config
                    .as_ref()
                    .is_some_and(|current| current != config)
                {
                    return Err(DomainError::InvalidConfig(
                        "本地 MQTT Broker 已运行，请先停止后再修改配置".into(),
                    ));
                }
                return Ok(state.status.clone());
            }
            let _ = task.join();
            state.config = None;
            state.stop = None;
            state.commands = None;
            state.events = None;
        }

        let (ready_sender, ready_receiver) = sync_channel(1);
        let (stop_sender, stop_receiver) = oneshot::channel();
        let (publish_sender, publish_receiver) = mpsc::channel(LOCAL_SERVER_COMMAND_QUEUE);
        let (event_sender, event_receiver) = async_channel::bounded(256);
        let allow_anonymous = config.allow_anonymous;
        let users = config.users.clone();
        let task = std::thread::Builder::new()
            .name("ramag-mqtt-broker".into())
            .spawn(move || {
                run_local_server(
                    address,
                    allow_anonymous,
                    users,
                    stop_receiver,
                    publish_receiver,
                    event_sender,
                    ready_sender,
                )
            })
            .map_err(|error| local_server_error("启动本地 MQTT Broker 线程", error))?;

        let ready = smol::unblock(move || ready_receiver.recv())
            .await
            .map_err(|error| local_server_error("启动本地 MQTT Broker", error))?;
        if let Err(error) = ready {
            let _ = task.join();
            return Err(local_server_error("启动本地 MQTT Broker", error));
        }
        let status = MqttLocalServerStatus::running(config);
        state.status = status.clone();
        state.config = Some(config.clone());
        state.stop = Some(stop_sender);
        state.commands = Some(publish_sender);
        state.events = Some(event_receiver);
        state.task = Some(task);
        Ok(status)
    }

    async fn stop(&self) -> Result<MqttLocalServerStatus> {
        let mut state = self.state.lock().await;
        if let Some(stop) = state.stop.take() {
            let _ = stop.send(());
        }
        state.commands = None;
        state.events = None;
        if let Some(task) = state.task.take() {
            smol::unblock(move || task.join()).await.map_err(|_| {
                local_server_error("等待本地 MQTT Broker 停止", "Broker 线程异常退出")
            })?;
        }
        state.config = None;
        state.status.running = false;
        Ok(state.status.clone())
    }

    async fn status(&self) -> Result<MqttLocalServerStatus> {
        // 状态查询同时回收已结束的后台线程，避免后续启动继续持有旧句柄。
        let mut state = self.state.lock().await;
        if state
            .task
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
        {
            state.task = None;
            state.config = None;
            state.stop = None;
            state.commands = None;
            state.events = None;
            state.status.running = false;
        }
        Ok(state.status.clone())
    }

    async fn publish(&self, request: &MqttPublishRequest) -> Result<MqttPublishResult> {
        request.validate().map_err(DomainError::InvalidConfig)?;
        let (response_sender, response_receiver) = oneshot::channel();
        let sender = {
            let state = self.state.lock().await;
            state
                .commands
                .as_ref()
                .filter(|_| state.status.running)
                .cloned()
                .ok_or_else(|| local_server_error("发布到本地 MQTT Broker", "Broker 当前未运行"))?
        };
        sender
            .send(LocalServerCommand::Publish {
                request: request.clone(),
                response: response_sender,
            })
            .await
            .map_err(|_| local_server_error("发布到本地 MQTT Broker", "Broker 运行线程已退出"))?;
        response_receiver
            .await
            .map_err(|_| local_server_error("发布到本地 MQTT Broker", "Broker 未返回发布结果"))?
    }

    async fn snapshot(&self) -> Result<MqttBrokerSnapshot> {
        let (response_sender, response_receiver) = oneshot::channel();
        let sender = {
            let state = self.state.lock().await;
            state
                .commands
                .as_ref()
                .filter(|_| state.status.running)
                .cloned()
                .ok_or_else(|| {
                    local_server_error("读取本地 MQTT Broker 快照", "Broker 当前未运行")
                })?
        };
        sender
            .send(LocalServerCommand::Snapshot {
                response: response_sender,
            })
            .await
            .map_err(|_| {
                local_server_error("读取本地 MQTT Broker 快照", "Broker 运行线程已退出")
            })?;
        response_receiver
            .await
            .map_err(|_| local_server_error("读取本地 MQTT Broker 快照", "Broker 未返回快照结果"))?
    }

    async fn subscribe_events(
        &self,
        sink: MqttLocalServerEventSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        let receiver = {
            let state = self.state.lock().await;
            state.events.as_ref().cloned().ok_or_else(|| {
                DomainError::NotImplemented("mqtt_local_server_events_not_running".into())
            })?
        };
        while !cancelled.load(Ordering::Acquire) {
            match receiver.try_recv() {
                Ok(event) => match sink(event) {
                    MqttLocalServerEventSinkResult::Accepted => {}
                    MqttLocalServerEventSinkResult::Backpressured => {
                        tracing::debug!(
                            operation = "mqtt_local_server_event_sink",
                            "本地 MQTT Broker 事件接收方繁忙，丢弃一条事件"
                        );
                    }
                    MqttLocalServerEventSinkResult::Closed => break,
                },
                Err(TryRecvError::Empty) => {
                    smol::Timer::after(Duration::from_millis(20)).await;
                }
                Err(TryRecvError::Closed) => break,
            }
        }
        Ok(())
    }
}

enum LocalServerCommand {
    Publish {
        request: MqttPublishRequest,
        response: oneshot::Sender<Result<MqttPublishResult>>,
    },
    Snapshot {
        response: oneshot::Sender<Result<MqttBrokerSnapshot>>,
    },
}

fn local_server_error(operation: &'static str, error: impl std::fmt::Display) -> DomainError {
    DomainError::Mqtt(MqttError::new(
        MqttErrorCategory::Network,
        operation,
        format!("{operation}失败：{error}"),
    ))
}

fn run_local_server(
    address: SocketAddr,
    allow_anonymous: bool,
    users: Vec<MqttLocalServerUser>,
    stop_receiver: oneshot::Receiver<()>,
    mut publish_receiver: mpsc::Receiver<LocalServerCommand>,
    event_sender: Sender<MqttLocalServerEvent>,
    ready_sender: std::sync::mpsc::SyncSender<std::result::Result<(), String>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready_sender.send(Err(format!("无法初始化 Broker 运行时：{error}")));
            return;
        }
    };
    runtime.block_on(async move {
        let listener = match Builder::new()
            .name("ramag-local-mqtt")
            .laddr(address)
            .max_connections(LOCAL_SERVER_MAX_CONNECTIONS)
            .max_packet_size(LOCAL_SERVER_MAX_PACKET_BYTES)
            .allow_anonymous(allow_anonymous)
            .bind()
        {
            Ok(listener) => listener,
            Err(error) => {
                let _ = ready_sender.send(Err(format!("绑定本地 MQTT Broker 失败：{error}")));
                return;
            }
        };
        let context = ServerContext::new()
            .busy_check_enable(false)
            .task_exec_workers(LOCAL_SERVER_TASK_WORKERS)
            .task_exec_queue_max(LOCAL_SERVER_TASK_QUEUE)
            .mqtt_max_sessions(LOCAL_SERVER_MAX_CONNECTIONS as isize)
            .build()
            .await;
        *context.extends.retain_mut().await = Box::new(LocalRetainStorage::new());

        let register = context.extends.hook_mgr().register();
        if !allow_anonymous || !users.is_empty() {
            register
                .add(
                    Type::ClientAuthenticate,
                    Box::new(LocalAuthHandler::new(allow_anonymous, users)),
                )
                .await;
        }
        register
            .add(
                Type::ClientConnected,
                Box::new(LocalEventHandler::new(event_sender.clone())),
            )
            .await;
        register
            .add(
                Type::ClientDisconnected,
                Box::new(LocalEventHandler::new(event_sender.clone())),
            )
            .await;
        register
            .add(
                Type::SessionSubscribed,
                Box::new(LocalEventHandler::new(event_sender.clone())),
            )
            .await;
        register
            .add(
                Type::SessionUnsubscribed,
                Box::new(LocalEventHandler::new(event_sender.clone())),
            )
            .await;
        register
            .add(
                Type::MessagePublish,
                Box::new(LocalEventHandler::new(event_sender.clone())),
            )
            .await;
        register.start().await;

        let server = MqttServer::new(context.clone()).listener(listener).build();
        if ready_sender.send(Ok(())).is_err() {
            return;
        }
        tokio::select! {
            result = server.run() => {
                if let Err(error) = result {
                    tracing::error!(operation = "mqtt_local_server_run", error = %error, "local MQTT Broker stopped unexpectedly");
                }
            }
            _ = process_local_commands(context, &mut publish_receiver, event_sender.clone()) => {}
            _ = stop_receiver => {}
        }
    });
}

async fn process_local_commands(
    context: ServerContext,
    receiver: &mut mpsc::Receiver<LocalServerCommand>,
    event_sender: Sender<MqttLocalServerEvent>,
) {
    while let Some(command) = receiver.recv().await {
        match command {
            LocalServerCommand::Publish { request, response } => {
                let result = publish_to_local_server(&context, &request, &event_sender).await;
                let _ = response.send(result);
            }
            LocalServerCommand::Snapshot { response } => {
                let result = snapshot_local_server(&context).await;
                let _ = response.send(result);
            }
        }
    }
}

async fn snapshot_local_server(context: &ServerContext) -> Result<MqttBrokerSnapshot> {
    let entries = context.extends.shared().await.iter().collect::<Vec<_>>();
    let mut online_clients = Vec::with_capacity(entries.len());
    for entry in entries {
        if !entry.online().await {
            continue;
        }
        let Some(session) = entry.session() else {
            continue;
        };
        let connect_info = session.connect_info().await.ok();
        let subscriptions = entry.subscriptions().await.unwrap_or_default();
        let subscriptions = subscriptions
            .into_iter()
            .map(|subscription| MqttSubscription {
                filter: subscription.topic.to_string(),
                qos: match subscription.opts.qos_value() {
                    0 => MqttQos::AtMostOnce,
                    1 => MqttQos::AtLeastOnce,
                    _ => MqttQos::ExactlyOnce,
                },
                no_local: subscription.opts.no_local().unwrap_or(false),
            })
            .collect();
        let connected_at = session
            .connected_at()
            .await
            .ok()
            .and_then(chrono::DateTime::from_timestamp_millis);
        online_clients.push(MqttOnlineClient {
            client_id: session.id.client_id.to_string(),
            username: connect_info
                .as_ref()
                .and_then(|info| info.username())
                .map(ToString::to_string),
            remote_address: connect_info
                .as_ref()
                .and_then(|info| info.ipaddress())
                .map(|address| address.to_string()),
            connected_at,
            subscriptions,
        });
    }
    Ok(MqttBrokerSnapshot {
        topics: Vec::new(),
        online_clients,
        topics_complete: false,
        online_clients_complete: true,
    })
}

async fn publish_to_local_server(
    context: &ServerContext,
    request: &MqttPublishRequest,
    event_sender: &Sender<MqttLocalServerEvent>,
) -> Result<MqttPublishResult> {
    let qos = match request.qos {
        MqttQos::AtMostOnce => QoS::AtMostOnce,
        MqttQos::AtLeastOnce => QoS::AtLeastOnce,
        MqttQos::ExactlyOnce => QoS::ExactlyOnce,
    };
    let properties = PublishProperties {
        user_properties: request
            .user_properties
            .iter()
            .map(|property| (property.name.clone().into(), property.value.clone().into()))
            .collect(),
        ..Default::default()
    };
    let publish = Publish::new(
        Box::new(CodecPublish {
            dup: false,
            retain: request.retain,
            qos,
            topic: request.topic.clone().into(),
            packet_id: None,
            payload: Bytes::from(request.payload.clone()),
            properties: Some(properties),
        }),
        None,
        None,
        None,
    );
    let from = From::from_system(Id::from(0, "ramag-local-server".into()));

    if request.retain {
        let retain = context.extends.retain().await;
        if retain.enable() {
            retain
                .set(
                    &publish.topic,
                    Retain {
                        msg_id: None,
                        from: from.clone(),
                        publish: publish.clone(),
                    },
                    None,
                )
                .await
                .map_err(|error| local_server_error("保存本地 MQTT 保留消息", error))?;
        }
    }

    match context
        .extends
        .shared()
        .await
        .forwards(None, from.clone(), publish)
        .await
    {
        Ok(0) => context.extends.hook_mgr().message_nonsubscribed(from).await,
        Ok(_) => {}
        Err((_subscriber_count, errors)) => {
            for (to, from, publish, reason) in errors {
                context
                    .extends
                    .hook_mgr()
                    .message_dropped(Some(to), from, publish, reason)
                    .await;
            }
        }
    }

    emit_local_event(
        event_sender,
        MqttLocalServerEvent::BrokerPublished {
            message: MqttMessage {
                topic: request.topic.clone(),
                payload: request.payload.clone(),
                qos: request.qos,
                retain: request.retain,
                duplicate: false,
                received_at: chrono::Utc::now(),
                user_properties: request.user_properties.clone(),
            },
            occurred_at: chrono::Utc::now(),
        },
    );

    Ok(MqttPublishResult {
        topic: request.topic.clone(),
        packet_id: None,
        qos: request.qos,
    })
}

#[derive(Clone)]
struct LocalEventHandler {
    sender: Sender<MqttLocalServerEvent>,
}

impl LocalEventHandler {
    fn new(sender: Sender<MqttLocalServerEvent>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl Handler for LocalEventHandler {
    async fn hook(&self, parameter: &Parameter, _acc: Option<HookResult>) -> ReturnType {
        match parameter {
            Parameter::ClientConnected(session) => {
                if let Some(client) = online_client_from_session(session).await {
                    emit_local_event(
                        &self.sender,
                        MqttLocalServerEvent::ClientConnected {
                            client,
                            occurred_at: chrono::Utc::now(),
                        },
                    );
                }
            }
            Parameter::ClientDisconnected(session, reason) => {
                emit_local_event(
                    &self.sender,
                    MqttLocalServerEvent::ClientDisconnected {
                        client_id: session.id.client_id.to_string(),
                        reason: Some(format!("{reason:?}")),
                        occurred_at: chrono::Utc::now(),
                    },
                );
            }
            Parameter::SessionSubscribed(session, subscribe) => {
                if let Some(subscription) = subscription_from_oximqtt(subscribe) {
                    emit_local_event(
                        &self.sender,
                        MqttLocalServerEvent::ClientSubscribed {
                            client_id: session.id.client_id.to_string(),
                            subscription,
                            occurred_at: chrono::Utc::now(),
                        },
                    );
                }
            }
            Parameter::SessionUnsubscribed(session, unsubscribe) => {
                emit_local_event(
                    &self.sender,
                    MqttLocalServerEvent::ClientUnsubscribed {
                        client_id: session.id.client_id.to_string(),
                        filter: unsubscribe.topic_filter.to_string(),
                        occurred_at: chrono::Utc::now(),
                    },
                );
            }
            Parameter::MessagePublish(Some(session), _, publish) => {
                if let Some(message) = message_from_oximqtt(publish) {
                    emit_local_event(
                        &self.sender,
                        MqttLocalServerEvent::ClientPublished {
                            client_id: session.id.client_id.to_string(),
                            message,
                            occurred_at: chrono::Utc::now(),
                        },
                    );
                }
            }
            _ => {}
        }
        (true, None)
    }
}

async fn online_client_from_session(session: &Session) -> Option<MqttOnlineClient> {
    let connect_info = session.connect_info().await.ok();
    let connected_at = session
        .connected_at()
        .await
        .ok()
        .and_then(chrono::DateTime::from_timestamp_millis);
    let client = MqttOnlineClient {
        client_id: session.id.client_id.to_string(),
        username: connect_info
            .as_ref()
            .and_then(|info| info.username())
            .map(ToString::to_string),
        remote_address: connect_info
            .as_ref()
            .and_then(|info| info.ipaddress())
            .map(|address| address.to_string()),
        connected_at,
        subscriptions: Vec::new(),
    };
    client.validate().ok().map(|_| client)
}

fn subscription_from_oximqtt(subscribe: &oximqtt::types::Subscribe) -> Option<MqttSubscription> {
    let subscription = MqttSubscription {
        filter: subscribe.topic_filter.to_string(),
        qos: qos_from_oximqtt(subscribe.opts.qos_value()),
        no_local: subscribe.opts.no_local().unwrap_or(false),
    };
    subscription.validate().ok().map(|_| subscription)
}

fn message_from_oximqtt(publish: &Publish) -> Option<MqttMessage> {
    let inner = publish.inner.as_ref();
    let user_properties = inner
        .properties
        .as_ref()
        .map(|properties| {
            properties
                .user_properties
                .iter()
                .map(|(name, value)| MqttUserProperty {
                    name: name.to_string(),
                    value: value.to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    let message = MqttMessage {
        topic: inner.topic.to_string(),
        payload: inner.payload.to_vec(),
        qos: qos_from_oximqtt(inner.qos.value()),
        retain: inner.retain,
        duplicate: inner.dup,
        received_at: chrono::Utc::now(),
        user_properties,
    };
    message.validate().ok().map(|_| message)
}

fn qos_from_oximqtt(value: u8) -> MqttQos {
    match value {
        0 => MqttQos::AtMostOnce,
        1 => MqttQos::AtLeastOnce,
        _ => MqttQos::ExactlyOnce,
    }
}

fn emit_local_event(sender: &Sender<MqttLocalServerEvent>, event: MqttLocalServerEvent) {
    if let Err(error) = event.validate() {
        tracing::warn!(
            operation = "mqtt_local_server_event",
            error = %error,
            "本地 MQTT Broker 事件未通过数据校验"
        );
        return;
    }
    match sender.try_send(event) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            tracing::warn!(
                operation = "mqtt_local_server_event",
                "本地 MQTT Broker 事件队列已满，丢弃一条事件"
            );
        }
        Err(TrySendError::Closed(_)) => {}
    }
}

struct LocalAuthHandler {
    allow_anonymous: bool,
    users: Vec<MqttLocalServerUser>,
}

impl LocalAuthHandler {
    fn new(allow_anonymous: bool, users: Vec<MqttLocalServerUser>) -> Self {
        Self {
            allow_anonymous,
            users,
        }
    }
}

#[async_trait]
impl Handler for LocalAuthHandler {
    async fn hook(&self, parameter: &Parameter, _acc: Option<HookResult>) -> ReturnType {
        let Parameter::ClientAuthenticate(connect_info) = parameter else {
            return (true, None);
        };
        let username = connect_info.username().map(|value| value.as_ref());
        let password = connect_info.password().map(|value| value.as_ref());
        let anonymous = username.is_none() && password.is_none();
        let authenticated = (self.allow_anonymous && anonymous)
            || username.is_some_and(|username| {
                self.users.iter().any(|user| {
                    user.username == username && password == Some(user.password.as_bytes())
                })
            });
        let result = if authenticated {
            AuthResult::Allow(false, None)
        } else {
            AuthResult::BadUsernameOrPassword
        };
        (false, Some(HookResult::AuthResult(result)))
    }
}

#[derive(Clone)]
struct LocalRetainStorage {
    inner: Arc<DefaultRetainStorage>,
}

impl LocalRetainStorage {
    fn new() -> Self {
        Self {
            inner: Arc::new(DefaultRetainStorage::new()),
        }
    }
}

#[async_trait]
impl RetainStorage for LocalRetainStorage {
    fn enable(&self) -> bool {
        true
    }

    async fn set(
        &self,
        topic: &TopicName,
        retain: Retain,
        expiry_interval: Option<std::time::Duration>,
    ) -> OxiResult<()> {
        self.inner
            .set_with_timeout(topic, retain, expiry_interval)
            .await
    }

    async fn get(&self, topic_filter: &TopicFilter) -> OxiResult<Vec<(TopicName, Retain)>> {
        self.inner.get_message(topic_filter).await
    }

    async fn count(&self) -> isize {
        self.inner.count().await
    }

    async fn max(&self) -> isize {
        self.inner.max().await
    }
}
