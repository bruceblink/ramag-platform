use std::sync::{Arc, atomic::AtomicBool};

use super::*;

impl KafkaService {
    /// 校验只读查询、转发到独立 ksqlDB 驱动，并在应用边界再次校验有界结果。
    pub async fn execute_ksqldb_query(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaKsqlDbQuery,
    ) -> Result<KafkaKsqlDbQueryResult> {
        self.execute_ksqldb_query_with_cancel(config, query, Arc::new(AtomicBool::new(false)))
            .await
    }

    /// 执行 ksqlDB 查询并把取消信号传到 HTTP 适配器；不记录查询正文。
    pub async fn execute_ksqldb_query_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaKsqlDbQuery,
        cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaKsqlDbQueryResult> {
        validate_config(config)?;
        query.validate().map_err(DomainError::InvalidConfig)?;
        let started = std::time::Instant::now();
        let result = self
            .ksqldb_driver
            .execute_query_with_cancel(config, query, cancelled)
            .await
            .and_then(|result| {
                result
                    .validate()
                    .map(|()| result)
                    .map_err(DomainError::InvalidConfig)
            });
        tracing::info!(
            operation = "kafka_ksqldb_query",
            cluster_id = %config.id,
            elapsed_ms = started.elapsed().as_millis(),
            success = result.is_ok(),
            result_row_count = result.as_ref().map_or(0, |result| result.rows.len()),
            result_truncated = result.as_ref().is_ok_and(|result| result.truncated),
            "ksqlDB read-only query completed"
        );
        result
    }
}
