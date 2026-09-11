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

fn prepare_config(config: &KafkaClusterConfig, operation: &'static str) -> Result<()> {
    config.validate().map_err(DomainError::InvalidConfig)?;
    if config.uses_tls() {
        return Err(DomainError::Kafka(KafkaError::new(
            KafkaErrorCategory::Tls,
            operation,
            "纯 Rust Kafka 路径当前未启用 TLS；请使用 native TLS 路径",
        )));
    }
    if let Some(mechanism) = config.sasl_mechanism {
        match mechanism {
            ramag_domain::entities::KafkaSaslMechanism::Plain
            | ramag_domain::entities::KafkaSaslMechanism::ScramSha256
            | ramag_domain::entities::KafkaSaslMechanism::ScramSha512 => {}
            ramag_domain::entities::KafkaSaslMechanism::Gssapi
            | ramag_domain::entities::KafkaSaslMechanism::OAuthBearer => {
                return Err(DomainError::Kafka(KafkaError::new(
                    KafkaErrorCategory::Unsupported,
                    operation,
                    "纯 Rust Kafka 路径当前只支持 PLAIN 和 SCRAM 认证",
                )));
            }
        }
    }
    Ok(())
}

fn unsupported(operation: &'static str, message: &'static str) -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::Unsupported,
        operation,
        message,
    ))
}

fn cancelled(operation: &'static str) -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::Cancelled,
        operation,
        "纯 Rust Kafka 读取任务已取消",
    ))
}

fn ensure_not_cancelled(cancel_flag: &AtomicBool, operation: &'static str) -> Result<()> {
    if cancel_flag.load(Ordering::Acquire) {
        return Err(cancelled(operation));
    }
    Ok(())
}

/// 在独立 Tokio 线程中运行 `rskafka`，避免把 Tokio executor 混入 GPUI/smol 调度器。
async fn run_on_tokio<T, F>(
    timeout: Duration,
    operation: &'static str,
    cancel_flag: Arc<AtomicBool>,
    future: F,
) -> Result<T>
where
    T: Send + 'static,
    F: Future<Output = Result<T>> + Send + 'static,
{
    smol::unblock(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| {
                DomainError::Kafka(KafkaError::new(
                    KafkaErrorCategory::Network,
                    operation,
                    "无法创建纯 Rust Kafka 运行时",
                ))
            })?;
        match runtime.block_on(async move {
            tokio::time::timeout(timeout, async move {
                tokio::select! {
                    result = future => result,
                    _ = wait_for_cancellation(cancel_flag) => Err(cancelled(operation)),
                }
            })
            .await
        }) {
            Ok(result) => result,
            Err(_) => Err(DomainError::Kafka(
                KafkaError::new(
                    KafkaErrorCategory::Timeout,
                    operation,
                    "纯 Rust Kafka 请求超时",
                )
                .retryable(true),
            )),
        }
    })
    .await
}

async fn wait_for_cancellation(cancel_flag: Arc<AtomicBool>) {
    while !cancel_flag.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn build_client(config: &KafkaClusterConfig, operation: &'static str) -> Result<Client> {
    let client_id = Arc::<str>::from(
        config
            .client_id
            .clone()
            .unwrap_or_else(|| "ramag-kafka".into()),
    );
    let mut builder = ClientBuilder::new(config.bootstrap_servers.clone())
        .client_id(client_id)
        .max_message_size(PURE_RUST_MAX_MESSAGE_SIZE);

    if config.uses_sasl() {
        let credentials = Credentials::new(
            config.sasl_username.clone().unwrap_or_default(),
            config.sasl_password.clone().unwrap_or_default(),
        );
        builder = match config.sasl_mechanism {
            Some(ramag_domain::entities::KafkaSaslMechanism::Plain) => {
                builder.sasl_config(SaslConfig::Plain(credentials))
            }
            Some(ramag_domain::entities::KafkaSaslMechanism::ScramSha256) => {
                builder.sasl_config(SaslConfig::ScramSha256(credentials))
            }
            Some(ramag_domain::entities::KafkaSaslMechanism::ScramSha512) => {
                builder.sasl_config(SaslConfig::ScramSha512(credentials))
            }
            Some(ramag_domain::entities::KafkaSaslMechanism::Gssapi)
            | Some(ramag_domain::entities::KafkaSaslMechanism::OAuthBearer)
            | None => {
                return Err(unsupported(
                    operation,
                    "纯 Rust Kafka 路径当前不支持所选 SASL 机制",
                ));
            }
        };
    }

    builder
        .build()
        .await
        .map_err(|error| map_error(error, operation))
}

fn map_error(error: RskafkaError, operation: &'static str) -> DomainError {
    let category = match &error {
        RskafkaError::Connection(error) => match error {
            rskafka::ConnectionError::SaslFailed(_) => KafkaErrorCategory::Authentication,
            _ => KafkaErrorCategory::Network,
        },
        RskafkaError::Request(error) => match error {
            RequestError::IO(_) | RequestError::Poisoned(_) => KafkaErrorCategory::Network,
            _ => KafkaErrorCategory::Protocol,
        },
        RskafkaError::ServerError { protocol_error, .. } => {
            protocol_error_category(*protocol_error)
        }
        RskafkaError::InvalidResponse(_) => KafkaErrorCategory::Protocol,
        RskafkaError::RetryFailed(_) => KafkaErrorCategory::Network,
        RskafkaError::Timeout => KafkaErrorCategory::Timeout,
        _ => KafkaErrorCategory::Unknown,
    };
    DomainError::Kafka(
        KafkaError::new(
            category,
            operation,
            format!("纯 Rust Kafka 操作失败：{operation}"),
        )
        .retryable(matches!(
            category,
            KafkaErrorCategory::Network | KafkaErrorCategory::Timeout
        )),
    )
}

fn protocol_error_category(error: ProtocolError) -> KafkaErrorCategory {
    match error {
        ProtocolError::SaslAuthenticationFailed
        | ProtocolError::IllegalSaslState
        | ProtocolError::UnacceptableCredential => KafkaErrorCategory::Authentication,
        ProtocolError::TopicAuthorizationFailed
        | ProtocolError::GroupAuthorizationFailed
        | ProtocolError::ClusterAuthorizationFailed
        | ProtocolError::TransactionalIdAuthorizationFailed
        | ProtocolError::DelegationTokenAuthorizationFailed => KafkaErrorCategory::PermissionDenied,
        ProtocolError::UnknownTopicOrPartition
        | ProtocolError::InvalidTopicException
        | ProtocolError::GroupIdNotFound
        | ProtocolError::ResourceNotFound => KafkaErrorCategory::NotFound,
        ProtocolError::UnsupportedSaslMechanism
        | ProtocolError::UnsupportedVersion
        | ProtocolError::UnsupportedForMessageFormat
        | ProtocolError::SecurityDisabled => KafkaErrorCategory::Unsupported,
        ProtocolError::RequestTimedOut | ProtocolError::ThrottlingQuotaExceeded => {
            KafkaErrorCategory::Timeout
        }
        ProtocolError::LeaderNotAvailable
        | ProtocolError::NotLeaderOrFollower
        | ProtocolError::BrokerNotAvailable
        | ProtocolError::ReplicaNotAvailable
        | ProtocolError::NetworkException
        | ProtocolError::CoordinatorLoadInProgress
        | ProtocolError::CoordinatorNotAvailable
        | ProtocolError::NotCoordinator
        | ProtocolError::PreferredLeaderNotAvailable => KafkaErrorCategory::Network,
        ProtocolError::InvalidFetchSize
        | ProtocolError::InvalidPartitions
        | ProtocolError::InvalidReplicationFactor
        | ProtocolError::InvalidConfig
        | ProtocolError::InvalidRequest
        | ProtocolError::InvalidRequiredAcks => KafkaErrorCategory::InvalidConfig,
        _ => KafkaErrorCategory::Protocol,
    }
}

async fn test_connection_async(
    config: KafkaClusterConfig,
    cancelled: Arc<AtomicBool>,
) -> Result<()> {
    ensure_not_cancelled(&cancelled, "测试纯 Rust Kafka 连接")?;
    let _client = build_client(&config, "测试纯 Rust Kafka 连接").await?;
    ensure_not_cancelled(&cancelled, "测试纯 Rust Kafka 连接")?;
    Ok(())
}

async fn list_topics_async(
    config: KafkaClusterConfig,
    cancelled: Arc<AtomicBool>,
) -> Result<Vec<KafkaTopic>> {
    ensure_not_cancelled(&cancelled, "读取纯 Rust Kafka Topic")?;
    let client = build_client(&config, "读取纯 Rust Kafka Topic").await?;
    ensure_not_cancelled(&cancelled, "读取纯 Rust Kafka Topic")?;
    let topics = client
        .list_topics()
        .await
        .map_err(|error| map_error(error, "读取纯 Rust Kafka Topic"))?;
    if topics.len() > MAX_KAFKA_TOPICS {
        return Err(DomainError::InvalidConfig(format!(
            "Kafka Topic 数量超过 {MAX_KAFKA_TOPICS} 个上限"
        )));
    }

    let mut total_partitions = 0usize;
    let mut result = Vec::with_capacity(topics.len());
    for topic in topics {
        ensure_not_cancelled(&cancelled, "读取纯 Rust Kafka Topic")?;
        total_partitions = total_partitions
            .checked_add(topic.partitions.len())
            .ok_or_else(|| DomainError::InvalidConfig("Kafka Partition 总数量溢出".into()))?;
        if total_partitions > MAX_KAFKA_PARTITIONS {
            return Err(DomainError::InvalidConfig(format!(
                "Kafka Partition 总数量超过 {MAX_KAFKA_PARTITIONS} 个上限"
            )));
        }
        let partitions = topic
            .partitions
            .into_iter()
            .map(|id| KafkaPartition {
                id,
                leader: None,
                replicas: Vec::new(),
                isr: Vec::new(),
                low_watermark: None,
                high_watermark: None,
            })
            .collect();
        let internal = topic.name.starts_with("__");
        let topic = KafkaTopic {
            name: topic.name,
            partitions,
            internal,
        };
        topic.validate().map_err(DomainError::InvalidConfig)?;
        result.push(topic);
    }
    Ok(result)
}

#[derive(Default)]
struct PartitionScan {
    records: Vec<KafkaMessageRecord>,
    scanned_records: usize,
    scanned_bytes: u64,
    truncated: bool,
}

async fn scan_messages_async(
    config: KafkaClusterConfig,
    query: KafkaMessageQuery,
    search: Option<KafkaMessageSearchQuery>,
    request_timeout: Duration,
    cancelled: Arc<AtomicBool>,
) -> Result<KafkaMessagePage> {
    ensure_not_cancelled(&cancelled, "读取纯 Rust Kafka 消息")?;
    let matcher = search.as_ref().map(MessageSearchMatcher::new).transpose()?;
    let client = build_client(&config, "创建纯 Rust Kafka 读取客户端").await?;
    let deadline = Instant::now() + Duration::from_secs(u64::from(query.max_scan_seconds));
    let mut page = KafkaMessagePage::empty();

    for &partition in &query.partitions {
        ensure_not_cancelled(&cancelled, "读取纯 Rust Kafka 消息")?;
        if page.scanned_records >= query.max_records
            || page.scanned_bytes >= query.max_bytes
            || Instant::now() >= deadline
        {
            page.truncated = true;
            break;
        }
        let remaining_records = query.max_records - page.scanned_records;
        let remaining_bytes = query.max_bytes - page.scanned_bytes;
        let scanned = scan_partition(PartitionScanRequest {
            client: &client,
            query: &query,
            partition,
            max_records: remaining_records,
            max_bytes: remaining_bytes,
            deadline,
            request_timeout,
            search: matcher.as_ref(),
            cancelled: &cancelled,
        })
        .await?;
        page.scanned_records = page.scanned_records.saturating_add(scanned.scanned_records);
        page.scanned_bytes = page.scanned_bytes.saturating_add(scanned.scanned_bytes);
        page.records.extend(scanned.records);
        if scanned.truncated {
            page.truncated = true;
            break;
        }
    }
    if Instant::now() >= deadline {
        page.truncated = true;
    }
    ensure_not_cancelled(&cancelled, "读取纯 Rust Kafka 消息")?;
    page.validate().map_err(DomainError::InvalidConfig)?;
    Ok(page)
}

struct PartitionScanRequest<'a> {
    client: &'a Client,
    query: &'a KafkaMessageQuery,
    partition: i32,
    max_records: usize,
    max_bytes: u64,
    deadline: Instant,
    request_timeout: Duration,
    search: Option<&'a MessageSearchMatcher>,
    cancelled: &'a AtomicBool,
}

async fn scan_partition(request: PartitionScanRequest<'_>) -> Result<PartitionScan> {
    let PartitionScanRequest {
        client,
        query,
        partition,
        max_records,
        max_bytes,
        deadline,
        request_timeout,
        search,
        cancelled,
    } = request;
    ensure_not_cancelled(cancelled, "读取纯 Rust Kafka Partition")?;
    let partition_client = client
        .partition_client(query.topic.clone(), partition, UnknownTopicHandling::Retry)
        .await
        .map_err(|error| map_error(error, "创建纯 Rust Kafka Partition 客户端"))?;
    let low = partition_client
        .get_offset(OffsetAt::Earliest)
        .await
        .map_err(|error| map_error(error, "读取纯 Rust Kafka Partition 起始 Offset"))?
        .max(0);
    let high = partition_client
        .get_offset(OffsetAt::Latest)
        .await
        .map_err(|error| map_error(error, "读取纯 Rust Kafka Partition 末尾 Offset"))?
        .max(low);
    let start = resolve_offset(
        &partition_client,
        query.start_offset,
        query.start_time,
        low,
        low,
        high,
        "解析纯 Rust Kafka 消息起始 Offset",
    )
    .await?;
    let end = resolve_offset(
        &partition_client,
        query.end_offset,
        query.end_time,
        high,
        low,
        high,
        "解析纯 Rust Kafka 消息结束 Offset",
    )
    .await?;
    if start >= end {
        return Ok(PartitionScan::default());
    }

    let timeout_ms = i32::try_from(request_timeout.as_millis().min(i32::MAX as u128))
        .unwrap_or(i32::MAX)
        .max(1);
    let mut next_offset = start;
    let mut scanned = PartitionScan::default();
    while next_offset < end && Instant::now() < deadline {
        ensure_not_cancelled(cancelled, "读取纯 Rust Kafka 消息")?;
        if scanned.scanned_records >= max_records || scanned.scanned_bytes >= max_bytes {
            scanned.truncated = true;
            break;
        }
        let remaining_bytes = max_bytes.saturating_sub(scanned.scanned_bytes);
        let fetch_bytes = remaining_bytes
            .clamp(1, PURE_RUST_MAX_FETCH_BYTES)
            .min(i32::MAX as u64) as i32;
        let (records, _) = partition_client
            .fetch_records(next_offset, 0..fetch_bytes, timeout_ms)
            .await
            .map_err(|error| map_error(error, "读取纯 Rust Kafka 消息"))?;
        if records.is_empty() {
            break;
        }

        let mut advanced = false;
        for record in records {
            ensure_not_cancelled(cancelled, "读取纯 Rust Kafka 消息")?;
            next_offset = next_offset.max(record.offset.saturating_add(1));
            advanced = true;
            if record.offset < start {
                continue;
            }
            if record.offset >= end {
                break;
            }
            let record = record_from_rskafka(&query.topic, partition, record);
            let record_bytes = record.retained_bytes();
            if scanned.scanned_bytes.saturating_add(record_bytes) > max_bytes {
                scanned.truncated = true;
                break;
            }
            scanned.scanned_records = scanned.scanned_records.saturating_add(1);
            scanned.scanned_bytes = scanned.scanned_bytes.saturating_add(record_bytes);
            if search.is_none_or(|matcher| message_matches(&record, matcher)) {
                scanned.records.push(record);
            }
            if scanned.scanned_records >= max_records {
                scanned.truncated = true;
                break;
            }
        }
        if scanned.truncated || !advanced {
            break;
        }
    }
    if Instant::now() >= deadline {
        scanned.truncated = true;
    }
    Ok(scanned)
}

async fn resolve_offset(
    partition_client: &PartitionClient,
    offset: Option<i64>,
    timestamp: Option<DateTime<Utc>>,
    fallback: i64,
    low: i64,
    high: i64,
    operation: &'static str,
) -> Result<i64> {
    let value = match (offset, timestamp) {
        (Some(offset), None) => offset,
        (None, Some(timestamp)) => partition_client
            .get_offset(OffsetAt::Timestamp(timestamp))
            .await
            .map_err(|error| map_error(error, operation))?,
        (None, None) => fallback,
        (Some(_), Some(_)) => {
            return Err(DomainError::InvalidConfig(
                "Kafka 消息查询不能同时使用 Offset 和时间范围".into(),
            ));
        }
    };
    // Kafka returns -1 when a timestamp has no matching offset. Treat an
    // unmatched lower bound as the retained beginning and an unmatched upper
    // bound as the current end, matching the offset query semantics above.
    Ok(if value < 0 {
        fallback
    } else {
        value.clamp(low, high)
    })
}

fn record_from_rskafka(topic: &str, partition: i32, record: RecordAndOffset) -> KafkaMessageRecord {
    let record_data = record.record;
    let headers = record_data
        .headers
        .into_iter()
        .map(|(key, value)| KafkaMessageHeader {
            key,
            value: Some(value),
        })
        .collect();
    KafkaMessageRecord {
        topic: topic.to_owned(),
        partition,
        offset: record.offset,
        timestamp: Some(record_data.timestamp),
        key: record_data.key,
        value: record_data.value,
        headers,
    }
}

struct MessageSearchMatcher {
    query: String,
    fields: Vec<KafkaMessageSearchField>,
    regex: Option<regex::Regex>,
}

impl MessageSearchMatcher {
    fn new(query: &KafkaMessageSearchQuery) -> Result<Self> {
        query.validate().map_err(DomainError::InvalidConfig)?;
        let regex = (query.mode == KafkaMessageSearchMode::Regex)
            .then(|| {
                regex::RegexBuilder::new(&query.query)
                    .case_insensitive(true)
                    .size_limit(1024 * 1024)
                    .dfa_size_limit(1024 * 1024)
                    .build()
            })
            .transpose()
            .map_err(|error| DomainError::InvalidConfig(format!("消息正则表达式无效：{error}")))?;
        Ok(Self {
            query: query.query.to_lowercase(),
            fields: query.fields.clone(),
            regex,
        })
    }
}

fn message_matches(record: &KafkaMessageRecord, matcher: &MessageSearchMatcher) -> bool {
    matcher.fields.iter().any(|field| match field {
        KafkaMessageSearchField::Key => record
            .key
            .as_deref()
            .is_some_and(|bytes| text_matches(bytes, matcher)),
        KafkaMessageSearchField::Value => record
            .value
            .as_deref()
            .is_some_and(|bytes| text_matches(bytes, matcher)),
        KafkaMessageSearchField::Headers => record.headers.iter().any(|header| {
            text_matches(header.key.as_bytes(), matcher)
                || header
                    .value
                    .as_deref()
                    .is_some_and(|bytes| text_matches(bytes, matcher))
        }),
    })
}

fn text_matches(bytes: &[u8], matcher: &MessageSearchMatcher) -> bool {
    let text = String::from_utf8_lossy(bytes);
    matcher.regex.as_ref().map_or_else(
        || text.to_lowercase().contains(&matcher.query),
        |regex| regex.is_match(&text),
    )
}

#[async_trait::async_trait]
impl KafkaDriver for PureRustTransport {
    fn transport_capabilities(&self) -> KafkaTransportCapabilities {
        self.capabilities()
    }

    async fn test_connection(&self, config: &KafkaClusterConfig) -> Result<()> {
        self.test_connection_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn test_connection_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        prepare_config(config, "测试纯 Rust Kafka 连接")?;
        ensure_not_cancelled(&cancelled, "测试纯 Rust Kafka 连接")?;
        run_on_tokio(
            self.request_timeout,
            "测试纯 Rust Kafka 连接",
            cancelled.clone(),
            test_connection_async(config.clone(), cancelled),
        )
        .await
    }

    async fn cluster_metadata(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<ramag_domain::entities::KafkaClusterMetadata> {
        self.cluster_metadata_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn cluster_metadata_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<ramag_domain::entities::KafkaClusterMetadata> {
        prepare_config(config, "读取纯 Rust Kafka 集群元数据")?;
        ensure_not_cancelled(&cancelled, "读取纯 Rust Kafka 集群元数据")?;
        Err(unsupported(
            "读取纯 Rust Kafka 集群元数据",
            "当前纯 Rust Kafka 客户端未暴露完整 Broker/Controller 元数据",
        ))
    }

    async fn list_topics(&self, config: &KafkaClusterConfig) -> Result<Vec<KafkaTopic>> {
        self.list_topics_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn list_topics_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<KafkaTopic>> {
        prepare_config(config, "读取纯 Rust Kafka Topic")?;
        ensure_not_cancelled(&cancelled, "读取纯 Rust Kafka Topic")?;
        run_on_tokio(
            self.request_timeout,
            "读取纯 Rust Kafka Topic",
            cancelled.clone(),
            list_topics_async(config.clone(), cancelled),
        )
        .await
    }

    async fn read_messages(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaMessageQuery,
    ) -> Result<KafkaMessagePage> {
        self.read_messages_with_cancel(config, query, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn read_messages_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaMessageQuery,
        cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaMessagePage> {
        prepare_config(config, "读取纯 Rust Kafka 消息")?;
        query.validate().map_err(DomainError::InvalidConfig)?;
        ensure_not_cancelled(&cancelled, "读取纯 Rust Kafka 消息")?;
        run_on_tokio(
            self.request_timeout,
            "读取纯 Rust Kafka 消息",
            cancelled.clone(),
            scan_messages_async(
                config.clone(),
                query.clone(),
                None,
                self.request_timeout,
                cancelled,
            ),
        )
        .await
    }

    async fn search_messages(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaMessageSearchQuery,
    ) -> Result<KafkaMessagePage> {
        self.search_messages_with_cancel(config, query, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn search_messages_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaMessageSearchQuery,
        cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaMessagePage> {
        prepare_config(config, "搜索纯 Rust Kafka 消息")?;
        query.validate().map_err(DomainError::InvalidConfig)?;
        ensure_not_cancelled(&cancelled, "搜索纯 Rust Kafka 消息")?;
        run_on_tokio(
            self.request_timeout,
            "搜索纯 Rust Kafka 消息",
            cancelled.clone(),
            scan_messages_async(
                config.clone(),
                query.scan.clone(),
                Some(query.clone()),
                self.request_timeout,
                cancelled,
            ),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::{KafkaSaslMechanism, KafkaSecurityProtocol};

    #[test]
    fn capabilities_only_advertise_the_pure_rust_reading_slice() {
        let capabilities = PureRustTransport::new().capabilities();
        assert_eq!(capabilities.backend, KafkaTransportBackend::PureRust);
        assert!(capabilities.build_available);
        assert!(capabilities.fetch);
        assert!(capabilities.list_offsets);
        assert!(!capabilities.metadata);
        assert!(!capabilities.consumer_groups);
        assert!(!capabilities.topic_admin);
        assert!(!capabilities.tls);
        assert!(capabilities.sasl);
    }

    #[test]
    fn tls_is_rejected_before_network_access() {
        let mut config = KafkaClusterConfig::new("secure", vec!["broker:9093".into()]);
        config.security_protocol = KafkaSecurityProtocol::Ssl;
        let result = smol::block_on(PureRustTransport::new().test_connection(&config));
        assert!(matches!(
            result,
            Err(DomainError::Kafka(error)) if error.category == KafkaErrorCategory::Tls
        ));
    }

    #[test]
    fn unsupported_sasl_mechanisms_are_rejected_without_exposing_credentials() {
        let mut config = KafkaClusterConfig::new("secure", vec!["broker:9092".into()]);
        config.security_protocol = KafkaSecurityProtocol::SaslPlaintext;
        config.sasl_mechanism = Some(KafkaSaslMechanism::Gssapi);
        config.sasl_username = Some("user".into());
        config.sasl_password = Some("password".into());
        let result = smol::block_on(PureRustTransport::new().test_connection(&config));
        assert!(matches!(
            result,
            Err(DomainError::Kafka(error))
                if error.category == KafkaErrorCategory::Unsupported
                    && !error.safe_message.contains("password")
        ));
    }

    #[test]
    fn cancelled_requests_stop_before_creating_the_client() {
        let config = KafkaClusterConfig::new("local", vec!["broker:9092".into()]);
        let cancelled = Arc::new(AtomicBool::new(true));
        let result = smol::block_on(
            PureRustTransport::new().test_connection_with_cancel(&config, cancelled),
        );
        assert!(matches!(
            result,
            Err(DomainError::Kafka(error)) if error.category == KafkaErrorCategory::Cancelled
        ));
    }
}
