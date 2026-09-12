//! `rskafka` 纯 Rust 读取适配器。
//!
//! 该适配器是显式 feature 下的能力验证路径，不改变当前 native 默认实现。
//! 它只提供不提交业务 Offset 的 Topic 列表、Offset/时间范围读取和客户端搜索；
//! 集群 Broker 详情、消费者组、实时 Tail、动态配置和 ACL 仍由能力快照明确标记为不可用。

use chrono::{DateTime, Utc};
use ramag_domain::entities::{
    KafkaClusterConfig, KafkaMessageHeader, KafkaMessagePage, KafkaMessageQuery,
    KafkaMessageRecord, KafkaMessageSearchField, KafkaMessageSearchMode, KafkaMessageSearchQuery,
    KafkaPartition, KafkaTopic, KafkaTransportBackend, KafkaTransportCapabilities,
    MAX_KAFKA_PARTITIONS, MAX_KAFKA_TOPICS,
};
use ramag_domain::error::{DomainError, KafkaError, KafkaErrorCategory, Result};
use ramag_domain::traits::{KafkaDriver, KafkaTransport};
use rskafka::client::error::{Error as RskafkaError, ProtocolError, RequestError};
use rskafka::client::partition::{OffsetAt, PartitionClient, UnknownTopicHandling};
use rskafka::client::{Client, ClientBuilder, Credentials, SaslConfig};
use rskafka::record::RecordAndOffset;
use std::future::Future;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

const PURE_RUST_MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;
const PURE_RUST_MAX_FETCH_BYTES: u64 = 8 * 1024 * 1024;

/// 不链接 `librdkafka` 的 Kafka 读取适配器。
#[derive(Debug, Clone, Copy)]
pub struct PureRustTransport {
    request_timeout: Duration,
}

/// 纯 Rust 适配器的兼容名称，便于集成测试和后续组合根选择实现。
pub type PureRustDriver = PureRustTransport;

impl PureRustTransport {
    /// 创建使用固定默认请求预算的纯 Rust 驱动。
    pub fn new() -> Self {
        Self {
            request_timeout: super::DEFAULT_KAFKA_REQUEST_TIMEOUT,
        }
    }

    /// 创建可指定请求预算的驱动，所有底层请求共用同一个上限。
    pub fn with_request_timeout(request_timeout: Duration) -> Result<Self> {
        super::validate_request_timeout(request_timeout)?;
        Ok(Self { request_timeout })
    }

    fn capabilities() -> KafkaTransportCapabilities {
        KafkaTransportCapabilities {
            backend: KafkaTransportBackend::PureRust,
            build_available: true,
            // rskafka 可以列 Topic，但当前公开 API 不返回完整 Broker/Controller 快照。
            metadata: false,
            fetch: true,
            list_offsets: true,
            consumer_groups: false,
            topic_admin: false,
            config_admin: false,
            acl_admin: false,
            metrics_snapshot: false,
            tls: false,
            sasl: true,
        }
    }
}

impl Default for PureRustTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl KafkaTransport for PureRustTransport {
    fn capabilities(&self) -> KafkaTransportCapabilities {
        Self::capabilities()
    }
}

include!("pure_rust/helpers.rs");
include!("pure_rust/driver.rs");

#[cfg(test)]
mod tests {
    include!("pure_rust/tests.rs");
}
