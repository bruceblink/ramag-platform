//! MQTT 连接配置、协议边界和 Mosquitto 管理对象。
//!
//! MQTT 的连接数据面、Mosquitto Dynamic Security 管理面和静态文件管理面
//! 使用同一份配置保存，但能力由后续适配器单独声明，不把 Broker 管理接口
//! 混入通用 MQTT 协议。

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{TlsVerify, ssh::SshProfileId};

pub const DEFAULT_MQTT_PORT: u16 = 1883;
pub const DEFAULT_MQTT_TLS_PORT: u16 = 8883;
pub const DEFAULT_MQTT_KEEP_ALIVE_SECONDS: u16 = 60;
pub const MAX_MQTT_PROFILES: usize = 512;
pub const MAX_MQTT_PROFILE_NAME_BYTES: usize = 256;
pub const MAX_MQTT_HOST_BYTES: usize = 1024;
pub const MAX_MQTT_CLIENT_ID_BYTES: usize = 256;
pub const MAX_MQTT_USERNAME_BYTES: usize = 1024;
pub const MAX_MQTT_PASSWORD_BYTES: usize = 64 * 1024;
pub const MAX_MQTT_REMARK_BYTES: usize = 16 * 1024;
pub const MAX_MQTT_TLS_PATH_BYTES: usize = 32 * 1024;
pub const MAX_MQTT_TOPIC_BYTES: usize = u16::MAX as usize;
pub const MAX_MOSQUITTO_NAME_BYTES: usize = 256;
pub const MAX_MOSQUITTO_DESCRIPTION_BYTES: usize = 16 * 1024;
pub const MAX_MOSQUITTO_ACLS: usize = 4096;
pub const MAX_MQTT_PROFILE_RECORD_BYTES: usize = 1024 * 1024;
pub const MAX_MQTT_PROFILE_LIST_BYTES: usize = 64 * 1024 * 1024;
include!("mqtt/part01.rs");
include!("mqtt/part02.rs");
include!("mqtt/part03.rs");
include!("mqtt/part04.rs");

#[cfg(test)]
mod tests {
    include!("mqtt/tests.rs");
}
