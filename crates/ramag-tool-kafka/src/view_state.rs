use super::*;

impl KafkaView {
    /// 创建视图状态并异步加载本地配置；加载失败会保留在页面上，不伪装成空列表。
    pub fn new(service: Arc<KafkaService>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let cluster_search = input(window, cx, 4 * 1024, "搜索集群…", false, "");
        let topic_search = input(window, cx, 4 * 1024, "筛选 Topic…", false, "");
        let consumer_group_search = input(window, cx, 4 * 1024, "筛选消费者组…", false, "");
        let acl_principal_filter = input(
            window,
            cx,
            MAX_KAFKA_ACL_HOST_BYTES,
            "Principal（可选）",
            false,
            "",
        );
        let acl_host_filter = input(
            window,
            cx,
            MAX_KAFKA_ACL_HOST_BYTES,
            "Host（可选）",
            false,
            "",
        );
        let acl_resource_name_filter = input(
            window,
            cx,
            MAX_KAFKA_ACL_RESOURCE_NAME_BYTES,
            "资源名称（可选）",
            false,
            "",
        );
        let message_search = input(
            window,
            cx,
            4 * 1024,
            "搜索 Key / Value / Header（可选）",
            false,
            "",
        );
        let name = input(window, cx, 256, "集群名称", false, "");
        let bootstrap_servers = input(
            window,
            cx,
            16 * 1024,
            "broker-1:9092, broker-2:9092",
            false,
            "",
        );
        let client_id = input(window, cx, 256, "Client ID（可选）", false, "ramag-kafka");
        let sasl_username = input(window, cx, 4 * 1024, "SASL 用户名", false, "");
        let sasl_password = input(
            window,
            cx,
            64 * 1024,
            "SASL 密码（留空保持已保存密码）",
            true,
            "",
        );
        let remark = input(window, cx, 16 * 1024, "备注（可选）", false, "");
        let broker_metrics_endpoint = input(
            window,
            cx,
            4 * 1024,
            "Prometheus / exporter 指标端点（可选）",
            false,
            "",
        );
        let ca_cert_path = input(window, cx, 32 * 1024, "CA 证书路径（可选）", false, "");
        let client_cert_path = input(window, cx, 32 * 1024, "客户端证书路径（可选）", false, "");
        let client_key_path = input(window, cx, 32 * 1024, "客户端密钥路径（可选）", false, "");
        let config_resource_name = input(
            window,
            cx,
            MAX_KAFKA_CONFIG_RESOURCE_NAME_BYTES,
            "Topic 名称或 Broker ID",
            false,
            "",
        );
        let config_value = input(
            window,
            cx,
            MAX_KAFKA_CONFIG_VALUE_BYTES,
            "配置值",
            false,
            "",
        );
        let topic_input = input(window, cx, 249, "Topic", false, "");
        let partition_input = input(window, cx, 4 * 1024, "Partition，例如 0,1,2", false, "0");
        let topic_create_name = input(window, cx, 249, "新 Topic 名称", false, "");
        let topic_create_partitions = input(window, cx, 32, "初始 Partition 数量", false, "1");
        let topic_create_replication_factor = input(window, cx, 32, "副本因子", false, "1");
        let topic_target_partitions = input(window, cx, 32, "目标 Partition 总数", false, "");
        let acl_principal = input(
            window,
            cx,
            MAX_KAFKA_ACL_HOST_BYTES,
            "例如 User:app",
            false,
            "",
        );
        let acl_host = input(
            window,
            cx,
            MAX_KAFKA_ACL_HOST_BYTES,
            "例如 10.0.0.1 或 *",
            false,
            "*",
        );
        let acl_resource_name = input(
            window,
            cx,
            MAX_KAFKA_ACL_RESOURCE_NAME_BYTES,
            "Topic / Group 名称",
            false,
            "",
        );
        let start_offset_input = input(window, cx, 32, "起始 Offset（可选）", false, "0");
        let end_offset_input = input(window, cx, 32, "结束 Offset（可选）", false, "");
        let start_time_input = input(window, cx, 64, "起始时间 RFC3339（可选）", false, "");
        let end_time_input = input(window, cx, 64, "结束时间 RFC3339（可选）", false, "");
        let max_records_input = input(window, cx, 32, "最多读取条数", false, "200");
        let metrics_refresh_default = DEFAULT_KAFKA_METRICS_REFRESH_SECONDS.to_string();
        let metrics_refresh_seconds_input = input(
            window,
            cx,
            8,
            "指标刷新秒数",
            false,
            &metrics_refresh_default,
        );
        let message_tail_offset_input = input(window, cx, 32, "实时起始 Offset", false, "0");
        let message_tail_window_messages_input = input(window, cx, 32, "窗口条数", false, "500");
        let message_tail_window_bytes_input =
            input(window, cx, 32, "窗口字节数", false, "16777216");
        let message_tail_max_message_bytes_input =
            input(window, cx, 32, "单条最大字节数", false, "4194304");
        let message_tail_poll_timeout_input = input(window, cx, 16, "轮询毫秒", false, "250");

        let mut subscriptions = Vec::new();
        for field in [
            &cluster_search,
            &consumer_group_search,
            &acl_principal_filter,
            &acl_host_filter,
            &acl_resource_name_filter,
            &message_search,
            &name,
            &bootstrap_servers,
            &client_id,
            &sasl_username,
            &sasl_password,
            &remark,
            &broker_metrics_endpoint,
            &ca_cert_path,
            &client_cert_path,
            &client_key_path,
            &config_value,
            &topic_input,
            &partition_input,
            &topic_create_name,
            &topic_create_partitions,
            &topic_create_replication_factor,
            &topic_target_partitions,
            &acl_principal,
            &acl_host,
            &acl_resource_name,
            &start_offset_input,
            &end_offset_input,
            &start_time_input,
            &end_time_input,
            &max_records_input,
            &metrics_refresh_seconds_input,
            &message_tail_offset_input,
            &message_tail_window_messages_input,
            &message_tail_window_bytes_input,
            &message_tail_max_message_bytes_input,
            &message_tail_poll_timeout_input,
        ] {
            subscriptions.push(cx.subscribe(field, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.notice = None;
                    cx.notify();
                }
            }));
        }
        subscriptions.push(
            cx.subscribe(&topic_search, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.topic_page_index = 0;
                    this.topic_scroll
                        .0
                        .borrow()
                        .base_handle
                        .set_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)));
                    this.notice = None;
                    cx.notify();
                }
            }),
        );
        subscriptions.push(
            cx.subscribe(&topic_input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.invalidate_message_tail();
                    this.notice = None;
                    cx.notify();
                }
            }),
        );
        subscriptions.push(cx.subscribe(
            &config_resource_name,
            |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.invalidate_config_request();
                    this.notice = None;
                    cx.notify();
                }
            },
        ));
        for field in [
            &acl_principal_filter,
            &acl_host_filter,
            &acl_resource_name_filter,
        ] {
            subscriptions.push(cx.subscribe(field, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.clear_acl_snapshot();
                    this.notice = None;
                    cx.notify();
                }
            }));
        }

        let mut this = Self {
            service,
            clusters: Vec::new(),
            selected_cluster_id: None,
            selected_topic: None,
            metadata: None,
            topics: Vec::new(),
            topic_page_index: 0,
            topic_page_size: DEFAULT_TOPIC_PAGE_SIZE,
            topic_scroll: UniformListScrollHandle::new(),
            consumer_groups: Vec::new(),
            selected_consumer_group: None,
            consumer_group_error: None,
            message_page: None,
            message_tail_records: VecDeque::new(),
            message_tail_bytes: 0,
            message_tail_dropped_records: 0,
            message_tail_dropped_bytes: 0,
            message_tail_evicted_records: 0,
            message_tail_evicted_bytes: 0,
            message_tail_window_messages: DEFAULT_KAFKA_TAIL_WINDOW_MESSAGES,
            message_tail_window_bytes: DEFAULT_KAFKA_TAIL_WINDOW_BYTES,
            message_tail_scroll: UniformListScrollHandle::new(),
            message_tail_reconnect_attempt: 0,
            message_tail_connected: false,
            message_tail_paused: false,
            message_tail_running: false,
            selected_tail_message: None,
            metrics_snapshot: None,
            metrics_error: None,
            broker_metrics_snapshot: None,
            broker_metrics_error: None,
            metrics_loading: false,
            metrics_refresh_seconds_input,
            metrics_refresh_generation: 0,
            metrics_refresh_cancelled: None,
            selected_message: None,
            message_page_index: 0,
            message_page_size: DEFAULT_MESSAGE_PAGE_SIZE,
            message_page_scroll: ScrollHandle::new(),
            message_scroll: UniformListScrollHandle::new(),
            message_horizontal_scroll: ScrollHandle::new(),
            topic_partition_scroll: ScrollHandle::new(),
            consumer_group_scroll: UniformListScrollHandle::new(),
            acls: Vec::new(),
            selected_acl: None,
            acls_loaded: false,
            acl_error: None,
            acl_scroll: UniformListScrollHandle::new(),
            section: KafkaSection::Overview,
            cluster_search,
            topic_search,
            consumer_group_search,
            acl_principal_filter,
            acl_host_filter,
            acl_resource_name_filter,
            message_search,
            name,
            bootstrap_servers,
            client_id,
            sasl_username,
            sasl_password,
            remark,
            broker_metrics_endpoint,
            ca_cert_path,
            client_cert_path,
            client_key_path,
            config_resource_name,
            config_value,
            topic_input,
            partition_input,
            topic_create_name,
            topic_create_partitions,
            topic_create_replication_factor,
            topic_target_partitions,
            acl_principal,
            acl_host,
            acl_resource_name,
            start_offset_input,
            end_offset_input,
            start_time_input,
            end_time_input,
            max_records_input,
            message_tail_offset_input,
            message_tail_window_messages_input,
            message_tail_window_bytes_input,
            message_tail_max_message_bytes_input,
            message_tail_poll_timeout_input,
            search_fields: [true, true, true],
            range_mode: KafkaRangeMode::Offset,
            message_tail_start_mode: KafkaTailStartMode::Latest,
            security_protocol: KafkaSecurityProtocol::default(),
            sasl_mechanism: KafkaSaslMechanism::Plain,
            config_resource_type: KafkaConfigResourceType::Topic,
            acl_filter_resource_type: None,
            acl_filter_pattern_type: None,
            acl_filter_operation: None,
            acl_filter_permission: None,
            acl_resource_type: KafkaAclResourceType::Topic,
            acl_pattern_type: KafkaAclPatternType::Literal,
            acl_operation_kind: KafkaAclOperation::Read,
            acl_permission: KafkaAclPermission::Allow,
            config_entries: Vec::new(),
            editing_config_key: None,
            read_only: KafkaReadOnlyState::default(),
            loading_clusters: true,
            cluster_load_error: None,
            loading_runtime: false,
            runtime_error: None,
            loading_messages: false,
            loading_consumer_groups: false,
            loading_acls: false,
            loading_configs: false,
            testing: false,
            saving: false,
            deleting: false,
            updating_config: false,
            acl_operation: false,
            exporting: false,
            profile_operation_id: 0,
            cluster_request_id: 0,
            runtime_request_id: 0,
            message_request_id: 0,
            message_read_cancelled: None,
            message_tail_request_id: 0,
            message_tail_cancelled: None,
            consumer_group_request_id: 0,
            config_request_id: 0,
            acl_request_id: 0,
            topic_operation_id: 0,
            acl_operation_id: 0,
            topic_operation: false,
            notice: None,
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        this.load_clusters(window, cx);
        this
    }
}
