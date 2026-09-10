use super::*;

fn docker_broker_exporter_endpoint() -> Option<String> {
    match env::var("RAMAG_TEST_KAFKA_BROKER_METRICS") {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!(
                "Skipping Docker Kafka JMX exporter integration test; set RAMAG_TEST_KAFKA_BROKER_METRICS or run scripts/kafka-test/kafka-test.ps1 test."
            );
            None
        }
    }
}

/// Requests metrics from the real Kafka JVM through the local JMX exporter.
#[test]
fn docker_kafka_reads_real_broker_jmx_exporter() -> Result<(), Box<dyn std::error::Error>> {
    install_tls_crypto_provider();
    let Some(endpoint) = docker_broker_exporter_endpoint() else {
        return Ok(());
    };
    let mut config =
        KafkaClusterConfig::new("ramag-docker-kafka-jmx", vec!["127.0.0.1:19092".into()]);
    config.broker_metrics.endpoint = Some(endpoint);
    let driver = PrometheusBrokerMetricsDriver::new()?;

    let snapshot = smol::block_on(driver.broker_metrics_snapshot(&config))?;
    assert_eq!(
        snapshot.state,
        ramag_domain::entities::KafkaMetricsSnapshotState::Ready
    );
    assert_eq!(snapshot.brokers.len(), 1);
    let broker = &snapshot.brokers[0];
    assert_eq!(broker.broker_id, 1);
    assert!(broker.cpu_usage_percent.is_some_and(|value| value >= 0.0));
    assert!(broker.memory_used_bytes.is_some_and(|value| value > 0.0));
    assert!(broker.disk_used_bytes.is_some_and(|value| value > 0.0));
    assert!(broker.request_latency_ms.is_some_and(|value| value >= 0.0));
    Ok(())
}
