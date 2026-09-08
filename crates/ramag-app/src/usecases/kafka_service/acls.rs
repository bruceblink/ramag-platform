use std::sync::{Arc, atomic::AtomicBool};

use ramag_domain::entities::{KafkaAcl, KafkaAclFilter, KafkaClusterConfig};
use ramag_domain::error::{DomainError, Result};

use super::{
    KafkaService, log_acl_result, log_runtime_result, validate_acls, validate_admin_request,
    validate_config,
};

impl KafkaService {
    /// 按明确过滤条件读取 Kafka ACL；返回结果在应用边界再次执行数量和字段校验。
    pub async fn list_acls(
        &self,
        config: &KafkaClusterConfig,
        filter: &KafkaAclFilter,
    ) -> Result<Vec<KafkaAcl>> {
        self.list_acls_with_cancel(config, filter, Arc::new(AtomicBool::new(false)))
            .await
    }

    /// 读取 ACL 快照并把取消信号传到管理适配器。
    pub async fn list_acls_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        filter: &KafkaAclFilter,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<KafkaAcl>> {
        validate_config(config)?;
        filter.validate().map_err(DomainError::InvalidConfig)?;
        let started = std::time::Instant::now();
        let result = self
            .admin_driver
            .list_acls_with_cancel(config, filter, cancelled)
            .await
            .and_then(validate_acls);
        log_runtime_result(
            "kafka_acl_list",
            config,
            started,
            result.as_ref().ok().map(Vec::len),
            result.as_ref().err(),
        );
        result
    }

    /// 创建一条完整 ACL 规则；应用层和驱动层都要求管理模式已开启。
    pub async fn create_acl(&self, config: &KafkaClusterConfig, acl: &KafkaAcl) -> Result<()> {
        validate_admin_request(config, acl.validate())?;
        let started = std::time::Instant::now();
        let result = self.admin_driver.create_acl(config, acl).await;
        log_acl_result("kafka_acl_create", config, acl, started, &result);
        result
    }

    /// 按完整 ACL 规则删除；空资源名等模糊条件在进入驱动前直接拒绝。
    pub async fn delete_acl(&self, config: &KafkaClusterConfig, acl: &KafkaAcl) -> Result<()> {
        validate_admin_request(config, acl.validate())?;
        let started = std::time::Instant::now();
        let result = self.admin_driver.delete_acl(config, acl).await;
        log_acl_result("kafka_acl_delete", config, acl, started, &result);
        result
    }
}
