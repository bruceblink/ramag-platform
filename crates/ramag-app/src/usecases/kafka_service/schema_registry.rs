use std::collections::HashSet;
use std::sync::{Arc, atomic::AtomicBool};

use super::*;

impl KafkaService {
    pub async fn list_schema_subjects(
        &self,
        config: &KafkaClusterConfig,
    ) -> Result<Vec<KafkaSchemaRegistrySubject>> {
        self.list_schema_subjects_with_cancel(config, Arc::new(AtomicBool::new(false)))
            .await
    }

    /// 读取 Schema Registry Subject，并在应用边界再次限制数量、名称和重复项。
    pub async fn list_schema_subjects_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<KafkaSchemaRegistrySubject>> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        let started = std::time::Instant::now();
        let result = self
            .schema_registry_driver
            .list_subjects_with_cancel(config, cancelled)
            .await
            .and_then(validate_schema_subjects);
        tracing::info!(
            operation = "kafka_schema_registry_subjects",
            cluster_id = %config.id,
            elapsed_ms = started.elapsed().as_millis(),
            success = result.is_ok(),
            result_count = result.as_ref().map_or(0, Vec::len),
            "Kafka Schema Registry Subject listing completed"
        );
        result
    }
}

fn validate_schema_subjects(
    subjects: Vec<KafkaSchemaRegistrySubject>,
) -> Result<Vec<KafkaSchemaRegistrySubject>> {
    if subjects.len() > ramag_domain::entities::MAX_KAFKA_SCHEMA_SUBJECTS {
        return Err(DomainError::InvalidConfig(format!(
            "Schema Registry Subject 数量超过 {} 个上限",
            ramag_domain::entities::MAX_KAFKA_SCHEMA_SUBJECTS
        )));
    }
    let mut names = HashSet::with_capacity(subjects.len());
    for subject in &subjects {
        subject.validate().map_err(DomainError::InvalidConfig)?;
        if !names.insert(subject.name.as_str()) {
            return Err(DomainError::InvalidConfig(format!(
                "Schema Registry Subject 名称重复：{}",
                subject.name
            )));
        }
    }
    Ok(subjects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::MAX_KAFKA_SCHEMA_SUBJECTS;

    fn subject(name: impl Into<String>) -> KafkaSchemaRegistrySubject {
        KafkaSchemaRegistrySubject { name: name.into() }
    }

    #[test]
    fn validates_subject_names_and_rejects_duplicates() {
        let valid = validate_schema_subjects(vec![subject("orders-value"), subject("orders-key")])
            .expect("valid subjects");
        assert_eq!(valid.len(), 2);

        let error =
            validate_schema_subjects(vec![subject("orders-value"), subject("orders-value")])
                .expect_err("duplicate subject");
        assert!(error.to_string().contains("名称重复"));
    }

    #[test]
    fn caps_schema_registry_subject_snapshots() {
        let subjects = (0..=MAX_KAFKA_SCHEMA_SUBJECTS)
            .map(|index| subject(format!("subject-{index}")))
            .collect();
        let error = validate_schema_subjects(subjects).expect_err("oversized subject snapshot");
        assert!(error.to_string().contains("超过 2000"));
    }
}
