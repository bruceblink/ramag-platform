use super::kafka::*;

fn valid_cluster() -> KafkaClusterConfig {
    KafkaClusterConfig::new("local", vec!["127.0.0.1:9092".into()])
}

#[test]
fn broker_metrics_endpoint_is_optional_bounded_and_redacted() {
    let mut config = valid_cluster();
    assert!(config.broker_metrics.validate().is_ok());

    config.broker_metrics.endpoint = Some("http://127.0.0.1:9090/metrics".into());
    assert!(config.validate().is_ok());
    config.broker_metrics.username = Some("metrics-user".into());
    config.broker_metrics.password = Some("metrics-password".into());
    assert!(config.validate().is_ok());
    let rendered = format!("{:?}", config.broker_metrics);
    assert!(!rendered.contains("127.0.0.1"));
    assert!(!rendered.contains("metrics-user"));
    assert!(!rendered.contains("metrics-password"));
    assert!(rendered.contains("[CONFIGURED]"));

    config.broker_metrics.password = None;
    assert!(config.validate().is_err());
    config.broker_metrics.username = None;
    config.broker_metrics.endpoint = None;
    assert!(config.broker_metrics.validate().is_ok());
    config.broker_metrics.username = Some("orphaned-user".into());
    assert!(config.broker_metrics.validate().is_err());

    config.broker_metrics.username = None;
    config.broker_metrics.password = None;
    config.broker_metrics.endpoint = Some("ftp://127.0.0.1/metrics".into());
    assert!(config.validate().is_err());
    config.broker_metrics.endpoint = Some("http://127.0.0.1/metrics\nnext".into());
    assert!(config.validate().is_err());
}
