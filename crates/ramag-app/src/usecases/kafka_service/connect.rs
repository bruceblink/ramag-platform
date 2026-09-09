use std::collections::HashSet;
use std::sync::{Arc, atomic::AtomicBool};

use super::*;

impl KafkaService {
    pub async fn list_connectors(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<Vec<KafkaConnectConnector>> {
        self.list_connectors_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    /// 读取 Kafka Connect 连接器状态，并在应用边界再次限制名称、Task 和错误文本。
    pub async fn list_connectors_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<KafkaConnectConnector>> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        let started = std::time::Instant::now();
        let result = self
            .connect_driver
            .list_connectors_with_cancel(config, cancelled)
            .await
            .and_then(validate_connectors);
        tracing::info!(
            operation = "kafka_connectors",
            cluster_id = %config.id,
            elapsed_ms = started.elapsed().as_millis(),
            success = result.is_ok(),
            result_count = result.as_ref().map_or(0, Vec::len),
            "Kafka Connect connector listing completed"
        );
        result
    }
}

fn validate_connectors(
    connectors: Vec<KafkaConnectConnector>,
) -> Result<Vec<KafkaConnectConnector>> {
    if connectors.len() > ramag_domain::entities::MAX_KAFKA_CONNECTORS {
        return Err(DomainError::InvalidConfig(format!(
            "Kafka Connect 连接器数量超过 {} 个上限",
            ramag_domain::entities::MAX_KAFKA_CONNECTORS
        )));
    }
    let mut names = HashSet::with_capacity(connectors.len());
    for connector in &connectors {
        connector.validate().map_err(DomainError::InvalidConfig)?;
        if !names.insert(connector.name.as_str()) {
            return Err(DomainError::InvalidConfig(format!(
                "Kafka Connect 连接器名称重复：{}",
                connector.name
            )));
        }
    }
    Ok(connectors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::{KafkaConnectConnector, KafkaConnectTask, MAX_KAFKA_CONNECTORS};

    fn connector(name: &str) -> KafkaConnectConnector {
        KafkaConnectConnector {
            name: name.into(),
            connector_type: Some("sink".into()),
            state: "RUNNING".into(),
            worker_id: Some("worker-1".into()),
            tasks: vec![KafkaConnectTask {
                id: 0,
                state: "RUNNING".into(),
                worker_id: Some("worker-1".into()),
                trace: None,
            }],
            error: None,
        }
    }

    #[test]
    fn validates_connector_names_and_tasks() {
        let valid = validate_connectors(vec![connector("orders-sink")]).expect("valid connector");
        assert_eq!(valid.len(), 1);
        let error = validate_connectors(vec![connector("orders-sink"), connector("orders-sink")])
            .expect_err("duplicate connector");
        assert!(error.to_string().contains("名称重复"));
    }

    #[test]
    fn caps_connector_snapshots() {
        let connectors = (0..=MAX_KAFKA_CONNECTORS)
            .map(|index| connector(&format!("connector-{index}")))
            .collect();
        let error = validate_connectors(connectors).expect_err("oversized connector snapshot");
        assert!(error.to_string().contains("超过 2000"));
    }
}
