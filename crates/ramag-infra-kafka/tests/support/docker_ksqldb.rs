//! Docker-backed ksqlDB HTTP integration tests.
use super::*;

use ramag_domain::entities::{KafkaKsqlDbQuery, MAX_KAFKA_KSQLDB_RESULT_ROWS};
use ramag_domain::error::KafkaErrorCategory;
use ramag_domain::traits::KafkaKsqlDbDriver;
use ramag_infra_kafka::KsqlDbHttpDriver;

fn docker_ksqldb_endpoint() -> Option<String> {
    match env::var("RAMAG_TEST_KSQLDB") {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!(
                "Skipping Docker ksqlDB integration test; set RAMAG_TEST_KSQLDB or run scripts/kafka-test/kafka-test.ps1 test."
            );
            None
        }
    }
}

/// Executes the real ksqlDB REST `/query` endpoint and verifies bounded rows.
#[test]
fn docker_kafka_executes_ksqldb_read_only_query() -> Result<(), Box<dyn std::error::Error>> {
    install_tls_crypto_provider();
    let Some(endpoint) = docker_ksqldb_endpoint() else {
        return Ok(());
    };
    let Some(bootstrap) = docker_bootstrap() else {
        return Ok(());
    };
    let mut config = KafkaClusterConfig::new("ramag-docker-ksqldb", vec![bootstrap]);
    config.ksqldb.endpoint = Some(endpoint);
    let driver = KsqlDbHttpDriver::new()?;
    let query =
        KafkaKsqlDbQuery::new("SELECT * FROM RAMAG_INTEGRATION_STREAM EMIT CHANGES LIMIT 20;");

    let result = smol::block_on(driver.execute_query(&config, &query))?;
    assert_eq!(result.columns, vec!["EVENT", "SEQUENCE", "SOURCE"]);
    assert!(
        !result.rows.is_empty(),
        "ksqlDB should return seeded stream rows"
    );
    assert!(result.rows.len() <= 20);
    assert!(
        result
            .rows
            .iter()
            .any(|row| row.iter().any(|value| value == "docker-fixture")),
        "ksqlDB should return rows from the seeded Docker fixture"
    );
    Ok(())
}

/// Verifies ksqlDB HTTP error mapping and the infrastructure row budget.
#[test]
fn docker_kafka_bounds_ksqldb_errors_and_rows() -> Result<(), Box<dyn std::error::Error>> {
    install_tls_crypto_provider();
    let Some(endpoint) = docker_ksqldb_endpoint() else {
        return Ok(());
    };
    let Some(bootstrap) = docker_bootstrap() else {
        return Ok(());
    };
    let mut config = KafkaClusterConfig::new("ramag-docker-ksqldb-bounds", vec![bootstrap]);
    config.ksqldb.endpoint = Some(format!("{endpoint}/missing"));
    let driver = KsqlDbHttpDriver::new()?;
    let query =
        KafkaKsqlDbQuery::new("SELECT * FROM RAMAG_INTEGRATION_STREAM EMIT CHANGES LIMIT 1;");
    assert!(matches!(
        smol::block_on(driver.execute_query(&config, &query)),
        Err(DomainError::Kafka(error)) if error.category == KafkaErrorCategory::NotFound
    ));

    config.ksqldb.endpoint = Some(endpoint);
    let query = KafkaKsqlDbQuery::new(format!(
        "SELECT * FROM RAMAG_INTEGRATION_STREAM EMIT CHANGES LIMIT {};",
        MAX_KAFKA_KSQLDB_RESULT_ROWS + 1
    ));
    let result = smol::block_on(driver.execute_query(&config, &query))?;
    assert_eq!(result.rows.len(), MAX_KAFKA_KSQLDB_RESULT_ROWS);
    assert!(result.truncated);
    Ok(())
}
