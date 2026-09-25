use ramag_domain::entities::{ConnectionConfig, Query, VirtualView};
use ramag_domain::error::{DomainError, Result};

use super::ConnectionService;

impl ConnectionService {
    /// Reads virtual view metadata while preserving the standard idempotent-read retry behavior.
    pub async fn list_virtual_views(&self, config: &ConnectionConfig) -> Result<Vec<VirtualView>> {
        let result = retry_idempotent_read!(
            config.id,
            self.evict_pool(config),
            self.driver_for(config)?.list_virtual_views(config).await
        );
        super::log_connection_result("sql_list_virtual_views", config, None, None, &result);
        result
    }

    /// Returns a fixed driver query, rejecting unknown names instead of interpolating tree text.
    pub fn virtual_view_query(&self, config: &ConnectionConfig, name: &str) -> Result<Query> {
        if name.is_empty() || name.chars().any(char::is_control) {
            return Err(DomainError::InvalidConfig("虚拟视图名称无效".into()));
        }
        self.driver_for(config)?
            .virtual_view_query(name)
            .map(Query::new)
            .ok_or_else(|| DomainError::NotImplemented(format!("虚拟视图不支持：{name}")))
    }
}
