//! Docker-backed Schema Registry version and content integration tests.
use super::*;

use ramag_domain::traits::KafkaSchemaRegistryDriver;
use ramag_infra_kafka::SchemaRegistryHttpDriver;

fn docker_schema_registry_endpoint() -> Option<String> {
    match env::var("RAMAG_TEST_SCHEMA_REGISTRY") {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!(
                "Skipping Docker Schema Registry integration test; set RAMAG_TEST_SCHEMA_REGISTRY or run scripts/kafka-test/kafka-test.ps1 test."
            );
            None
        }
    }
}

/// Reads the real Docker Registry REST endpoints for versions and Schema content.
#[test]
fn docker_schema_registry_reads_versions_and_latest_content()
-> Result<(), Box<dyn std::error::Error>> {
    install_tls_crypto_provider();
    let Some(endpoint) = docker_schema_registry_endpoint() else {
        return Ok(());
    };
    let Some(bootstrap) = docker_bootstrap() else {
        return Ok(());
    };
    let mut config = KafkaClusterConfig::new("ramag-docker-schema-registry", vec![bootstrap]);
    config.schema_registry.endpoint = Some(endpoint);
    let driver = SchemaRegistryHttpDriver::new()?;

    let versions = smol::block_on(driver.list_versions(&config, "ramag.integration.orders-value"))?;
    assert_eq!(versions, vec![1, 2]);
    let detail = smol::block_on(driver.get_version(&config, "ramag.integration.orders-value", 2))?;
    assert_eq!(detail.subject, "ramag.integration.orders-value");
    assert_eq!(detail.version, 2);
    assert!(detail.schema.contains("status"));
    assert_eq!(detail.schema_type.as_deref(), Some("JSON"));
    Ok(())
}
