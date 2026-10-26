use super::*;

impl ConnectionService {
    /// Executes a cancellable internal query without adding generated SQL to query history.
    pub async fn execute_cancellable_without_history(
        &self,
        config: &ConnectionConfig,
        query: &Query,
        handle: CancelHandle,
    ) -> Result<QueryResult> {
        let result = match self.driver_for(config) {
            Ok(driver) => driver.execute_cancellable(config, query, handle).await,
            Err(error) => Err(error),
        };
        log_query_result(config, query, &result, false);
        result
    }
}
