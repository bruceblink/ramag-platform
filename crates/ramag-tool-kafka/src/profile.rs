use super::*;

/// 只接受代次和当前上下文都一致的结果，避免迟到任务覆盖新页面。
pub(super) fn request_matches<T: PartialEq>(
    current_request_id: u64,
    request_id: u64,
    current_context: Option<&T>,
    request_context: Option<&T>,
) -> bool {
    current_request_id == request_id && current_context == request_context
}

impl KafkaView {
    /// 使保存、连接测试和删除任务失效；连接测试会发出取消信号，迟到结果不能更新视图。
    pub(super) fn invalidate_profile_operation(&mut self) {
        self.profile_operation_id = self.profile_operation_id.wrapping_add(1);
        self.saving = false;
        self.testing = false;
        self.deleting = false;
        if let Some(cancelled) = self.connection_test_cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
    }

    /// 使元数据刷新任务失效，防止切换到草稿或删除配置后恢复旧的集群快照。
    pub(super) fn invalidate_runtime_request(&mut self) {
        self.runtime_request_id = self.runtime_request_id.wrapping_add(1);
        self.loading_runtime = false;
        self.runtime_error = None;
        if let Some(cancelled) = self.runtime_cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
        self.invalidate_metrics_refresh();
        self.clear_metrics_snapshot();
    }

    /// 重新读取本地配置；每次读取都有独立代次，重复触发时只接受最后一次结果。
    pub(super) fn load_clusters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cluster_request_id = self.cluster_request_id.wrapping_add(1);
        let request_id = self.cluster_request_id;
        self.loading_clusters = true;
        self.cluster_load_error = None;
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.list_clusters().await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.cluster_request_id != request_id {
                    return;
                }
                this.loading_clusters = false;
                match result {
                    Ok(clusters) => {
                        this.cluster_load_error = None;
                        let selected = this
                            .selected_cluster_id
                            .clone()
                            .filter(|id| clusters.iter().any(|cluster| &cluster.id == id));
                        this.clusters = clusters;
                        if let Some(id) = selected {
                            if let Some(config) = this.cluster_by_id(&id).cloned() {
                                this.selected_cluster_id = Some(id);
                                this.set_form_from_config(&config, window, cx);
                            }
                        } else if this.selected_cluster_id.is_some() {
                            this.new_profile(window, cx);
                        }
                    }
                    Err(error) => {
                        this.cluster_load_error = Some(error.user_message());
                        this.notice =
                            Some((format!("加载集群配置失败：{}", error.user_message()), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn retry_cluster_load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading_clusters {
            return;
        }
        self.load_clusters(window, cx);
        cx.notify();
    }

    /// 重新读取当前已保存集群的 Broker 元数据和 Topic；草稿或并发操作不会触发请求。
    pub(super) fn retry_runtime(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading_runtime
            || self.testing
            || self.saving
            || self.deleting
            || self.topic_operation
            || self.acl_operation
            || self.consumer_group_operation
        {
            return;
        }
        let Some(config) = self.selected_config() else {
            return;
        };
        self.load_runtime(config, window, cx);
    }

    pub(super) fn cluster_by_id(&self, id: &KafkaClusterId) -> Option<&KafkaClusterConfig> {
        self.clusters.iter().find(|cluster| &cluster.id == id)
    }

    pub(super) fn selected_config(&self) -> Option<KafkaClusterConfig> {
        self.selected_cluster_id
            .as_ref()
            .and_then(|id| self.cluster_by_id(id))
            .cloned()
    }

    pub(super) fn new_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.invalidate_profile_operation();
        self.invalidate_runtime_request();
        self.invalidate_message_request();
        self.invalidate_produce_operation();
        self.invalidate_consumer_group_request();
        self.invalidate_consumer_group_operation();
        self.invalidate_topic_operation();
        self.invalidate_config_request();
        self.clear_schema_registry_snapshot();
        self.clear_connect_snapshot();
        self.reset_ksqldb_query();
        self.reset_acl_state(window, cx);
        self.selected_cluster_id = None;
        self.selected_topic = None;
        self.metadata = None;
        self.topics.clear();
        self.reset_topic_paging();
        self.consumer_groups.clear();
        self.selected_consumer_group = None;
        self.consumer_group_error = None;
        self.message_page = None;
        self.clear_message_tail(cx);
        self.selected_message = None;
        self.section = KafkaSection::Config;
        self.security_protocol = KafkaSecurityProtocol::default();
        self.sasl_mechanism = KafkaSaslMechanism::Plain;
        self.read_only = KafkaReadOnlyState::default();
        for field in [
            &self.name,
            &self.bootstrap_servers,
            &self.client_id,
            &self.consumer_group_search,
            &self.sasl_username,
            &self.sasl_password,
            &self.remark,
            &self.broker_metrics_endpoint,
            &self.schema_registry_endpoint,
            &self.schema_registry_username,
            &self.schema_registry_password,
            &self.connect_endpoint,
            &self.connect_username,
            &self.connect_password,
            &self.ksqldb.endpoint,
            &self.ca_cert_path,
            &self.client_cert_path,
            &self.client_key_path,
            &self.config_resource_name,
            &self.config_value,
            &self.topic_input,
            &self.produce_topic_input,
            &self.produce_partition_input,
            &self.produce_key_input,
            &self.produce_value_input,
            &self.topic_create_name,
            &self.topic_target_partitions,
        ] {
            set_value(field, "", window, cx);
        }
        set_value(&self.topic_create_partitions, "1", window, cx);
        set_value(&self.topic_create_replication_factor, "1", window, cx);
        self.bootstrap_servers.update(cx, |state, cx| {
            state.set_placeholder("broker-1:9092, broker-2:9092", window, cx);
        });
        self.sasl_password.update(cx, |state, cx| {
            state.set_placeholder("SASL 密码", window, cx);
        });
        self.schema_registry_password.update(cx, |state, cx| {
            state.set_placeholder("Schema Registry 密码", window, cx);
        });
        self.connect_password.update(cx, |state, cx| {
            state.set_placeholder("Kafka Connect 密码", window, cx);
        });
        self.notice = None;
        cx.notify();
    }

    pub(super) fn select_cluster(
        &mut self,
        id: KafkaClusterId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(config) = self.cluster_by_id(&id).cloned() else {
            return;
        };
        self.invalidate_profile_operation();
        self.invalidate_runtime_request();
        self.invalidate_message_request();
        self.invalidate_produce_operation();
        self.invalidate_consumer_group_request();
        self.invalidate_consumer_group_operation();
        self.clear_schema_registry_snapshot();
        self.clear_connect_snapshot();
        self.reset_ksqldb_query();
        self.clear_acl_snapshot();
        self.invalidate_acl_operation();
        self.selected_cluster_id = Some(id);
        self.selected_topic = None;
        self.invalidate_topic_operation();
        self.metadata = None;
        self.topics.clear();
        self.reset_topic_paging();
        self.consumer_groups.clear();
        self.selected_consumer_group = None;
        self.consumer_group_error = None;
        self.message_page = None;
        self.clear_message_tail(cx);
        self.selected_message = None;
        self.section = KafkaSection::Overview;
        self.set_form_from_config(&config, window, cx);
        self.notice = None;
        self.load_runtime(config, window, cx);
        cx.notify();
    }

    /// 组装当前表单；保存或连接测试都复用这条路径，保证校验规则一致。
    pub(super) fn form_config(&self, cx: &App) -> Result<KafkaClusterConfig, String> {
        let name = value(&self.name, cx);
        let bootstrap_servers = parse_bootstrap_servers(&value(&self.bootstrap_servers, cx));
        let mut config = self
            .selected_config()
            .unwrap_or_else(|| KafkaClusterConfig::new(name.clone(), bootstrap_servers.clone()));
        config.name = name;
        config.bootstrap_servers = bootstrap_servers;
        config.security_protocol = self.security_protocol;
        config.client_id = optional_value(&self.client_id, cx);
        config.remark = optional_value(&self.remark, cx);
        config.broker_metrics.endpoint = optional_value(&self.broker_metrics_endpoint, cx);
        config.schema_registry.endpoint = optional_value(&self.schema_registry_endpoint, cx);
        config.schema_registry.username = optional_value(&self.schema_registry_username, cx);
        if config.schema_registry.endpoint.is_none() {
            config.schema_registry.username = None;
            config.schema_registry.password = None;
        } else if let Some(password) = optional_value(&self.schema_registry_password, cx) {
            config.schema_registry.password = Some(password);
        }
        config.connect.endpoint = optional_value(&self.connect_endpoint, cx);
        config.connect.username = optional_value(&self.connect_username, cx);
        if config.connect.endpoint.is_none() {
            config.connect.username = None;
            config.connect.password = None;
        } else if let Some(password) = optional_value(&self.connect_password, cx) {
            config.connect.password = Some(password);
        }
        config.ksqldb.endpoint = optional_value(&self.ksqldb.endpoint, cx);
        config.tls = KafkaTlsConfig {
            verify: config.tls.verify,
            ca_cert_path: optional_value(&self.ca_cert_path, cx),
            client_cert_path: optional_value(&self.client_cert_path, cx),
            client_key_path: optional_value(&self.client_key_path, cx),
        };
        if self.security_protocol.uses_sasl() {
            config.sasl_mechanism = Some(self.sasl_mechanism);
            config.sasl_username = optional_value(&self.sasl_username, cx);
            if let Some(password) = optional_value(&self.sasl_password, cx) {
                config.sasl_password = Some(password);
            }
        } else {
            config.sasl_mechanism = None;
            config.sasl_username = None;
            config.sasl_password = None;
        }
        config.read_only = self.read_only;
        config
            .validate()
            .map(|()| config)
            .map_err(|error| error.to_string())
    }

    pub(super) fn save_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || self.testing || self.deleting {
            return;
        }
        let config = match self.form_config(cx) {
            Ok(config) => config,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let service = self.service.clone();
        let id = config.id.clone();
        let name = config.name.clone();
        let context_cluster_id = self.selected_cluster_id.clone();
        self.profile_operation_id = self.profile_operation_id.wrapping_add(1);
        let operation_id = self.profile_operation_id;
        self.saving = true;
        self.notice = Some(("正在保存本地加密配置…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service.save_cluster(&config).await;
            let _ = this.update_in(cx, |this, _window, cx| {
                if !request_matches(
                    this.profile_operation_id,
                    operation_id,
                    this.selected_cluster_id.as_ref(),
                    context_cluster_id.as_ref(),
                ) {
                    return;
                }
                this.saving = false;
                match result {
                    Ok(()) => {
                        if let Some(existing) =
                            this.clusters.iter_mut().find(|cluster| cluster.id == id)
                        {
                            *existing = config;
                        } else {
                            this.clusters.push(config);
                        }
                        this.selected_cluster_id = Some(id);
                        this.clear_schema_registry_snapshot();
                        this.clear_connect_snapshot();
                        this.notice = Some((format!("已保存「{name}」，配置保存在本机",), false));
                    }
                    Err(error) => {
                        this.notice = Some((format!("保存失败：{}", error.user_message()), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn test_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.testing || self.saving || self.deleting {
            return;
        }
        let config = match self.form_config(cx) {
            Ok(config) => config,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let service = self.service.clone();
        let context_cluster_id = self.selected_cluster_id.clone();
        self.profile_operation_id = self.profile_operation_id.wrapping_add(1);
        let operation_id = self.profile_operation_id;
        let cancelled = Arc::new(AtomicBool::new(false));
        self.connection_test_cancelled = Some(cancelled.clone());
        self.testing = true;
        self.notice = Some(("正在连接 Kafka Broker…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .test_connection_with_cancel(&config, cancelled)
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !request_matches(
                    this.profile_operation_id,
                    operation_id,
                    this.selected_cluster_id.as_ref(),
                    context_cluster_id.as_ref(),
                ) {
                    return;
                }
                this.connection_test_cancelled = None;
                this.testing = false;
                match result {
                    Ok(()) => {
                        this.notice = Some(("连接成功；正在读取集群元数据和 Topic…".into(), false));
                        this.load_runtime(config, window, cx);
                    }
                    Err(error) => {
                        this.mark_runtime_failure("连接 Kafka", &error);
                        this.notice = Some((format!("连接失败：{}", error.user_message()), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn load_runtime(
        &mut self,
        config: KafkaClusterConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_runtime_request();
        self.invalidate_message_request();
        self.invalidate_consumer_group_request();
        self.clear_schema_registry_snapshot();
        self.clear_connect_snapshot();
        self.clear_acl_snapshot();
        self.invalidate_acl_operation();
        self.consumer_groups.clear();
        self.selected_consumer_group = None;
        self.consumer_group_error = None;
        self.reset_topic_paging();
        let request_id = self.runtime_request_id;
        let context_cluster_id = self.selected_cluster_id.clone();
        self.runtime_error = None;
        self.loading_runtime = true;
        let cancelled = Arc::new(AtomicBool::new(false));
        self.runtime_cancelled = Some(cancelled.clone());
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let metadata = service
                .cluster_metadata_with_cancel(&config, cancelled.clone())
                .await;
            let topics = service.list_topics_with_cancel(&config, cancelled).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !request_matches(
                    this.runtime_request_id,
                    request_id,
                    this.selected_cluster_id.as_ref(),
                    context_cluster_id.as_ref(),
                ) {
                    return;
                }
                this.runtime_cancelled = None;
                this.loading_runtime = false;
                match (metadata, topics) {
                    (Ok(metadata), Ok(topics)) => {
                        this.metadata = Some(metadata);
                        this.topics = topics;
                        this.runtime_error = None;
                        this.notice = Some(("集群元数据已更新".into(), false));
                    }
                    (Err(metadata_error), Ok(topics)) => {
                        this.metadata = None;
                        this.topics = topics;
                        this.runtime_error =
                            Some(format!("元数据读取失败：{}", metadata_error.user_message()));
                        this.notice = Some((
                            format!("元数据读取失败：{}", metadata_error.user_message()),
                            true,
                        ));
                    }
                    (Ok(metadata), Err(topic_error)) => {
                        this.metadata = Some(metadata);
                        this.topics.clear();
                        this.runtime_error = Some(format!(
                            "Topic 列表读取失败：{}",
                            topic_error.user_message()
                        ));
                        this.notice = Some((
                            format!("Topic 列表读取失败：{}", topic_error.user_message()),
                            true,
                        ));
                    }
                    (Err(error), Err(topic_error)) => {
                        this.metadata = None;
                        this.topics.clear();
                        this.runtime_error = Some(format!(
                            "元数据读取失败：{}；Topic 列表读取失败：{}",
                            error.user_message(),
                            topic_error.user_message()
                        ));
                        this.notice = Some((
                            format!(
                                "元数据读取失败：{}；Topic 列表读取失败：{}",
                                error.user_message(),
                                topic_error.user_message()
                            ),
                            true,
                        ));
                    }
                }
                // 指标刷新和消费者组详细快照错峰，避免切换集群时同时保留两组大快照。
                if this.section == KafkaSection::ConsumerGroups
                    && this.selected_cluster_id.is_some()
                {
                    this.load_consumer_groups_with_metrics(config.clone(), window, cx, true);
                } else {
                    this.start_metrics_refresh(config.clone(), window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
}
