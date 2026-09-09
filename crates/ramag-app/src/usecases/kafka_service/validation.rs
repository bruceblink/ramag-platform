use ramag_domain::entities::{KafkaConfigResource, KafkaConfigResourceType};
use ramag_domain::error::{DomainError, Result};

pub(super) fn validate_config_resource(
    resource: KafkaConfigResource,
    expected_type: KafkaConfigResourceType,
    expected_name: &str,
) -> Result<KafkaConfigResource> {
    resource.validate().map_err(DomainError::InvalidConfig)?;
    if resource.resource_type != expected_type || resource.resource_name != expected_name {
        return Err(DomainError::InvalidConfig(
            "Kafka 配置资源与请求不一致".into(),
        ));
    }
    Ok(resource)
}
