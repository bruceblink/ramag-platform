use super::*;

impl RdkafkaTransport {
    pub(super) fn fetch_metadata_blocking(
        &self,
        config: &KafkaClusterConfig,
        operation: &'static str,
    ) -> Result<(BaseConsumer, Metadata)> {
        let cancelled = AtomicBool::new(false);
        self.fetch_metadata_blocking_with_cancel(config, operation, &cancelled)
    }

    fn fetch_metadata_blocking_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        operation: &'static str,
        cancelled: &AtomicBool,
    ) -> Result<(BaseConsumer, Metadata)> {
        ensure_not_cancelled(cancelled, operation)?;
        let consumer = self.create_consumer(config)?;
        ensure_not_cancelled(cancelled, operation)?;
        let metadata = consumer
            .fetch_metadata(None, self.request_timeout)
            .map_err(|error| errors::map_kafka_error(error, operation))?;
        ensure_not_cancelled(cancelled, operation)?;
        Ok((consumer, metadata))
    }

    /// 读取当前集群的元数据；消费者没有 group.id，因此不会加入业务消费组。
    pub(super) fn cluster_metadata_blocking_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: &AtomicBool,
    ) -> Result<KafkaClusterMetadata> {
        ensure_not_cancelled(cancelled, "读取 Kafka 集群元数据")?;
        let (consumer, metadata) =
            self.fetch_metadata_blocking_with_cancel(config, "读取 Kafka 集群元数据", cancelled)?;
        ensure_not_cancelled(cancelled, "读取 Kafka 集群元数据")?;
        self.cluster_metadata_from_metadata(&consumer, &metadata)
    }

    pub(super) fn cluster_metadata_from_metadata(
        &self,
        consumer: &BaseConsumer,
        metadata: &Metadata,
    ) -> Result<KafkaClusterMetadata> {
        let brokers = metadata
            .brokers()
            .iter()
            .map(|broker| {
                let port = u16::try_from(broker.port()).map_err(|_| {
                    DomainError::InvalidConfig(format!(
                        "Kafka Broker 端口超出 1 - 65535 范围：{}",
                        broker.port()
                    ))
                })?;
                Ok(KafkaBroker {
                    id: broker.id(),
                    host: broker.host().to_owned(),
                    port,
                    rack: None,
                    version: None,
                    is_controller: false,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if brokers.len() > MAX_KAFKA_BROKERS {
            return Err(DomainError::InvalidConfig(format!(
                "Kafka Broker 数量超过 {MAX_KAFKA_BROKERS} 个上限"
            )));
        }
        let result = KafkaClusterMetadata {
            cluster_id: consumer.client().fetch_cluster_id(self.request_timeout),
            // librdkafka 的 metadata wrapper 不暴露 Controller ID，不能根据 Broker 顺序猜测。
            controller_id: None,
            brokers,
            kafka_version: None,
        };
        result.validate().map_err(DomainError::InvalidConfig)?;
        Ok(result)
    }

    /// 读取 Topic、Partition 和水位；水位查询仍使用同一个无消费组读取客户端。
    pub(super) fn list_topics_blocking_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: &AtomicBool,
    ) -> Result<Vec<KafkaTopic>> {
        let (consumer, metadata) =
            self.fetch_metadata_blocking_with_cancel(config, "读取 Kafka Topic 元数据", cancelled)?;
        self.list_topics_from_metadata_with_cancel(&consumer, &metadata, cancelled)
    }

    pub(super) fn list_topics_from_metadata(
        &self,
        consumer: &BaseConsumer,
        metadata: &Metadata,
    ) -> Result<Vec<KafkaTopic>> {
        let cancelled = AtomicBool::new(false);
        self.list_topics_from_metadata_with_cancel(consumer, metadata, &cancelled)
    }

    fn list_topics_from_metadata_with_cancel(
        &self,
        consumer: &BaseConsumer,
        metadata: &Metadata,
        cancelled: &AtomicBool,
    ) -> Result<Vec<KafkaTopic>> {
        ensure_not_cancelled(cancelled, "读取 Kafka Topic 元数据")?;
        if metadata.topics().len() > MAX_KAFKA_TOPICS {
            return Err(DomainError::InvalidConfig(format!(
                "Kafka Topic 数量超过 {MAX_KAFKA_TOPICS} 个上限"
            )));
        }

        let mut topics = Vec::with_capacity(metadata.topics().len());
        let mut total_partitions = 0usize;
        let mut total_replica_ids = 0usize;
        for topic_metadata in metadata.topics() {
            ensure_not_cancelled(cancelled, "读取 Kafka Topic 元数据")?;
            if let Some(error) = topic_metadata.error() {
                return Err(errors::map_kafka_error(
                    rdkafka::error::KafkaError::MetadataFetch(error.into()),
                    "读取 Kafka Topic 元数据",
                ));
            }
            validate_partition_budget(
                &mut total_partitions,
                topic_metadata.name(),
                topic_metadata.partitions().len(),
            )?;
            let name = topic_metadata.name().to_owned();
            let mut partitions = Vec::with_capacity(topic_metadata.partitions().len());
            for partition_metadata in topic_metadata.partitions() {
                ensure_not_cancelled(cancelled, "读取 Kafka Topic 元数据")?;
                validate_partition_replica_budget(
                    &mut total_replica_ids,
                    topic_metadata.name(),
                    partition_metadata.id(),
                    partition_metadata.replicas().len(),
                    partition_metadata.isr().len(),
                )?;
                let (low, high) = consumer
                    .fetch_watermarks(&name, partition_metadata.id(), self.request_timeout)
                    .map_err(|error| errors::map_kafka_error(error, "读取 Kafka Partition 水位"))?;
                ensure_not_cancelled(cancelled, "读取 Kafka Topic 元数据")?;
                partitions.push(KafkaPartition {
                    id: partition_metadata.id(),
                    leader: (partition_metadata.leader() >= 0)
                        .then_some(partition_metadata.leader()),
                    replicas: partition_metadata.replicas().to_vec(),
                    isr: partition_metadata.isr().to_vec(),
                    low_watermark: (low >= 0).then_some(low),
                    high_watermark: (high >= 0).then_some(high),
                });
            }
            let topic = KafkaTopic {
                internal: name.starts_with("__"),
                name,
                partitions,
            };
            topic.validate().map_err(DomainError::InvalidConfig)?;
            topics.push(topic);
        }
        ensure_not_cancelled(cancelled, "读取 Kafka Topic 元数据")?;
        Ok(topics)
    }
}
