use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::mpsc::sync_channel;

use async_trait::async_trait;
use oximqtt::Result as OxiResult;
use oximqtt::context::ServerContext;
use oximqtt::hook::{Handler, HookResult, Parameter, ReturnType, Type};
use oximqtt::net::Builder;
use oximqtt::retain::{DefaultRetainStorage, RetainStorage};
use oximqtt::server::MqttServer;
use oximqtt::types::{AuthResult, Retain, TopicFilter, TopicName};
use ramag_domain::entities::{MqttLocalServerConfig, MqttLocalServerStatus, MqttLocalServerUser};
use ramag_domain::error::{DomainError, MqttError, MqttErrorCategory, Result};
use ramag_domain::traits::MqttLocalServerDriver;
use tokio::sync::{Mutex, oneshot};

const LOCAL_SERVER_MAX_CONNECTIONS: usize = 1024;
const LOCAL_SERVER_MAX_PACKET_BYTES: u32 = 16 * 1024 * 1024;
const LOCAL_SERVER_TASK_WORKERS: usize = 4;
const LOCAL_SERVER_TASK_QUEUE: usize = 2048;

#[derive(Clone)]
pub struct NativeMqttLocalServer {
    state: Arc<Mutex<LocalServerState>>,
}

struct LocalServerState {
    status: MqttLocalServerStatus,
    stop: Option<oneshot::Sender<()>>,
    task: Option<std::thread::JoinHandle<()>>,
}

impl NativeMqttLocalServer {
    pub fn new() -> Self {
        let config = MqttLocalServerConfig::default();
        Self {
            state: Arc::new(Mutex::new(LocalServerState {
                status: MqttLocalServerStatus::stopped(&config),
                stop: None,
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
                return Ok(state.status.clone());
            }
            let _ = task.join();
            state.stop = None;
        }

        let (ready_sender, ready_receiver) = sync_channel(1);
        let (stop_sender, stop_receiver) = oneshot::channel();
        let allow_anonymous = config.allow_anonymous;
        let users = config.users.clone();
        let task = std::thread::Builder::new()
            .name("ramag-mqtt-broker".into())
            .spawn(move || {
                run_local_server(address, allow_anonymous, users, stop_receiver, ready_sender)
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
        state.stop = Some(stop_sender);
        state.task = Some(task);
        Ok(status)
    }

    async fn stop(&self) -> Result<MqttLocalServerStatus> {
        let mut state = self.state.lock().await;
        if let Some(stop) = state.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = state.task.take() {
            smol::unblock(move || task.join()).await.map_err(|_| {
                local_server_error("等待本地 MQTT Broker 停止", "Broker 线程异常退出")
            })?;
        }
        state.status.running = false;
        Ok(state.status.clone())
    }

    async fn status(&self) -> Result<MqttLocalServerStatus> {
        let mut state = self.state.lock().await;
        if state
            .task
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
        {
            state.task = None;
            state.status.running = false;
        }
        Ok(state.status.clone())
    }
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

        if !allow_anonymous || !users.is_empty() {
            let register = context.extends.hook_mgr().register();
            register
                .add(
                    Type::ClientAuthenticate,
                    Box::new(LocalAuthHandler::new(allow_anonymous, users)),
                )
                .await;
            register.start().await;
        }

        let server = MqttServer::new(context).listener(listener).build();
        if ready_sender.send(Ok(())).is_err() {
            return;
        }
        tokio::select! {
            result = server.run() => {
                if let Err(error) = result {
                    tracing::error!(operation = "mqtt_local_server_run", error = %error, "local MQTT Broker stopped unexpectedly");
                }
            }
            _ = stop_receiver => {}
        }
    });
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
