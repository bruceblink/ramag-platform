use super::*;

pub(super) fn log_storage_result<T>(operation: &'static str, result: &Result<T>) {
    if let Err(error) = result {
        tracing::warn!(operation, error = %error, "Kafka local storage operation failed");
    }
}

pub(super) fn log_runtime_result(
    operation: &'static str,
    config: &KafkaClusterConfig,
    started: std::time::Instant,
    result_count: Option<usize>,
    error: Option<&ramag_domain::error::DomainError>,
) {
    tracing::info!(
        operation,
        cluster_id = %config.id,
        elapsed_ms = started.elapsed().as_millis(),
        result_count,
        success = error.is_none(),
        "Kafka read operation completed"
    );
    if let Some(error) = error {
        tracing::warn!(operation, cluster_id = %config.id, error = %error, "Kafka read operation failed");
    }
}

pub(super) fn log_message_result(
    operation: &'static str,
    config: &KafkaClusterConfig,
    topic: &str,
    started: std::time::Instant,
    result: &Result<KafkaMessagePage>,
) {
    tracing::info!(
        operation,
        cluster_id = %config.id,
        topic,
        elapsed_ms = started.elapsed().as_millis(),
        result_count = result.as_ref().map_or(0, |page| page.records.len()),
        scanned_records = result.as_ref().map_or(0, |page| page.scanned_records),
        success = result.is_ok(),
        "Kafka message operation completed"
    );
    if let Err(error) = result {
        tracing::warn!(operation, cluster_id = %config.id, topic, error = %error, "Kafka message operation failed");
    }
}

pub(super) fn log_message_produce_result(
    operation: &'static str,
    config: &KafkaClusterConfig,
    request: &KafkaMessageProduceRequest,
    started: std::time::Instant,
    result: &Result<KafkaMessageProduceResult>,
) {
    tracing::info!(
        operation,
        cluster_id = %config.id,
        topic = %request.topic,
        requested_partition = request.partition,
        actual_partition = result.as_ref().ok().map(|value| value.partition),
        offset = result.as_ref().ok().map(|value| value.offset),
        elapsed_ms = started.elapsed().as_millis(),
        success = result.is_ok(),
        "Kafka message production completed"
    );
    if let Err(error) = result {
        tracing::warn!(
            operation,
            cluster_id = %config.id,
            topic = %request.topic,
            requested_partition = request.partition,
            error = %error,
            "Kafka message production failed"
        );
    }
}

pub(super) fn log_admin_result(
    operation: &'static str,
    config: &KafkaClusterConfig,
    topic: &str,
    started: std::time::Instant,
    result: &Result<()>,
) {
    tracing::info!(
        operation,
        cluster_id = %config.id,
        topic,
        elapsed_ms = started.elapsed().as_millis(),
        success = result.is_ok(),
        "Kafka 管理操作完成"
    );
    if let Err(error) = result {
        tracing::warn!(operation, cluster_id = %config.id, topic, error = %error, "Kafka 管理操作失败");
    }
}

pub(super) fn log_config_read_result(
    operation: &'static str,
    config: &KafkaClusterConfig,
    resource_type: KafkaConfigResourceType,
    resource_name: &str,
    started: std::time::Instant,
    result: &Result<KafkaConfigResource>,
) {
    tracing::info!(
        operation,
        cluster_id = %config.id,
        resource_type = resource_type.label(),
        resource_name,
        elapsed_ms = started.elapsed().as_millis(),
        entry_count = result.as_ref().map_or(0, |resource| resource.entries.len()),
        success = result.is_ok(),
        "Kafka 配置读取完成"
    );
    if let Err(error) = result {
        tracing::warn!(
            operation,
            cluster_id = %config.id,
            resource_type = resource_type.label(),
            resource_name,
            error = %error,
            "Kafka 配置读取失败"
        );
    }
}

pub(super) fn log_config_update_result(
    operation: &'static str,
    config: &KafkaClusterConfig,
    request: &KafkaConfigUpdateRequest,
    started: std::time::Instant,
    result: &Result<()>,
) {
    tracing::info!(
        operation,
        cluster_id = %config.id,
        resource_type = request.resource_type.label(),
        resource_name = %request.resource_name,
        config_key = %request.key,
        config_operation = request.operation.label(),
        elapsed_ms = started.elapsed().as_millis(),
        success = result.is_ok(),
        "Kafka 配置修改完成"
    );
    if let Err(error) = result {
        tracing::warn!(
            operation,
            cluster_id = %config.id,
            resource_type = request.resource_type.label(),
            resource_name = %request.resource_name,
            config_key = %request.key,
            config_operation = request.operation.label(),
            error = %error,
            "Kafka 配置修改失败"
        );
    }
}

pub(super) fn log_acl_result(
    operation: &'static str,
    config: &KafkaClusterConfig,
    acl: &KafkaAcl,
    started: std::time::Instant,
    result: &Result<()>,
) {
    tracing::info!(
        operation,
        cluster_id = %config.id,
        principal = %acl.principal,
        host = %acl.host,
        resource_type = acl.resource_type.label(),
        resource_name = %acl.resource_name,
        pattern_type = acl.pattern_type.label(),
        acl_operation = acl.operation.label(),
        permission = acl.permission.label(),
        elapsed_ms = started.elapsed().as_millis(),
        success = result.is_ok(),
        "Kafka ACL 管理操作完成"
    );
    if let Err(error) = result {
        tracing::warn!(
            operation,
            cluster_id = %config.id,
            principal = %acl.principal,
            host = %acl.host,
            resource_type = acl.resource_type.label(),
            resource_name = %acl.resource_name,
            pattern_type = acl.pattern_type.label(),
            acl_operation = acl.operation.label(),
            permission = acl.permission.label(),
            error = %error,
            "Kafka ACL 管理操作失败"
        );
    }
}
