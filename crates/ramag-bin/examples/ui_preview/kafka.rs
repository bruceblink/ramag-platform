//! 隔离 Kafka 原生验收，仅连接本机 Docker 测试 Broker。

use std::sync::Arc;

use ramag_app::KafkaService;
use ramag_domain::{entities::KafkaClusterConfig, traits::Storage};
use ramag_infra_kafka::RdkafkaTransport;

pub(super) fn service(storage: Arc<dyn Storage>) -> anyhow::Result<Arc<KafkaService>> {
    let service = Arc::new(KafkaService::new(
        Arc::new(RdkafkaTransport::new()),
        storage,
    ));
    let cluster = KafkaClusterConfig::new("本机 Kafka 测试集群", vec!["127.0.0.1:19092".into()]);
    futures::executor::block_on(service.save_cluster(&cluster))?;
    Ok(service)
}
