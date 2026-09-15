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
            .block_on(
                async move { tokio::time::timeout(Duration::from_secs(15), operation()).await },
            )
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
    include!("native/driver_operations.rs");
    include!("native/dynamic_security_protocol.rs");
    include!("native/mqtt_data_plane.rs");
    include!("native/transport_support.rs");

    #[cfg(test)]
    mod tests {
        include!("native/tests.rs");
    }
}

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
