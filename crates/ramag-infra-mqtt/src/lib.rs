//! MQTT 基础设施层。
//!
//! `native` 特性启用 `rumqttc`，默认构建仍保留无 native 依赖的能力探测和明确的
//! 不支持结果。适配器在独立 Tokio 运行时中驱动 EventLoop，再把领域消息交给有界 sink。

use std::sync::{Arc, atomic::AtomicBool};
#[cfg(feature = "native")]
use std::time::Duration;

use async_trait::async_trait;
use ramag_domain::entities::{
    MosquittoClient, MosquittoDynamicSecuritySnapshot, MosquittoGroup, MosquittoRole,
    MosquittoStaticFile, MosquittoStaticFileKind, MqttBrokerSnapshot, MqttMessageSink, MqttProfile,
    MqttPublishRequest, MqttPublishResult, MqttSubscribeRequest, MqttTransportBackend,
    MqttTransportCapabilities,
};
use ramag_domain::error::{DomainError, MqttError, MqttErrorCategory, Result};
use ramag_domain::traits::{
    MosquittoDynamicSecurityDriver, MosquittoStaticConfigDriver, MqttDriver, MqttTransport,
};

#[cfg(feature = "native")]
use native::{publish_native, subscribe_native, test_connection_native};

#[cfg(feature = "native")]
const EVENT_LOOP_CAPACITY: usize = 32;

#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMqttTransport;

impl NativeMqttTransport {
    pub const fn new() -> Self {
        Self
    }
}

impl MqttTransport for NativeMqttTransport {
    fn capabilities(&self) -> MqttTransportCapabilities {
        self.transport_capabilities()
    }
}

impl NativeMqttTransport {
    pub const fn transport_capabilities(&self) -> MqttTransportCapabilities {
        MqttTransportCapabilities {
            backend: MqttTransportBackend::Native,
            build_available: cfg!(feature = "native"),
            mqtt311: cfg!(feature = "native"),
            mqtt5: cfg!(feature = "native"),
            tcp: cfg!(feature = "native"),
            tls: cfg!(feature = "native"),
            subscribe: cfg!(feature = "native"),
            publish: cfg!(feature = "native"),
            dynamic_security: cfg!(feature = "native"),
            static_config: false,
            metrics: false,
            retained_topics: false,
            online_clients: false,
        }
    }
}

/// Native MQTT 控制面适配器；只访问 Mosquitto Dynamic Security 明确规定的控制主题。
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMosquittoDynamicSecurityDriver;

impl NativeMosquittoDynamicSecurityDriver {
    pub const fn new() -> Self {
        Self
    }
}

/// 本机 Mosquitto 静态文件适配器；SSH 目标交给后续具备远程文件权限的适配器。
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalMosquittoStaticConfigDriver;

impl LocalMosquittoStaticConfigDriver {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl MosquittoDynamicSecurityDriver for NativeMosquittoDynamicSecurityDriver {
    async fn snapshot(&self, profile: &MqttProfile) -> Result<MosquittoDynamicSecuritySnapshot> {
        #[cfg(feature = "native")]
        {
            let profile = management_profile(profile);
            return run_native(move || native::dynamic_security_snapshot_native(profile)).await;
        }
        #[cfg(not(feature = "native"))]
        {
            let _ = profile;
            Err(native_unavailable("读取 Mosquitto Dynamic Security"))
        }
    }

    async fn save_client(&self, profile: &MqttProfile, client: &MosquittoClient) -> Result<()> {
        #[cfg(feature = "native")]
        {
            let profile = management_profile(profile);
            let client = client.clone();
            return run_native(move || native::save_client_native(profile, client)).await;
        }
        #[cfg(not(feature = "native"))]
        {
            let _ = (profile, client);
            Err(native_unavailable("保存 Mosquitto 客户端"))
        }
    }

    async fn delete_client(&self, profile: &MqttProfile, username: &str) -> Result<()> {
        dynamic_security_mutation(
            self,
            profile,
            "deleteClient",
            serde_json::json!({
                "username": username,
            }),
        )
        .await
    }

    async fn save_group(&self, profile: &MqttProfile, group: &MosquittoGroup) -> Result<()> {
        #[cfg(feature = "native")]
        {
            let profile = management_profile(profile);
            let group = group.clone();
            return run_native(move || native::save_group_native(profile, group)).await;
        }
        #[cfg(not(feature = "native"))]
        {
            let _ = (profile, group);
            Err(native_unavailable("保存 Mosquitto Group"))
        }
    }

    async fn delete_group(&self, profile: &MqttProfile, group_name: &str) -> Result<()> {
        dynamic_security_mutation(
            self,
            profile,
            "deleteGroup",
            serde_json::json!({
                "groupname": group_name,
            }),
        )
        .await
    }

    async fn save_role(&self, profile: &MqttProfile, role: &MosquittoRole) -> Result<()> {
        #[cfg(feature = "native")]
        {
            let profile = management_profile(profile);
            let role = role.clone();
            return run_native(move || native::save_role_native(profile, role)).await;
        }
        #[cfg(not(feature = "native"))]
        {
            let _ = (profile, role);
            Err(native_unavailable("保存 Mosquitto Role"))
        }
    }

    async fn delete_role(&self, profile: &MqttProfile, role_name: &str) -> Result<()> {
        dynamic_security_mutation(
            self,
            profile,
            "deleteRole",
            serde_json::json!({
                "rolename": role_name,
            }),
        )
        .await
    }
}

async fn dynamic_security_mutation(
    _driver: &NativeMosquittoDynamicSecurityDriver,
    profile: &MqttProfile,
    command: &'static str,
    fields: serde_json::Value,
) -> Result<()> {
    #[cfg(feature = "native")]
    {
        let profile = management_profile(profile);
        return run_native(move || {
            native::dynamic_security_mutation_native(profile, command, fields)
        })
        .await;
    }
    #[cfg(not(feature = "native"))]
    {
        let _ = (profile, command, fields);
        Err(native_unavailable("修改 Mosquitto Dynamic Security"))
    }
}

#[cfg(feature = "native")]
fn management_profile(profile: &MqttProfile) -> MqttProfile {
    let mut management_profile = profile.clone();
    if profile.management.admin_username.is_some() || profile.management.admin_password.is_some() {
        management_profile.username = profile.management.admin_username.clone();
        management_profile.password = profile.management.admin_password.clone();
    }
    management_profile
}

#[async_trait]
impl MosquittoStaticConfigDriver for LocalMosquittoStaticConfigDriver {
    async fn read_file(
        &self,
        profile: &MqttProfile,
        kind: MosquittoStaticFileKind,
    ) -> Result<MosquittoStaticFile> {
        ensure_local_target(profile)?;
        let path = kind.configured_path(profile).ok_or_else(|| {
            DomainError::InvalidConfig(format!("未配置 Mosquitto {} 路径", kind.label()))
        })?;
        let path = std::path::Path::new(path);
        if !path.is_absolute() {
            return Err(DomainError::InvalidConfig(
                "Mosquitto 静态配置路径必须是绝对路径".into(),
            ));
        }
        let metadata = std::fs::metadata(path)
            .map_err(|error| local_file_error("读取 Mosquitto 静态配置", error))?;
        if metadata.len() > ramag_domain::entities::MAX_MOSQUITTO_STATIC_FILE_BYTES as u64 {
            return Err(DomainError::InvalidConfig(
                "Mosquitto 静态配置文件超过大小上限".into(),
            ));
        }
        let content = std::fs::read_to_string(path)
            .map_err(|error| local_file_error("读取 Mosquitto 静态配置", error))?;
        Ok(MosquittoStaticFile {
            kind,
            path: path.to_string_lossy().into_owned(),
            content,
        })
    }

    async fn write_file(&self, profile: &MqttProfile, file: &MosquittoStaticFile) -> Result<()> {
        ensure_local_target(profile)?;
        file.validate().map_err(DomainError::InvalidConfig)?;
        let expected = file.kind.configured_path(profile).ok_or_else(|| {
            DomainError::InvalidConfig(format!("未配置 Mosquitto {} 路径", file.kind.label()))
        })?;
        if expected != file.path {
            return Err(DomainError::InvalidConfig(
                "Mosquitto 静态配置路径与配置不一致".into(),
            ));
        }
        let path = std::path::Path::new(&file.path);
        if !path.is_absolute() {
            return Err(DomainError::InvalidConfig(
                "Mosquitto 静态配置路径必须是绝对路径".into(),
            ));
        }
        let parent = path
            .parent()
            .ok_or_else(|| DomainError::InvalidConfig("Mosquitto 静态配置路径缺少父目录".into()))?;
        if !parent.is_dir() {
            return Err(DomainError::InvalidConfig(
                "Mosquitto 静态配置路径的父目录不存在".into(),
            ));
        }
        let suffix = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        );
        let file_name = path
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .ok_or_else(|| DomainError::InvalidConfig("Mosquitto 静态配置文件名无效".into()))?;
        let temporary = parent.join(format!(".{file_name}.ramag-{suffix}.tmp"));
        let backup = parent.join(format!(".{file_name}.ramag-{suffix}.bak"));
        std::fs::write(&temporary, file.content.as_bytes())
            .map_err(|error| local_file_error("写入 Mosquitto 静态配置", error))?;
        let existing = match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_file() => true,
            Ok(_) => {
                let _ = std::fs::remove_file(&temporary);
                return Err(DomainError::InvalidConfig(
                    "Mosquitto 静态配置目标不是普通文件".into(),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                return Err(local_file_error("检查 Mosquitto 静态配置", error));
            }
        };
        if existing && let Err(error) = std::fs::rename(path, &backup) {
            let _ = std::fs::remove_file(&temporary);
            return Err(local_file_error("备份 Mosquitto 静态配置", error));
        }
        if let Err(error) = std::fs::rename(&temporary, path) {
            if existing {
                let _ = std::fs::rename(&backup, path);
            }
            let _ = std::fs::remove_file(&temporary);
            return Err(local_file_error("替换 Mosquitto 静态配置", error));
        }
        if existing && let Err(error) = std::fs::remove_file(&backup) {
            tracing::warn!(path = %file.path, error = %error, "Mosquitto 静态配置备份清理失败");
        }
        Ok(())
    }
}

fn ensure_local_target(profile: &MqttProfile) -> Result<()> {
    if matches!(
        profile
            .management
            .static_config
            .as_ref()
            .map(|config| &config.target),
        Some(ramag_domain::entities::MosquittoConfigTarget::Ssh { .. })
    ) {
        return Err(DomainError::Mqtt(MqttError::new(
            MqttErrorCategory::Unsupported,
            "访问 Mosquitto 静态配置",
            "当前构建只支持本机 Mosquitto 静态配置，SSH 目标尚未接入远程文件适配器",
        )));
    }
    Ok(())
}

fn local_file_error(operation: &'static str, error: std::io::Error) -> DomainError {
    let category = match error.kind() {
        std::io::ErrorKind::NotFound => MqttErrorCategory::NotFound,
        std::io::ErrorKind::PermissionDenied => MqttErrorCategory::PermissionDenied,
        _ => MqttErrorCategory::Unknown,
    };
    DomainError::Mqtt(MqttError::new(
        category,
        operation,
        format!("{operation}失败：{error}"),
    ))
}

#[async_trait]
impl MqttDriver for NativeMqttTransport {
    fn transport_capabilities(&self) -> MqttTransportCapabilities {
        Self::transport_capabilities(self)
    }

    async fn test_connection(&self, profile: &MqttProfile) -> Result<()> {
        #[cfg(feature = "native")]
        {
            let profile = profile.clone();
            return run_native(move || test_connection_native(profile)).await;
        }
        #[cfg(not(feature = "native"))]
        {
            let _ = profile;
            Err(native_unavailable("测试 MQTT 连接"))
        }
    }

    async fn publish(
        &self,
        profile: &MqttProfile,
        request: &MqttPublishRequest,
    ) -> Result<MqttPublishResult> {
        #[cfg(feature = "native")]
        {
            let profile = profile.clone();
            let request = request.clone();
            return run_native(move || publish_native(profile, request)).await;
        }
        #[cfg(not(feature = "native"))]
        {
            let _ = (profile, request);
            Err(native_unavailable("发布 MQTT 消息"))
        }
    }

    async fn subscribe(
        &self,
        profile: &MqttProfile,
        request: &MqttSubscribeRequest,
        sink: MqttMessageSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        #[cfg(feature = "native")]
        {
            let profile = profile.clone();
            let request = request.clone();
            return run_native(move || subscribe_native(profile, request, sink, cancelled)).await;
        }
        #[cfg(not(feature = "native"))]
        {
            let _ = (profile, request, sink, cancelled);
            Err(native_unavailable("订阅 MQTT 消息"))
        }
    }

    async fn broker_snapshot(&self, _profile: &MqttProfile) -> Result<MqttBrokerSnapshot> {
        Err(DomainError::Mqtt(MqttError::new(
            MqttErrorCategory::Unsupported,
            "读取 MQTT Broker 快照",
            "当前 native MQTT 适配器不能从标准 MQTT 数据面读取完整的主题目录或在线客户端列表",
        )))
    }
}

#[cfg(not(feature = "native"))]
fn native_unavailable(operation: &'static str) -> DomainError {
    DomainError::Mqtt(MqttError::new(
        MqttErrorCategory::Unsupported,
        operation,
        "当前构建未启用 MQTT native 客户端；请启用 native feature",
    ))
}

#[cfg(feature = "native")]
async fn run_native<F, Fut, T>(operation: F) -> Result<T>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    smol::unblock(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                DomainError::Mqtt(MqttError::new(
                    MqttErrorCategory::Unknown,
                    "初始化 MQTT 运行时",
                    format!("无法初始化 MQTT 运行时：{error}"),
                ))
            })?;
        runtime
            .block_on(tokio::time::timeout(Duration::from_secs(15), operation()))
            .map_err(|_| {
                DomainError::Mqtt(MqttError::new(
                    MqttErrorCategory::Timeout,
                    "执行 MQTT 操作",
                    "MQTT 操作超时",
                ))
            })?
    })
    .await
}

#[cfg(feature = "native")]
mod native {
    use super::*;
    use chrono::Utc;
    use ramag_domain::entities::{
        MqttMessage, MqttProtocolVersion, MqttQos, MqttTlsConfig, MqttTransport, MqttUserProperty,
        TlsVerify,
    };
    use rumqttc::{AsyncClient, Event, EventLoop, Incoming, MqttOptions, Outgoing, QoS, Transport};
    use std::fs;
    use std::time::Duration;

    const MAX_TLS_FILE_BYTES: u64 = 4 * 1024 * 1024;
    const DYNSEC_REQUEST_TOPIC: &str = "$CONTROL/dynamic-security/v1";
    const DYNSEC_RESPONSE_TOPIC: &str = "$CONTROL/dynamic-security/v1/response";

    type TlsMaterial = (Option<Vec<u8>>, Option<(Vec<u8>, Vec<u8>)>);

    pub(super) async fn test_connection_native(profile: MqttProfile) -> Result<()> {
        match profile.protocol_version {
            MqttProtocolVersion::V311 => {
                let (_, eventloop) = create_v311_client(&profile)?;
                wait_v311_connection(eventloop).await
            }
            MqttProtocolVersion::V5 => {
                let (_, eventloop) = create_v5_client(&profile)?;
                wait_v5_connection(eventloop).await
            }
        }
    }

    pub(super) async fn publish_native(
        profile: MqttProfile,
        request: MqttPublishRequest,
    ) -> Result<MqttPublishResult> {
        match profile.protocol_version {
            MqttProtocolVersion::V311 => publish_v311(&profile, &request).await,
            MqttProtocolVersion::V5 => publish_v5(&profile, &request).await,
        }
    }

    pub(super) async fn subscribe_native(
        profile: MqttProfile,
        request: MqttSubscribeRequest,
        sink: MqttMessageSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        match profile.protocol_version {
            MqttProtocolVersion::V311 => subscribe_v311(&profile, &request, sink, cancelled).await,
            MqttProtocolVersion::V5 => subscribe_v5(&profile, &request, sink, cancelled).await,
        }
    }

    pub(super) async fn dynamic_security_snapshot_native(
        profile: MqttProfile,
    ) -> Result<MosquittoDynamicSecuritySnapshot> {
        let clients = dynamic_security_request(
            &profile,
            serde_json::json!({"command": "listClients", "verbose": true}),
        )
        .await?;
        let groups = dynamic_security_request(
            &profile,
            serde_json::json!({"command": "listGroups", "verbose": true}),
        )
        .await?;
        let roles = dynamic_security_request(
            &profile,
            serde_json::json!({"command": "listRoles", "verbose": true}),
        )
        .await?;
        Ok(MosquittoDynamicSecuritySnapshot {
            clients: parse_clients(&clients)?,
            groups: parse_groups(&groups)?,
            roles: parse_roles(&roles)?,
        })
    }

    pub(super) async fn save_client_native(
        profile: MqttProfile,
        client: MosquittoClient,
    ) -> Result<()> {
        let existing = dynamic_security_request(
            &profile,
            serde_json::json!({"command": "getClient", "username": client.username}),
        )
        .await;
        let existed = match existing {
            Ok(_) => true,
            Err(error) if is_not_found(&error) => false,
            Err(error) => return Err(error),
        };
        dynamic_security_request(&profile, client_command(&client, existed)).await?;

        if client.disabled {
            dynamic_security_request(
                &profile,
                serde_json::json!({
                    "command": "disableClient",
                    "username": client.username,
                }),
            )
            .await?;
        } else if existed {
            dynamic_security_request(
                &profile,
                serde_json::json!({
                    "command": "enableClient",
                    "username": client.username,
                }),
            )
            .await?;
        }
        Ok(())
    }

    pub(super) async fn save_group_native(
        profile: MqttProfile,
        group: MosquittoGroup,
    ) -> Result<()> {
        let existing = dynamic_security_request(
            &profile,
            serde_json::json!({"command": "getGroup", "groupname": group.group_name}),
        )
        .await;
        let existed = match existing {
            Ok(_) => true,
            Err(error) if is_not_found(&error) => false,
            Err(error) => return Err(error),
        };
        dynamic_security_request(&profile, group_command(&group, existed))
            .await
            .map(|_| ())
    }

    pub(super) async fn save_role_native(profile: MqttProfile, role: MosquittoRole) -> Result<()> {
        let existing = dynamic_security_request(
            &profile,
            serde_json::json!({"command": "getRole", "rolename": role.role_name}),
        )
        .await;
        let existed = match existing {
            Ok(_) => true,
            Err(error) if is_not_found(&error) => false,
            Err(error) => return Err(error),
        };
        dynamic_security_request(&profile, role_command(&role, existed))
            .await
            .map(|_| ())
    }

    pub(super) async fn dynamic_security_mutation_native(
        profile: MqttProfile,
        command: &'static str,
        fields: serde_json::Value,
    ) -> Result<()> {
        let mut object = fields.as_object().cloned().ok_or_else(|| {
            DomainError::InvalidConfig("Mosquitto 管理命令字段必须是 JSON 对象".into())
        })?;
        object.insert("command".into(), serde_json::Value::String(command.into()));
        dynamic_security_request(&profile, serde_json::Value::Object(object))
            .await
            .map(|_| ())
    }

    fn client_command(client: &MosquittoClient, existing: bool) -> serde_json::Value {
        let mut object = serde_json::Map::new();
        object.insert("username".into(), client.username.clone().into());
        if let Some(client_id) = &client.client_id {
            object.insert("clientid".into(), client_id.clone().into());
        }
        if let Some(text_name) = &client.text_name {
            object.insert("textname".into(), text_name.clone().into());
        }
        if let Some(text_description) = &client.text_description {
            object.insert("textdescription".into(), text_description.clone().into());
        }
        object.insert(
            "roles".into(),
            serde_json::Value::Array(
                client
                    .roles
                    .iter()
                    .map(|binding| {
                        serde_json::json!({
                            "rolename": binding.role_name,
                            "priority": binding.priority,
                        })
                    })
                    .collect(),
            ),
        );
        object.insert(
            "groups".into(),
            serde_json::Value::Array(
                client
                    .groups
                    .iter()
                    .map(|binding| {
                        serde_json::json!({
                            "groupname": binding.group_name,
                            "priority": binding.priority,
                        })
                    })
                    .collect(),
            ),
        );
        if let Some(password) = &client.password {
            object.insert("password".into(), password.clone().into());
        }
        object.insert(
            "command".into(),
            serde_json::Value::String(
                if existing {
                    "modifyClient"
                } else {
                    "createClient"
                }
                .into(),
            ),
        );
        serde_json::Value::Object(object)
    }

    fn group_command(group: &MosquittoGroup, existing: bool) -> serde_json::Value {
        let mut object = serde_json::Map::new();
        object.insert(
            "command".into(),
            serde_json::Value::String(
                if existing {
                    "modifyGroup"
                } else {
                    "createGroup"
                }
                .into(),
            ),
        );
        object.insert("groupname".into(), group.group_name.clone().into());
        if let Some(text_name) = &group.text_name {
            object.insert("textname".into(), text_name.clone().into());
        }
        if let Some(text_description) = &group.text_description {
            object.insert("textdescription".into(), text_description.clone().into());
        }
        object.insert(
            "roles".into(),
            group
                .roles
                .iter()
                .map(|binding| {
                    serde_json::json!({
                        "rolename": binding.role_name,
                        "priority": binding.priority,
                    })
                })
                .collect::<Vec<_>>()
                .into(),
        );
        serde_json::Value::Object(object)
    }

    fn role_command(role: &MosquittoRole, existing: bool) -> serde_json::Value {
        let mut object = serde_json::Map::new();
        object.insert(
            "command".into(),
            serde_json::Value::String(if existing { "modifyRole" } else { "createRole" }.into()),
        );
        object.insert("rolename".into(), role.role_name.clone().into());
        if let Some(text_name) = &role.text_name {
            object.insert("textname".into(), text_name.clone().into());
        }
        if let Some(text_description) = &role.text_description {
            object.insert("textdescription".into(), text_description.clone().into());
        }
        object.insert(
            "allowwildcardsubs".into(),
            role.allow_wildcards_subscriptions.into(),
        );
        object.insert(
            "acls".into(),
            role.acls
                .iter()
                .map(|acl| {
                    serde_json::json!({
                        "acltype": acl.acl_type.as_str(),
                        "topic": acl.topic,
                        "allow": matches!(acl.decision, ramag_domain::entities::MosquittoAclDecision::Allow),
                        "priority": acl.priority,
                    })
                })
                .collect::<Vec<_>>()
                .into(),
        );
        serde_json::Value::Object(object)
    }

    async fn dynamic_security_request(
        profile: &MqttProfile,
        command: serde_json::Value,
    ) -> Result<serde_json::Value> {
        match profile.protocol_version {
            MqttProtocolVersion::V311 => dynamic_security_request_v311(profile, command).await,
            MqttProtocolVersion::V5 => dynamic_security_request_v5(profile, command).await,
        }
    }

    async fn dynamic_security_request_v311(
        profile: &MqttProfile,
        command: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let payload =
            serde_json::to_vec(&serde_json::json!({"commands": [command]})).map_err(|error| {
                DomainError::InvalidConfig(format!("编码 Mosquitto 管理命令失败：{error}"))
            })?;
        let (client, mut eventloop) = create_v311_client(profile)?;
        let mut subscribe_sent = false;
        let mut publish_sent = false;
        loop {
            match eventloop
                .poll()
                .await
                .map_err(|error| mqtt_connection_error(error.to_string()))?
            {
                Event::Incoming(Incoming::ConnAck(_)) if !subscribe_sent => {
                    client
                        .subscribe(DYNSEC_RESPONSE_TOPIC, QoS::AtLeastOnce)
                        .await
                        .map_err(|error| {
                            mqtt_client_error("订阅 Mosquitto 管理响应", error.to_string())
                        })?;
                    subscribe_sent = true;
                }
                Event::Incoming(Incoming::SubAck(_)) if !publish_sent => {
                    client
                        .publish(
                            DYNSEC_REQUEST_TOPIC,
                            QoS::AtLeastOnce,
                            false,
                            payload.clone(),
                        )
                        .await
                        .map_err(|error| {
                            mqtt_client_error("发布 Mosquitto 管理命令", error.to_string())
                        })?;
                    publish_sent = true;
                }
                Event::Incoming(Incoming::Publish(publish))
                    if publish.topic == DYNSEC_RESPONSE_TOPIC =>
                {
                    return decode_dynamic_security_response(&publish.payload);
                }
                Event::Incoming(_) | Event::Outgoing(_) => {}
            }
        }
    }

    async fn dynamic_security_request_v5(
        profile: &MqttProfile,
        command: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let payload =
            serde_json::to_vec(&serde_json::json!({"commands": [command]})).map_err(|error| {
                DomainError::InvalidConfig(format!("编码 Mosquitto 管理命令失败：{error}"))
            })?;
        let (client, mut eventloop) = create_v5_client(profile)?;
        let mut subscribe_sent = false;
        let mut publish_sent = false;
        loop {
            match eventloop
                .poll()
                .await
                .map_err(|error| mqtt_connection_error(error.to_string()))?
            {
                rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::ConnAck(_))
                    if !subscribe_sent =>
                {
                    client
                        .subscribe(
                            DYNSEC_RESPONSE_TOPIC,
                            rumqttc::v5::mqttbytes::QoS::AtLeastOnce,
                        )
                        .await
                        .map_err(|error| {
                            mqtt_client_error("订阅 Mosquitto 管理响应", error.to_string())
                        })?;
                    subscribe_sent = true;
                }
                rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::SubAck(_)) if !publish_sent => {
                    client
                        .publish(
                            DYNSEC_REQUEST_TOPIC,
                            rumqttc::v5::mqttbytes::QoS::AtLeastOnce,
                            false,
                            bytes::Bytes::from(payload.clone()),
                        )
                        .await
                        .map_err(|error| {
                            mqtt_client_error("发布 Mosquitto 管理命令", error.to_string())
                        })?;
                    publish_sent = true;
                }
                rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::Publish(publish))
                    if publish.topic.as_ref() == DYNSEC_RESPONSE_TOPIC.as_bytes() =>
                {
                    return decode_dynamic_security_response(&publish.payload);
                }
                rumqttc::v5::Event::Incoming(_) | rumqttc::v5::Event::Outgoing(_) => {}
            }
        }
    }

    fn decode_dynamic_security_response(payload: &[u8]) -> Result<serde_json::Value> {
        let tree: serde_json::Value = serde_json::from_slice(payload).map_err(|error| {
            DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Protocol,
                "解析 Mosquitto 管理响应",
                format!("Mosquitto 管理响应不是有效 JSON：{error}"),
            ))
        })?;
        let response = tree
            .get("responses")
            .and_then(serde_json::Value::as_array)
            .and_then(|responses| responses.first())
            .ok_or_else(|| {
                DomainError::Mqtt(MqttError::new(
                    MqttErrorCategory::Protocol,
                    "解析 Mosquitto 管理响应",
                    "Mosquitto 管理响应缺少 responses 数组",
                ))
            })?;
        if let Some(error) = response.get("error").and_then(serde_json::Value::as_str) {
            let category = broker_error_category(error);
            return Err(DomainError::Mqtt(MqttError::new(
                category,
                "执行 Mosquitto 管理命令",
                format!("Mosquitto 管理命令被 Broker 拒绝：{error}"),
            )));
        }
        Ok(response.clone())
    }

    fn response_data(response: &serde_json::Value) -> Result<&serde_json::Value> {
        response.get("data").ok_or_else(|| {
            DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Protocol,
                "解析 Mosquitto 管理响应",
                "Mosquitto 管理响应缺少 data 对象",
            ))
        })
    }

    fn parse_clients(response: &serde_json::Value) -> Result<Vec<MosquittoClient>> {
        let values = response_data(response)?
            .get("clients")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid_management_response("clients"))?;
        values.iter().map(parse_client).collect()
    }

    fn parse_client(value: &serde_json::Value) -> Result<MosquittoClient> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_management_response("client"))?;
        Ok(MosquittoClient {
            username: required_string(object, "username")?,
            client_id: optional_string(object, "clientid"),
            password_configured: object
                .get("password_configured")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            password: None,
            disabled: object
                .get("disabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            text_name: optional_string(object, "textname"),
            text_description: optional_string(object, "textdescription"),
            groups: parse_group_bindings(object.get("groups"))?,
            roles: parse_role_bindings(object.get("roles"))?,
        })
    }

    fn parse_groups(response: &serde_json::Value) -> Result<Vec<MosquittoGroup>> {
        let values = response_data(response)?
            .get("groups")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid_management_response("groups"))?;
        values.iter().map(parse_group).collect()
    }

    fn parse_group(value: &serde_json::Value) -> Result<MosquittoGroup> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_management_response("group"))?;
        Ok(MosquittoGroup {
            group_name: required_string(object, "groupname")?,
            text_name: optional_string(object, "textname"),
            text_description: optional_string(object, "textdescription"),
            roles: parse_role_bindings(object.get("roles"))?,
        })
    }

    fn parse_roles(response: &serde_json::Value) -> Result<Vec<MosquittoRole>> {
        let values = response_data(response)?
            .get("roles")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid_management_response("roles"))?;
        values.iter().map(parse_role).collect()
    }

    fn parse_role(value: &serde_json::Value) -> Result<MosquittoRole> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_management_response("role"))?;
        let acls = object
            .get("acls")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid_management_response("role.acls"))?
            .iter()
            .map(|value| {
                let acl = value
                    .as_object()
                    .ok_or_else(|| invalid_management_response("role.acl"))?;
                let acl_type = match required_string(acl, "acltype")?.as_str() {
                    "publishClientSend" => {
                        ramag_domain::entities::MosquittoAclType::PublishClientSend
                    }
                    "publishClientReceive" => {
                        ramag_domain::entities::MosquittoAclType::PublishClientReceive
                    }
                    "subscribeLiteral" => {
                        ramag_domain::entities::MosquittoAclType::SubscribeLiteral
                    }
                    "subscribePattern" => {
                        ramag_domain::entities::MosquittoAclType::SubscribePattern
                    }
                    "unsubscribeLiteral" => {
                        ramag_domain::entities::MosquittoAclType::UnsubscribeLiteral
                    }
                    "unsubscribePattern" => {
                        ramag_domain::entities::MosquittoAclType::UnsubscribePattern
                    }
                    _ => return Err(invalid_management_response("role.acl.acltype")),
                };
                Ok(ramag_domain::entities::MosquittoAcl {
                    acl_type,
                    topic: required_string(acl, "topic")?,
                    decision: if acl
                        .get("allow")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                    {
                        ramag_domain::entities::MosquittoAclDecision::Allow
                    } else {
                        ramag_domain::entities::MosquittoAclDecision::Deny
                    },
                    priority: acl
                        .get("priority")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(-1) as i32,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(MosquittoRole {
            role_name: required_string(object, "rolename")?,
            text_name: optional_string(object, "textname"),
            text_description: optional_string(object, "textdescription"),
            allow_wildcards_subscriptions: object
                .get("allowwildcardsubs")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            acls,
        })
    }

    fn parse_role_bindings(
        value: Option<&serde_json::Value>,
    ) -> Result<Vec<ramag_domain::entities::MosquittoRoleBinding>> {
        let Some(values) = value.and_then(serde_json::Value::as_array) else {
            return Ok(Vec::new());
        };
        values
            .iter()
            .map(|value| {
                let object = value
                    .as_object()
                    .ok_or_else(|| invalid_management_response("roles"))?;
                let priority = object
                    .get("priority")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(-1) as i32;
                Ok(ramag_domain::entities::MosquittoRoleBinding {
                    role_name: required_string(object, "rolename")?,
                    priority,
                })
            })
            .collect()
    }

    fn parse_group_bindings(
        value: Option<&serde_json::Value>,
    ) -> Result<Vec<ramag_domain::entities::MosquittoGroupBinding>> {
        let Some(values) = value.and_then(serde_json::Value::as_array) else {
            return Ok(Vec::new());
        };
        values
            .iter()
            .map(|value| {
                let object = value
                    .as_object()
                    .ok_or_else(|| invalid_management_response("groups"))?;
                Ok(ramag_domain::entities::MosquittoGroupBinding {
                    group_name: required_string(object, "groupname")?,
                    priority: object
                        .get("priority")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(-1) as i32,
                })
            })
            .collect()
    }

    fn required_string(
        object: &serde_json::Map<String, serde_json::Value>,
        key: &str,
    ) -> Result<String> {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| invalid_management_response(key))
    }

    fn optional_string(
        object: &serde_json::Map<String, serde_json::Value>,
        key: &str,
    ) -> Option<String> {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
    }

    fn invalid_management_response(field: &str) -> DomainError {
        DomainError::Mqtt(MqttError::new(
            MqttErrorCategory::Protocol,
            "解析 Mosquitto 管理响应",
            format!("Mosquitto 管理响应缺少有效字段：{field}"),
        ))
    }

    fn is_not_found(error: &DomainError) -> bool {
        matches!(
            error,
            DomainError::Mqtt(mqtt_error) if mqtt_error.category == MqttErrorCategory::NotFound
        ) || error.user_message().contains("not found")
            || error.user_message().contains("不存在")
    }

    fn broker_error_category(error: &str) -> MqttErrorCategory {
        let normalized = error.to_ascii_lowercase();
        if normalized.contains("not found") || error.contains("不存在") {
            MqttErrorCategory::NotFound
        } else if normalized.contains("not authorised")
            || normalized.contains("not authorized")
            || normalized.contains("permission")
            || normalized.contains("access denied")
        {
            MqttErrorCategory::PermissionDenied
        } else if normalized.contains("unknown command")
            || normalized.contains("unsupported")
            || normalized.contains("dynamic security") && normalized.contains("not enabled")
        {
            MqttErrorCategory::Unsupported
        } else {
            MqttErrorCategory::Protocol
        }
    }

    fn create_v311_client(profile: &MqttProfile) -> Result<(AsyncClient, EventLoop)> {
        let mut options = MqttOptions::new(client_id(profile), &profile.host, profile.port);
        options
            .set_keep_alive(Duration::from_secs(u64::from(profile.keep_alive_seconds)))
            .set_clean_session(profile.clean_start)
            .set_max_packet_size(
                ramag_domain::entities::MAX_MQTT_PUBLISH_PAYLOAD_BYTES,
                ramag_domain::entities::MAX_MQTT_PUBLISH_PAYLOAD_BYTES,
            );
        set_v311_credentials(&mut options, profile);
        if profile.transport == MqttTransport::Tls {
            options.set_transport(build_v311_transport(&profile.tls)?);
        }
        Ok(AsyncClient::new(options, EVENT_LOOP_CAPACITY))
    }

    fn create_v5_client(
        profile: &MqttProfile,
    ) -> Result<(rumqttc::v5::AsyncClient, rumqttc::v5::EventLoop)> {
        let mut options =
            rumqttc::v5::MqttOptions::new(client_id(profile), &profile.host, profile.port);
        options
            .set_keep_alive(Duration::from_secs(u64::from(profile.keep_alive_seconds)))
            .set_clean_start(profile.clean_start)
            .set_session_expiry_interval(profile.session_expiry_seconds)
            .set_max_packet_size(Some(
                ramag_domain::entities::MAX_MQTT_PUBLISH_PAYLOAD_BYTES as u32,
            ));
        set_v5_credentials(&mut options, profile);
        if profile.transport == MqttTransport::Tls {
            options.set_transport(build_v5_transport(&profile.tls)?);
        }
        Ok(rumqttc::v5::AsyncClient::new(options, EVENT_LOOP_CAPACITY))
    }

    fn client_id(profile: &MqttProfile) -> String {
        profile
            .client_id
            .clone()
            .unwrap_or_else(|| format!("ramag-{}", profile.id))
    }

    fn set_v311_credentials(options: &mut MqttOptions, profile: &MqttProfile) {
        if let (Some(username), Some(password)) = (&profile.username, &profile.password) {
            options.set_credentials(username, password);
        }
    }

    fn set_v5_credentials(options: &mut rumqttc::v5::MqttOptions, profile: &MqttProfile) {
        if let (Some(username), Some(password)) = (&profile.username, &profile.password) {
            options.set_credentials(username, password);
        }
    }

    async fn wait_v311_connection(mut eventloop: EventLoop) -> Result<()> {
        async move {
            loop {
                match eventloop
                    .poll()
                    .await
                    .map_err(|error| mqtt_connection_error(error.to_string()))?
                {
                    Event::Incoming(Incoming::ConnAck(_)) => return Ok(()),
                    Event::Incoming(_) | Event::Outgoing(_) => {}
                }
            }
        }
        .await
    }

    async fn wait_v5_connection(mut eventloop: rumqttc::v5::EventLoop) -> Result<()> {
        async move {
            loop {
                match eventloop
                    .poll()
                    .await
                    .map_err(|error| mqtt_connection_error(error.to_string()))?
                {
                    rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::ConnAck(_)) => {
                        return Ok(());
                    }
                    rumqttc::v5::Event::Incoming(_) | rumqttc::v5::Event::Outgoing(_) => {}
                }
            }
        }
        .await
    }

    async fn publish_v311(
        profile: &MqttProfile,
        request: &MqttPublishRequest,
    ) -> Result<MqttPublishResult> {
        let (client, mut eventloop) = create_v311_client(profile)?;
        let qos = qos_v311(request.qos);
        async move {
            client
                .publish(&request.topic, qos, request.retain, request.payload.clone())
                .await
                .map_err(|error| mqtt_client_error("发布 MQTT 3.1.1 消息", error.to_string()))?;
            let mut connected = false;
            loop {
                match eventloop
                    .poll()
                    .await
                    .map_err(|error| mqtt_connection_error(error.to_string()))?
                {
                    Event::Incoming(Incoming::ConnAck(_)) => connected = true,
                    Event::Outgoing(Outgoing::Publish(packet_id))
                        if connected && qos == QoS::AtMostOnce =>
                    {
                        return Ok(MqttPublishResult {
                            topic: request.topic.clone(),
                            packet_id: nonzero_packet_id(packet_id),
                            qos: request.qos,
                        });
                    }
                    Event::Incoming(Incoming::PubAck(ack))
                        if connected && qos == QoS::AtLeastOnce =>
                    {
                        return Ok(MqttPublishResult {
                            topic: request.topic.clone(),
                            packet_id: Some(ack.pkid),
                            qos: request.qos,
                        });
                    }
                    Event::Incoming(Incoming::PubComp(ack))
                        if connected && qos == QoS::ExactlyOnce =>
                    {
                        return Ok(MqttPublishResult {
                            topic: request.topic.clone(),
                            packet_id: Some(ack.pkid),
                            qos: request.qos,
                        });
                    }
                    Event::Incoming(_) | Event::Outgoing(_) => {}
                }
            }
        }
        .await
    }

    async fn publish_v5(
        profile: &MqttProfile,
        request: &MqttPublishRequest,
    ) -> Result<MqttPublishResult> {
        let (client, mut eventloop) = create_v5_client(profile)?;
        let qos = qos_v5(request.qos);
        let properties = if request.user_properties.is_empty() {
            None
        } else {
            Some(rumqttc::v5::mqttbytes::v5::PublishProperties {
                user_properties: user_properties(request),
                ..Default::default()
            })
        };
        async move {
            match properties {
                Some(properties) => {
                    client
                        .publish_with_properties(
                            &request.topic,
                            qos,
                            request.retain,
                            bytes::Bytes::from(request.payload.clone()),
                            properties,
                        )
                        .await
                }
                None => {
                    client
                        .publish(
                            &request.topic,
                            qos,
                            request.retain,
                            bytes::Bytes::from(request.payload.clone()),
                        )
                        .await
                }
            }
            .map_err(|error| mqtt_client_error("发布 MQTT 5 消息", error.to_string()))?;
            let mut connected = false;
            loop {
                match eventloop
                    .poll()
                    .await
                    .map_err(|error| mqtt_connection_error(error.to_string()))?
                {
                    rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::ConnAck(_)) => {
                        connected = true
                    }
                    rumqttc::v5::Event::Outgoing(Outgoing::Publish(packet_id))
                        if connected && qos == rumqttc::v5::mqttbytes::QoS::AtMostOnce =>
                    {
                        return Ok(MqttPublishResult {
                            topic: request.topic.clone(),
                            packet_id: nonzero_packet_id(packet_id),
                            qos: request.qos,
                        });
                    }
                    rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::PubAck(ack))
                        if connected && qos == rumqttc::v5::mqttbytes::QoS::AtLeastOnce =>
                    {
                        return Ok(MqttPublishResult {
                            topic: request.topic.clone(),
                            packet_id: Some(ack.pkid),
                            qos: request.qos,
                        });
                    }
                    rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::PubComp(ack))
                        if connected && qos == rumqttc::v5::mqttbytes::QoS::ExactlyOnce =>
                    {
                        return Ok(MqttPublishResult {
                            topic: request.topic.clone(),
                            packet_id: Some(ack.pkid),
                            qos: request.qos,
                        });
                    }
                    rumqttc::v5::Event::Incoming(_) | rumqttc::v5::Event::Outgoing(_) => {}
                }
            }
        }
        .await
    }

    async fn subscribe_v311(
        profile: &MqttProfile,
        request: &MqttSubscribeRequest,
        sink: MqttMessageSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        let (client, eventloop) = create_v311_client(profile)?;
        let filters = request
            .subscriptions
            .iter()
            .map(|subscription| {
                rumqttc::SubscribeFilter::new(
                    subscription.filter.clone(),
                    qos_v311(subscription.qos),
                )
            })
            .collect::<Vec<_>>();
        run_subscription_v311(client, eventloop, filters, sink, cancelled).await
    }

    async fn subscribe_v5(
        profile: &MqttProfile,
        request: &MqttSubscribeRequest,
        sink: MqttMessageSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        let (client, eventloop) = create_v5_client(profile)?;
        let filters = request
            .subscriptions
            .iter()
            .map(|subscription| {
                rumqttc::v5::mqttbytes::v5::Filter::new(
                    subscription.filter.clone(),
                    qos_v5(subscription.qos),
                )
            })
            .collect::<Vec<_>>();
        run_subscription_v5(client, eventloop, filters, sink, cancelled).await
    }

    async fn run_subscription_v311(
        client: AsyncClient,
        mut eventloop: EventLoop,
        filters: Vec<rumqttc::SubscribeFilter>,
        sink: MqttMessageSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        async move {
            client
                .subscribe_many(filters)
                .await
                .map_err(|error| mqtt_client_error("订阅 MQTT 3.1.1 Topic", error.to_string()))?;
            loop {
                if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                    return Ok(());
                }
                match eventloop
                    .poll()
                    .await
                    .map_err(|error| mqtt_connection_error(error.to_string()))?
                {
                    Event::Incoming(Incoming::Publish(publish)) => {
                        let message = MqttMessage {
                            topic: publish.topic,
                            payload: publish.payload.to_vec(),
                            qos: qos_from_v311(publish.qos),
                            retain: publish.retain,
                            duplicate: publish.dup,
                            received_at: Utc::now(),
                            user_properties: vec![],
                        };
                        message.validate().map_err(DomainError::InvalidConfig)?;
                        if matches!(
                            sink(message),
                            ramag_domain::entities::MqttMessageSinkResult::Closed
                        ) {
                            return Ok(());
                        }
                    }
                    Event::Incoming(_) | Event::Outgoing(_) => {}
                }
            }
        }
        .await
    }

    async fn run_subscription_v5(
        client: rumqttc::v5::AsyncClient,
        mut eventloop: rumqttc::v5::EventLoop,
        filters: Vec<rumqttc::v5::mqttbytes::v5::Filter>,
        sink: MqttMessageSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        async move {
            client
                .subscribe_many(filters)
                .await
                .map_err(|error| mqtt_client_error("订阅 MQTT 5 Topic", error.to_string()))?;
            loop {
                if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                    return Ok(());
                }
                match eventloop
                    .poll()
                    .await
                    .map_err(|error| mqtt_connection_error(error.to_string()))?
                {
                    rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::Publish(publish)) => {
                        let topic = String::from_utf8(publish.topic.to_vec()).map_err(|_| {
                            DomainError::Mqtt(MqttError::new(
                                MqttErrorCategory::Protocol,
                                "接收 MQTT 5 消息",
                                "Broker 返回了无效的 Topic 编码",
                            ))
                        })?;
                        let user_properties =
                            publish
                                .properties
                                .as_ref()
                                .map_or_else(Vec::new, |properties| {
                                    properties
                                        .user_properties
                                        .iter()
                                        .map(|(name, value)| MqttUserProperty {
                                            name: name.clone(),
                                            value: value.clone(),
                                        })
                                        .collect()
                                });
                        let message = MqttMessage {
                            topic,
                            payload: publish.payload.to_vec(),
                            qos: qos_from_v5(publish.qos),
                            retain: publish.retain,
                            duplicate: publish.dup,
                            received_at: Utc::now(),
                            user_properties,
                        };
                        message.validate().map_err(DomainError::InvalidConfig)?;
                        if matches!(
                            sink(message),
                            ramag_domain::entities::MqttMessageSinkResult::Closed
                        ) {
                            return Ok(());
                        }
                    }
                    rumqttc::v5::Event::Incoming(_) | rumqttc::v5::Event::Outgoing(_) => {}
                }
            }
        }
        .await
    }

    fn qos_v311(qos: MqttQos) -> QoS {
        match qos {
            MqttQos::AtMostOnce => QoS::AtMostOnce,
            MqttQos::AtLeastOnce => QoS::AtLeastOnce,
            MqttQos::ExactlyOnce => QoS::ExactlyOnce,
        }
    }

    fn qos_v5(qos: MqttQos) -> rumqttc::v5::mqttbytes::QoS {
        match qos {
            MqttQos::AtMostOnce => rumqttc::v5::mqttbytes::QoS::AtMostOnce,
            MqttQos::AtLeastOnce => rumqttc::v5::mqttbytes::QoS::AtLeastOnce,
            MqttQos::ExactlyOnce => rumqttc::v5::mqttbytes::QoS::ExactlyOnce,
        }
    }

    fn qos_from_v311(qos: QoS) -> MqttQos {
        match qos {
            QoS::AtMostOnce => MqttQos::AtMostOnce,
            QoS::AtLeastOnce => MqttQos::AtLeastOnce,
            QoS::ExactlyOnce => MqttQos::ExactlyOnce,
        }
    }

    fn qos_from_v5(qos: rumqttc::v5::mqttbytes::QoS) -> MqttQos {
        match qos {
            rumqttc::v5::mqttbytes::QoS::AtMostOnce => MqttQos::AtMostOnce,
            rumqttc::v5::mqttbytes::QoS::AtLeastOnce => MqttQos::AtLeastOnce,
            rumqttc::v5::mqttbytes::QoS::ExactlyOnce => MqttQos::ExactlyOnce,
        }
    }

    fn nonzero_packet_id(packet_id: u16) -> Option<u16> {
        (packet_id != 0).then_some(packet_id)
    }

    fn user_properties(request: &MqttPublishRequest) -> Vec<(String, String)> {
        request
            .user_properties
            .iter()
            .map(|property| (property.name.clone(), property.value.clone()))
            .collect()
    }

    fn build_v311_transport(tls: &MqttTlsConfig) -> Result<Transport> {
        let (ca, client_auth) = load_tls_material(tls)?;
        if tls.verify == TlsVerify::None {
            return Err(DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Unsupported,
                "创建 MQTT TLS 连接",
                "MQTT native 适配器不允许关闭服务器证书校验",
            )));
        }
        if tls.verify == TlsVerify::Ca {
            return Err(DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Unsupported,
                "创建 MQTT TLS 连接",
                "当前 MQTT native 适配器暂不支持仅校验证书链而跳过主机名校验",
            )));
        }
        Ok(match (ca, client_auth) {
            (None, None) => Transport::tls_with_default_config(),
            (Some(ca), auth) => Transport::tls(ca, auth, None),
            (None, Some(_)) => {
                return Err(DomainError::InvalidConfig(
                    "配置客户端证书时必须同时配置 CA 证书".into(),
                ));
            }
        })
    }

    fn build_v5_transport(tls: &MqttTlsConfig) -> Result<Transport> {
        build_v311_transport(tls)
    }

    fn load_tls_material(tls: &MqttTlsConfig) -> Result<TlsMaterial> {
        let ca = read_tls_file(tls.ca_cert_path.as_deref())?;
        let client_cert = read_tls_file(tls.client_cert_path.as_deref())?;
        let client_key = read_tls_file(tls.client_key_path.as_deref())?;
        let client_auth = match (client_cert, client_key) {
            (None, None) => None,
            (Some(cert), Some(key)) => Some((cert, key)),
            _ => {
                return Err(DomainError::InvalidConfig(
                    "客户端证书和客户端密钥必须同时配置".into(),
                ));
            }
        };
        Ok((ca, client_auth))
    }

    fn read_tls_file(path: Option<&str>) -> Result<Option<Vec<u8>>> {
        let Some(path) = path else {
            return Ok(None);
        };
        let metadata = fs::metadata(path).map_err(|_| {
            DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Tls,
                "读取 MQTT TLS 文件",
                "无法读取 MQTT TLS 文件",
            ))
        })?;
        if metadata.len() > MAX_TLS_FILE_BYTES {
            return Err(DomainError::InvalidConfig(
                "MQTT TLS 文件超过 4 MiB 上限".into(),
            ));
        }
        let contents = fs::read(path).map_err(|_| {
            DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Tls,
                "读取 MQTT TLS 文件",
                "无法读取 MQTT TLS 文件",
            ))
        })?;
        Ok(Some(contents))
    }

    fn mqtt_connection_error(message: String) -> DomainError {
        DomainError::Mqtt(MqttError::new(
            MqttErrorCategory::Network,
            "连接 MQTT Broker",
            format!("连接 MQTT Broker 失败：{message}"),
        ))
    }

    fn mqtt_client_error(operation: &'static str, message: String) -> DomainError {
        DomainError::Mqtt(MqttError::new(
            MqttErrorCategory::Protocol,
            operation,
            format!("{operation}失败：{message}"),
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn dynamic_security_commands_keep_secrets_out_of_debug_and_preserve_optional_fields() {
            let client = MosquittoClient {
                username: "operator".into(),
                client_id: None,
                password_configured: false,
                password: Some("secret-password".into()),
                disabled: false,
                text_name: Some("Operator".into()),
                text_description: None,
                groups: Vec::new(),
                roles: Vec::new(),
            };

            let command = client_command(&client, true);
            assert_eq!(command["command"], "modifyClient");
            assert_eq!(command["username"], "operator");
            assert_eq!(command["password"], "secret-password");
            assert!(command.get("clientid").is_none());
            assert!(command.get("textdescription").is_none());
            assert!(!format!("{client:?}").contains("secret-password"));
        }

        #[test]
        fn role_commands_send_the_broker_wildcard_flag_and_supported_acl_types() {
            let role = MosquittoRole {
                role_name: "operator".into(),
                text_name: None,
                text_description: None,
                allow_wildcards_subscriptions: true,
                acls: vec![ramag_domain::entities::MosquittoAcl {
                    acl_type: ramag_domain::entities::MosquittoAclType::SubscribePattern,
                    topic: "devices/%u/#".into(),
                    decision: ramag_domain::entities::MosquittoAclDecision::Allow,
                    priority: 10,
                }],
            };

            let command = role_command(&role, false);
            assert_eq!(command["command"], "createRole");
            assert_eq!(command["allowwildcardsubs"], true);
            assert_eq!(command["acls"][0]["acltype"], "subscribePattern");
        }

        #[test]
        fn dynamic_security_response_errors_are_classified_by_broker_reason() {
            let not_found = match decode_dynamic_security_response(
                br#"{"responses":[{"error":"Client not found"}]}"#,
            ) {
                Err(DomainError::Mqtt(error)) => Some(error.category),
                _ => None,
            };
            assert_eq!(not_found, Some(MqttErrorCategory::NotFound));

            let permission = match decode_dynamic_security_response(
                br#"{"responses":[{"error":"Not authorised"}]}"#,
            ) {
                Err(DomainError::Mqtt(error)) => Some(error.category),
                _ => None,
            };
            assert_eq!(permission, Some(MqttErrorCategory::PermissionDenied));
        }

        #[test]
        fn role_parser_rejects_unsupported_acl_aliases_and_reads_wildcard_flag() {
            let response = serde_json::json!({
                "data": {
                    "roles": [{
                        "rolename": "operator",
                        "allowwildcardsubs": true,
                        "acls": [{
                            "acltype": "subscribePattern",
                            "topic": "devices/%u/#",
                            "allow": true,
                            "priority": 1
                        }]
                    }]
                }
            });
            let allows_wildcards = parse_roles(&response)
                .ok()
                .and_then(|roles| roles.first().map(|role| role.allow_wildcards_subscriptions));
            assert_eq!(allows_wildcards, Some(true));

            let unsupported = serde_json::json!({
                "data": {
                    "roles": [{
                        "rolename": "operator",
                        "acls": [{
                            "acltype": "subscribe",
                            "topic": "devices/#",
                            "allow": true
                        }]
                    }]
                }
            });
            assert!(parse_roles(&unsupported).is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_build_exposes_native_capability_without_claiming_runtime_support() {
        let capabilities = NativeMqttTransport::new().transport_capabilities();
        assert_eq!(capabilities.backend, MqttTransportBackend::Native);
        assert_eq!(capabilities.build_available, cfg!(feature = "native"));
    }

    #[cfg(feature = "native")]
    #[test]
    fn management_profile_prefers_dedicated_management_credentials() {
        let mut profile = MqttProfile::new("local", "127.0.0.1", 1883);
        profile.username = Some("data-user".into());
        profile.password = Some("data-password".into());
        profile.management.admin_username = Some("admin".into());
        profile.management.admin_password = Some("admin-password".into());

        let connection = management_profile(&profile);
        assert_eq!(connection.username.as_deref(), Some("admin"));
        assert_eq!(connection.password.as_deref(), Some("admin-password"));
    }

    #[test]
    fn local_static_driver_round_trips_a_configured_file() -> std::result::Result<(), String> {
        let path = std::env::temp_dir().join(format!(
            "ramag-mosquitto-{}-{}.conf",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos()
        ));
        std::fs::write(&path, "old-content\n").map_err(|error| error.to_string())?;

        let mut profile = MqttProfile::new("local", "127.0.0.1", 1883);
        profile.management.enabled = true;
        profile.management.static_config = Some(ramag_domain::entities::MosquittoStaticConfig {
            target: ramag_domain::entities::MosquittoConfigTarget::Local,
            password_file: Some(path.to_string_lossy().into_owned()),
            acl_file: None,
        });
        let file = ramag_domain::entities::MosquittoStaticFile {
            kind: MosquittoStaticFileKind::Password,
            path: path.to_string_lossy().into_owned(),
            content: "new-content\n".into(),
        };
        let driver = LocalMosquittoStaticConfigDriver::new();
        let loaded = smol::block_on(async {
            driver
                .write_file(&profile, &file)
                .await
                .map_err(|error| error.user_message())?;
            driver
                .read_file(&profile, MosquittoStaticFileKind::Password)
                .await
                .map_err(|error| error.user_message())
        })?;
        assert_eq!(loaded.content, "new-content\n");
        let _ = std::fs::remove_file(path);
        Ok(())
    }
}
