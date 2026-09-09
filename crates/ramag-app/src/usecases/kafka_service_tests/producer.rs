//! Message production fixtures and application-boundary regression tests.
use super::*;

struct RecordingProducerDriver {
    calls: Arc<Mutex<Vec<KafkaMessageProduceRequest>>>,
}

#[async_trait]
impl KafkaProducerDriver for RecordingProducerDriver {
    async fn produce_message(
        &self,
        _config: &KafkaClusterConfig,
        request: &KafkaMessageProduceRequest,
    ) -> Result<KafkaMessageProduceResult> {
        self.calls.lock().unwrap().push(request.clone());
        Ok(KafkaMessageProduceResult::new(
            request.topic.clone(),
            request.partition.unwrap_or(0),
            12,
            None,
        ))
    }
}

struct MismatchedProducerDriver;

#[async_trait]
impl KafkaProducerDriver for MismatchedProducerDriver {
    async fn produce_message(
        &self,
        _config: &KafkaClusterConfig,
        _request: &KafkaMessageProduceRequest,
    ) -> Result<KafkaMessageProduceResult> {
        Ok(KafkaMessageProduceResult::new("other-topic", 0, 1, None))
    }
}

fn service_with_producer(producer: Arc<dyn KafkaProducerDriver>) -> KafkaService {
    KafkaService::new(Arc::new(NoopKafkaDriver), Arc::new(NoopStorage))
        .with_producer_driver(producer)
}

#[test]
fn message_production_requires_admin_mode_and_forwards_request() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let service = service_with_producer(Arc::new(RecordingProducerDriver {
        calls: calls.clone(),
    }));
    let mut config = KafkaClusterConfig::new("message-produce", vec!["localhost:9092".into()]);
    let request = KafkaMessageProduceRequest::new("events", b"payload".to_vec())
        .with_partition(2)
        .with_key(b"key".to_vec())
        .with_headers(vec![ramag_domain::entities::KafkaMessageHeader {
            key: "trace-id".into(),
            value: Some(b"abc".to_vec()),
        }]);

    assert!(matches!(
        smol::block_on(service.produce_message(&config, &request)),
        Err(DomainError::Forbidden(message)) if message == ramag_domain::error::READ_ONLY_MESSAGE
    ));
    assert!(calls.lock().unwrap().is_empty());

    config.read_only = KafkaReadOnlyState::ReadWrite;
    let result = smol::block_on(service.produce_message(&config, &request))
        .expect("admin mode should forward message production");
    assert_eq!(result.topic, "events");
    assert_eq!(result.partition, 2);
    assert_eq!(result.offset, 12);
    assert_eq!(calls.lock().unwrap().as_slice(), &[request]);
}

#[test]
fn message_production_rejects_driver_result_for_another_topic() {
    let service = service_with_producer(Arc::new(MismatchedProducerDriver));
    let mut config = KafkaClusterConfig::new("message-produce", vec!["localhost:9092".into()]);
    config.read_only = KafkaReadOnlyState::ReadWrite;
    let request = KafkaMessageProduceRequest::new("events", b"payload".to_vec());

    let result = smol::block_on(service.produce_message(&config, &request));
    assert!(matches!(
        result,
        Err(DomainError::InvalidConfig(message)) if message.contains("Topic")
    ));
}
