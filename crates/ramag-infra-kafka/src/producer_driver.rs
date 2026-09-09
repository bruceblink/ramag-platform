use super::RdkafkaTransport;
use ramag_domain::entities::{
    KafkaClusterConfig, KafkaMessageProduceRequest, KafkaMessageProduceResult,
};
use ramag_domain::error::{DomainError, READ_ONLY_MESSAGE, Result};
#[cfg(feature = "cmake-build")]
use ramag_domain::error::{KafkaError, KafkaErrorCategory};
use ramag_domain::traits::KafkaProducerDriver;

fn validate_producer_config(config: &KafkaClusterConfig) -> Result<()> {
    config.validate().map_err(DomainError::InvalidConfig)?;
    if !config.read_only.allows_admin() {
        return Err(DomainError::Forbidden(READ_ONLY_MESSAGE.into()));
    }
    Ok(())
}

#[cfg(feature = "cmake-build")]
#[async_trait::async_trait]
impl KafkaProducerDriver for RdkafkaTransport {
    async fn produce_message(
        &self,
        config: &KafkaClusterConfig,
        request: &KafkaMessageProduceRequest,
    ) -> Result<KafkaMessageProduceResult> {
        validate_producer_config(config)?;
        request.validate().map_err(DomainError::InvalidConfig)?;
        let driver = *self;
        let config = config.clone();
        let request = request.clone();
        smol::unblock(move || produce_message_blocking(&driver, &config, &request)).await
    }
}

#[cfg(feature = "cmake-build")]
fn produce_message_blocking(
    driver: &RdkafkaTransport,
    config: &KafkaClusterConfig,
    request: &KafkaMessageProduceRequest,
) -> Result<KafkaMessageProduceResult> {
    use rdkafka::message::{Header, OwnedHeaders};
    use rdkafka::producer::{FutureProducer, FutureRecord};

    let client_config = super::config::build_producer_config(config, driver.request_timeout)?;
    let producer: FutureProducer = client_config
        .create()
        .map_err(|error| super::errors::map_kafka_error(error, "创建 Kafka 消息生产器"))?;
    let mut record = FutureRecord::to(&request.topic).payload(&request.value);
    if let Some(partition) = request.partition {
        record = record.partition(partition);
    }
    if let Some(key) = request.key.as_ref() {
        record = record.key(key);
    }
    if !request.headers.is_empty() {
        let headers = request.headers.iter().fold(
            OwnedHeaders::new_with_capacity(request.headers.len()),
            |headers, header| {
                headers.insert(Header {
                    key: &header.key,
                    value: header.value.as_deref(),
                })
            },
        );
        record = record.headers(headers);
    }
    let delivery = producer
        .send_result(record)
        .map_err(|(error, _record)| super::errors::map_kafka_error(error, "写入 Kafka 消息"))?;
    let delivery = smol::block_on(delivery).map_err(|_| {
        DomainError::Kafka(KafkaError::new(
            KafkaErrorCategory::Network,
            "写入 Kafka 消息",
            "Kafka 消息生产结果通道已关闭",
        ))
    })?;
    let delivery = delivery
        .map_err(|(error, _message)| super::errors::map_kafka_error(error, "写入 Kafka 消息"))?;
    Ok(KafkaMessageProduceResult::new(
        request.topic.clone(),
        delivery.partition,
        delivery.offset,
        delivery
            .timestamp
            .to_millis()
            .and_then(chrono::DateTime::<chrono::Utc>::from_timestamp_millis),
    ))
}

#[cfg(not(feature = "cmake-build"))]
#[async_trait::async_trait]
impl KafkaProducerDriver for RdkafkaTransport {
    async fn produce_message(
        &self,
        config: &KafkaClusterConfig,
        request: &KafkaMessageProduceRequest,
    ) -> Result<KafkaMessageProduceResult> {
        let _ = self.request_timeout;
        validate_producer_config(config)?;
        request.validate().map_err(DomainError::InvalidConfig)?;
        Err(super::native_client_unavailable("写入 Kafka 消息"))
    }
}
