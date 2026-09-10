//! ksqlDB application-boundary fixtures and read-only validation tests.
use super::*;
use ramag_domain::entities::{KafkaKsqlDbQuery, KafkaKsqlDbQueryResult};
use ramag_domain::traits::KafkaKsqlDbDriver;

struct RecordingKsqlDbDriver {
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl KafkaKsqlDbDriver for RecordingKsqlDbDriver {
    async fn execute_query(
        &self,
        _config: &KafkaClusterConfig,
        query: &KafkaKsqlDbQuery,
    ) -> Result<KafkaKsqlDbQueryResult> {
        self.calls.lock().unwrap().push(query.sql.clone());
        Ok(KafkaKsqlDbQueryResult {
            columns: vec!["ID".into()],
            rows: vec![vec!["1".into()]],
            truncated: false,
            query_id: Some("query-1".into()),
            final_message: Some("done".into()),
        })
    }
}

#[test]
fn ksqldb_service_validates_read_only_query_and_result() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let service = KafkaService::new(Arc::new(NoopKafkaDriver), Arc::new(NoopStorage))
        .with_ksqldb_driver(Arc::new(RecordingKsqlDbDriver {
            calls: calls.clone(),
        }));
    let config = KafkaClusterConfig::new("ksql", vec!["localhost:9092".into()]);
    let query = KafkaKsqlDbQuery::new("SELECT * FROM stream;");
    let result = smol::block_on(service.execute_ksqldb_query(&config, &query))
        .expect("valid ksqlDB query should forward");
    assert_eq!(result.rows, vec![vec![String::from("1")]]);
    assert_eq!(calls.lock().unwrap().as_slice(), &["SELECT * FROM stream;"]);

    let invalid = KafkaKsqlDbQuery::new("INSERT INTO sink SELECT * FROM stream;");
    assert!(matches!(
        smol::block_on(service.execute_ksqldb_query(&config, &invalid)),
        Err(DomainError::InvalidConfig(message)) if message.contains("SELECT")
    ));
    assert_eq!(calls.lock().unwrap().len(), 1);
}
