//! Kafka 工作区 UI。
//!
//! 页面只通过 `KafkaService` 读取本地保存的配置和真实 Broker 数据，不在视图层生成
//! 集群、Topic 或消息样例。所有消息读取都要求用户给出 Topic、Partition、Offset 和
//! 有界预算，避免误把浏览操作变成无界消费。

use std::{
    collections::VecDeque,
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::{DateTime, Utc};
use gpui::{
    App, AppContext as _, ClickEvent, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, UniformListScrollHandle, Window, div,
    prelude::FluentBuilder as _, px, uniform_list,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    button::ButtonVariants as _,
    h_flex,
    input::{Input, InputEvent, InputState},
    scroll::{Scrollbar, ScrollbarShow},
    v_flex,
};
use ramag_app::KafkaService;
use ramag_domain::{
    entities::{
        DEFAULT_KAFKA_MAX_BYTES, DEFAULT_KAFKA_MAX_CONCURRENT_PARTITIONS,
        DEFAULT_KAFKA_MAX_SCAN_SECONDS, DEFAULT_KAFKA_METRICS_REFRESH_SECONDS,
        DEFAULT_KAFKA_TAIL_WINDOW_BYTES, DEFAULT_KAFKA_TAIL_WINDOW_MESSAGES, KafkaAcl,
        KafkaAclOperation, KafkaAclPatternType, KafkaAclPermission, KafkaAclResourceType,
        KafkaBrokerMetricsSnapshot, KafkaClusterConfig, KafkaClusterId, KafkaClusterMetadata,
        KafkaConfigEntry, KafkaConfigResourceType, KafkaConfigUpdateOperation,
        KafkaConfigUpdateRequest, KafkaConnectConnector, KafkaConsumerGroup,
        KafkaConsumerGroupOffsetReset, KafkaConsumerGroupOffsetResetRequest, KafkaKsqlDbQuery,
        KafkaKsqlDbQueryResult, KafkaMessagePage, KafkaMessageProduceRequest, KafkaMessageQuery,
        KafkaMessageRecord, KafkaMessageSearchField, KafkaMessageSearchQuery,
        KafkaMessageTailEvent, KafkaMessageTailRequest, KafkaMessageTailStart,
        KafkaMetricsSnapshot, KafkaMetricsSnapshotState, KafkaPartitionMetrics, KafkaReadOnlyState,
        KafkaSaslMechanism, KafkaSchemaRegistrySubject, KafkaSecurityProtocol, KafkaTlsConfig,
        KafkaTopic, KafkaTopicCreateRequest, KafkaTopicPartitionExpansion,
        MAX_KAFKA_ACL_HOST_BYTES, MAX_KAFKA_ACL_RESOURCE_NAME_BYTES,
        MAX_KAFKA_CONFIG_RESOURCE_NAME_BYTES, MAX_KAFKA_CONFIG_VALUE_BYTES,
        MAX_KAFKA_CONNECT_ENDPOINT_BYTES, MAX_KAFKA_CONNECT_PASSWORD_BYTES,
        MAX_KAFKA_CONNECT_USERNAME_BYTES, MAX_KAFKA_KSQLDB_ENDPOINT_BYTES,
        MAX_KAFKA_KSQLDB_QUERY_BYTES, MAX_KAFKA_METRICS_REFRESH_SECONDS, MAX_KAFKA_PARTITIONS,
        MAX_KAFKA_PRODUCE_MESSAGE_BYTES, MAX_KAFKA_QUERY_PARTITIONS, MAX_KAFKA_REPLICAS,
        MAX_KAFKA_SCAN_RECORDS, MAX_KAFKA_SCHEMA_REGISTRY_ENDPOINT_BYTES,
        MAX_KAFKA_SCHEMA_REGISTRY_PASSWORD_BYTES, MAX_KAFKA_SCHEMA_REGISTRY_USERNAME_BYTES,
        MIN_KAFKA_METRICS_REFRESH_SECONDS,
    },
    traits::{KafkaMessageTailSink, KafkaMessageTailSinkResult, Tool, ToolMeta},
};
use serde::Serialize;

const MAX_VISIBLE_PARTITIONS: usize = 200;
const MESSAGE_PREVIEW_BYTES: usize = 512;
const MAX_KAFKA_EXPORT_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_MESSAGE_PAGE_SIZE: usize = 100;
const MESSAGE_TABLE_MIN_WIDTH: f32 = 720.0;
const COMPACT_MESSAGE_RESULTS_HEIGHT: f32 = 480.0;
const DEFAULT_TOPIC_PAGE_SIZE: usize = 50;
const MESSAGE_TAIL_CHANNEL_CAPACITY: usize = 8;
const MESSAGE_TAIL_RESULTS_HEIGHT: f32 = 260.0;
const KAFKA_SIDEBAR_WIDTH: f32 = 260.0;
const KAFKA_TOPIC_SCROLLBAR_WIDTH: f32 = 16.0;
const KAFKA_SCHEMA_SUBJECT_SCROLLBAR_WIDTH: f32 = 16.0;
const MAX_VISIBLE_GROUP_OFFSETS: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KafkaTailStartMode {
    Latest,
    Earliest,
    Offset,
}

impl KafkaTailStartMode {
    const ALL: [Self; 3] = [Self::Latest, Self::Earliest, Self::Offset];

    const fn label(self) -> &'static str {
        match self {
            Self::Latest => "Latest",
            Self::Earliest => "Earliest",
            Self::Offset => "Offset",
        }
    }
}

/// 创建 Kafka 工具的主视图，窗口生命周期由主壳持有。
pub fn create_kafka_view(
    service: Arc<KafkaService>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<KafkaView> {
    cx.new(|cx| KafkaView::new(service, window, cx))
}

/// Kafka 工具在 Activity Bar 中显示的注册信息。
pub struct KafkaTool {
    meta: ToolMeta,
}

impl KafkaTool {
    pub const ID: &'static str = "kafka";

    /// 创建稳定的工具元数据；连接配置和运行时数据由 Kafka 工作区按需加载。
    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(Self::ID, "Kafka", "浏览集群、Topic、Partition 与消息")
                .with_icon("server"),
        }
    }
}

impl Default for KafkaTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for KafkaTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KafkaSection {
    Overview,
    Topics,
    Messages,
    ConsumerGroups,
    SchemaRegistry,
    Connect,
    KsqlDb,
    Acls,
    Config,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KafkaRangeMode {
    Offset,
    Time,
}

impl KafkaRangeMode {
    const fn label(self) -> &'static str {
        match self {
            Self::Offset => "Offset",
            Self::Time => "时间",
        }
    }
}

impl KafkaSection {
    const ALL: [Self; 9] = [
        Self::Overview,
        Self::Topics,
        Self::Messages,
        Self::ConsumerGroups,
        Self::SchemaRegistry,
        Self::Connect,
        Self::KsqlDb,
        Self::Acls,
        Self::Config,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Overview => "概览",
            Self::Topics => "Topics",
            Self::Messages => "消息",
            Self::ConsumerGroups => "消费者组",
            Self::SchemaRegistry => "Schema Registry",
            Self::Connect => "Kafka Connect",
            Self::KsqlDb => "ksqlDB",
            Self::Acls => "ACL",
            Self::Config => "配置",
        }
    }
}

/// Kafka 工作区的交互状态；运行时结果全部来自 `KafkaService`，空值表示尚未成功读取。
pub struct KafkaView {
    service: Arc<KafkaService>,
    clusters: Vec<KafkaClusterConfig>,
    selected_cluster_id: Option<KafkaClusterId>,
    selected_topic: Option<String>,
    metadata: Option<KafkaClusterMetadata>,
    topics: Vec<KafkaTopic>,
    topic_page_index: usize,
    topic_page_size: usize,
    topic_scroll: UniformListScrollHandle,
    overview_scroll: ScrollHandle,
    consumer_groups: Vec<KafkaConsumerGroup>,
    selected_consumer_group: Option<String>,
    consumer_group_error: Option<String>,
    schema_subjects: Vec<KafkaSchemaRegistrySubject>,
    schema_subject_search: Entity<InputState>,
    schema_subject_scroll: UniformListScrollHandle,
    schema_registry_cancelled: Option<Arc<AtomicBool>>,
    schema_registry_request_id: u64,
    loading_schema_subjects: bool,
    schema_subjects_loaded: bool,
    schema_subject_error: Option<String>,
    connect_connectors: Vec<KafkaConnectConnector>,
    connect_search: Entity<InputState>,
    connect_scroll: UniformListScrollHandle,
    connect_cancelled: Option<Arc<AtomicBool>>,
    connect_request_id: u64,
    loading_connectors: bool,
    connectors_loaded: bool,
    connect_error: Option<String>,
    ksqldb: KafkaKsqlDbState,
    acl_cancelled: Option<Arc<AtomicBool>>,
    message_page: Option<KafkaMessagePage>,
    message_tail_records: VecDeque<KafkaMessageRecord>,
    message_tail_bytes: u64,
    message_tail_dropped_records: u64,
    message_tail_dropped_bytes: u64,
    message_tail_evicted_records: u64,
    message_tail_evicted_bytes: u64,
    message_tail_window_messages: usize,
    message_tail_window_bytes: u64,
    message_tail_scroll: UniformListScrollHandle,
    message_tail_reconnect_attempt: u32,
    message_tail_connected: bool,
    message_tail_paused: bool,
    message_tail_running: bool,
    selected_tail_message: Option<usize>,
    consumer_group_cancelled: Option<Arc<AtomicBool>>,
    metrics_snapshot: Option<KafkaMetricsSnapshot>,
    metrics_error: Option<String>,
    broker_metrics_snapshot: Option<KafkaBrokerMetricsSnapshot>,
    broker_metrics_error: Option<String>,
    metrics_loading: bool,
    metrics_refresh_seconds_input: Entity<InputState>,
    metrics_refresh_generation: u64,
    metrics_refresh_cancelled: Option<Arc<AtomicBool>>,
    selected_message: Option<usize>,
    message_page_index: usize,
    message_page_size: usize,
    message_page_scroll: ScrollHandle,
    message_scroll: UniformListScrollHandle,
    message_horizontal_scroll: ScrollHandle,
    topic_partition_scroll: ScrollHandle,
    consumer_group_scroll: UniformListScrollHandle,
    acls: Vec<KafkaAcl>,
    selected_acl: Option<KafkaAcl>,
    acls_loaded: bool,
    acl_error: Option<String>,
    acl_scroll: UniformListScrollHandle,
    section: KafkaSection,
    cluster_search: Entity<InputState>,
    topic_search: Entity<InputState>,
    consumer_group_search: Entity<InputState>,
    acl_principal_filter: Entity<InputState>,
    acl_host_filter: Entity<InputState>,
    acl_resource_name_filter: Entity<InputState>,
    message_search: Entity<InputState>,
    name: Entity<InputState>,
    bootstrap_servers: Entity<InputState>,
    client_id: Entity<InputState>,
    sasl_username: Entity<InputState>,
    sasl_password: Entity<InputState>,
    remark: Entity<InputState>,
    broker_metrics_endpoint: Entity<InputState>,
    schema_registry_endpoint: Entity<InputState>,
    schema_registry_username: Entity<InputState>,
    schema_registry_password: Entity<InputState>,
    connect_endpoint: Entity<InputState>,
    connect_username: Entity<InputState>,
    connect_password: Entity<InputState>,
    ca_cert_path: Entity<InputState>,
    client_cert_path: Entity<InputState>,
    client_key_path: Entity<InputState>,
    config_resource_name: Entity<InputState>,
    config_value: Entity<InputState>,
    topic_input: Entity<InputState>,
    produce_topic_input: Entity<InputState>,
    produce_partition_input: Entity<InputState>,
    produce_key_input: Entity<InputState>,
    produce_value_input: Entity<InputState>,
    partition_input: Entity<InputState>,
    topic_create_name: Entity<InputState>,
    topic_create_partitions: Entity<InputState>,
    topic_create_replication_factor: Entity<InputState>,
    topic_target_partitions: Entity<InputState>,
    acl_principal: Entity<InputState>,
    acl_host: Entity<InputState>,
    acl_resource_name: Entity<InputState>,
    start_offset_input: Entity<InputState>,
    end_offset_input: Entity<InputState>,
    start_time_input: Entity<InputState>,
    end_time_input: Entity<InputState>,
    max_records_input: Entity<InputState>,
    message_tail_offset_input: Entity<InputState>,
    message_tail_window_messages_input: Entity<InputState>,
    message_tail_window_bytes_input: Entity<InputState>,
    message_tail_max_message_bytes_input: Entity<InputState>,
    message_tail_poll_timeout_input: Entity<InputState>,
    search_fields: [bool; 3],
    range_mode: KafkaRangeMode,
    message_tail_start_mode: KafkaTailStartMode,
    security_protocol: KafkaSecurityProtocol,
    sasl_mechanism: KafkaSaslMechanism,
    config_resource_type: KafkaConfigResourceType,
    acl_filter_resource_type: Option<KafkaAclResourceType>,
    acl_filter_pattern_type: Option<KafkaAclPatternType>,
    acl_filter_operation: Option<KafkaAclOperation>,
    acl_filter_permission: Option<KafkaAclPermission>,
    acl_resource_type: KafkaAclResourceType,
    acl_pattern_type: KafkaAclPatternType,
    acl_operation_kind: KafkaAclOperation,
    acl_permission: KafkaAclPermission,
    config_entries: Vec<KafkaConfigEntry>,
    editing_config_key: Option<String>,
    read_only: KafkaReadOnlyState,
    loading_clusters: bool,
    cluster_load_error: Option<String>,
    loading_runtime: bool,
    runtime_error: Option<String>,
    loading_messages: bool,
    loading_consumer_groups: bool,
    loading_acls: bool,
    loading_configs: bool,
    producing: bool,
    testing: bool,
    saving: bool,
    deleting: bool,
    updating_config: bool,
    acl_operation: bool,
    exporting: bool,
    profile_operation_id: u64,
    connection_test_cancelled: Option<Arc<AtomicBool>>,
    cluster_request_id: u64,
    runtime_request_id: u64,
    runtime_cancelled: Option<Arc<AtomicBool>>,
    message_request_id: u64,
    message_read_cancelled: Option<Arc<AtomicBool>>,
    message_tail_request_id: u64,
    message_tail_cancelled: Option<Arc<AtomicBool>>,
    consumer_group_request_id: u64,
    consumer_group_operation_id: u64,
    consumer_group_operation: bool,
    config_cancelled: Option<Arc<AtomicBool>>,
    config_request_id: u64,
    acl_request_id: u64,
    topic_operation_id: u64,
    produce_operation_id: u64,
    acl_operation_id: u64,
    topic_operation: bool,
    notice: Option<(String, bool)>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Drop for KafkaView {
    fn drop(&mut self) {
        self.invalidate_runtime_request();
        self.invalidate_profile_operation();
        self.invalidate_message_request();
        self.invalidate_message_tail();
        self.invalidate_consumer_group_request();
        self.invalidate_schema_registry_request();
        self.invalidate_connect_request();
        self.invalidate_ksqldb_request();
        self.invalidate_acl_request();
        // 已提交的 Kafka Admin 写入请求不主动取消；只让迟到回调失效，
        // 让有界请求自然结束后释放 native Admin 资源。
        self.invalidate_topic_operation();
        self.invalidate_produce_operation();
        self.invalidate_acl_operation();
        self.invalidate_consumer_group_operation();
        self.invalidate_config_request();
    }
}

mod helpers;
use helpers::*;
mod loading;
use loading::*;
mod acls;
mod admin;
mod consumer_group_admin;
mod consumer_groups;
mod ksqldb;
mod message_producer;
mod message_tail;
mod messages;
mod metrics;
use ksqldb::KafkaKsqlDbState;
mod profile;
mod profile_delete;
mod profile_form;
mod remote_config;
mod remote_config_render;
mod render_broker_metrics;
mod render_brokers;
mod render_config;
mod render_connect;
mod render_consumer_group_helpers;
mod render_consumer_groups;
mod render_main;
mod render_message_detail;
mod render_message_tail;
mod render_messages;
mod render_metrics;
mod render_metrics_partition;
mod render_overview;
mod render_schema_registry;
mod render_sidebar;
mod render_topic_detail;
mod render_topics;
mod render_workspace;
mod runtime_state;
#[cfg(test)]
mod tests;
mod view_state;

impl Focusable for KafkaView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for KafkaView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let compact = f32::from(window.viewport_size().width) < 900.0;
        h_flex()
            .id("kafka-root")
            .debug_selector(|| "kafka-root".into())
            .size_full()
            .min_w_0()
            .min_h_0()
            .when(compact, |root| root.flex_col().items_stretch())
            .bg(cx.theme().background)
            .child(self.render_sidebar(window, cx))
            .child(self.render_main(window, cx))
    }
}
