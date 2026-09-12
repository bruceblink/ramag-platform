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
