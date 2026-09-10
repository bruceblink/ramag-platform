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

    /// 读取指定 Subject 的有界版本号列表；不读取 Schema 正文或执行任何 Registry 写操作。
    pub async fn list_schema_versions(
        &self,
        config: &KafkaClusterConfig,
        subject: &str,
    ) -> Result<Vec<i32>> {
        self.list_schema_versions_with_cancel(config, subject, Arc::new(AtomicBool::new(false)))
            .await
    }

    pub async fn list_schema_versions_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        subject: &str,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<i32>> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        validate_schema_subject_name(subject)?;
        let started = std::time::Instant::now();
        let result = self
            .schema_registry_driver
            .list_versions_with_cancel(config, subject, cancelled)
            .await
            .and_then(|versions| validate_schema_versions(subject, versions));
        tracing::info!(
            operation = "kafka_schema_registry_versions",
            cluster_id = %config.id,
            subject = %subject,
            elapsed_ms = started.elapsed().as_millis(),
            success = result.is_ok(),
            result_count = result.as_ref().map_or(0, Vec::len),
            "Kafka Schema Registry version listing completed"
        );
        result
    }

    /// 读取单个 Schema 版本详情；日志只记录定位信息，不记录 Schema 正文。
    pub async fn get_schema_version(
        &self,
        config: &KafkaClusterConfig,
        subject: &str,
        version: i32,
    ) -> Result<KafkaSchemaRegistryVersion> {
        self.get_schema_version_with_cancel(
            config,
            subject,
            version,
            Arc::new(AtomicBool::new(false)),
        )
        .await
    }

    pub async fn get_schema_version_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        subject: &str,
        version: i32,
        cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaSchemaRegistryVersion> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        validate_schema_subject_name(subject)?;
        validate_schema_version_number(version)?;
        let started = std::time::Instant::now();
        let result = self
            .schema_registry_driver
            .get_version_with_cancel(config, subject, version, cancelled)
            .await
            .and_then(|detail| validate_schema_version_detail(subject, version, detail));
        tracing::info!(
            operation = "kafka_schema_registry_version",
            cluster_id = %config.id,
            subject = %subject,
            version,
            elapsed_ms = started.elapsed().as_millis(),
            success = result.is_ok(),
            "Kafka Schema Registry version detail completed"
        );
        result
    }
}

fn validate_schema_subject_name(subject: &str) -> Result<()> {
    KafkaSchemaRegistrySubject {
        name: subject.to_owned(),
    }
    .validate()
    .map_err(DomainError::InvalidConfig)
}

fn validate_schema_version_number(version: i32) -> Result<()> {
    if version < 0 {
        return Err(DomainError::InvalidConfig(
            "Schema Version 不能为负数".into(),
        ));
    }
    Ok(())
}

fn validate_schema_versions(subject: &str, versions: Vec<i32>) -> Result<Vec<i32>> {
    validate_schema_subject_name(subject)?;
    if versions.len() > ramag_domain::entities::MAX_KAFKA_SCHEMA_VERSIONS {
        return Err(DomainError::InvalidConfig(format!(
            "Schema Version 数量超过 {} 个上限",
            ramag_domain::entities::MAX_KAFKA_SCHEMA_VERSIONS
        )));
    }
    let mut unique = HashSet::with_capacity(versions.len());
    for version in &versions {
        validate_schema_version_number(*version)?;
        if !unique.insert(*version) {
            return Err(DomainError::InvalidConfig(format!(
                "Schema Version 重复：{version}"
            )));
        }
    }
    Ok(versions)
}

fn validate_schema_version_detail(
    subject: &str,
    version: i32,
    detail: KafkaSchemaRegistryVersion,
) -> Result<KafkaSchemaRegistryVersion> {
    validate_schema_subject_name(subject)?;
    validate_schema_version_number(version)?;
    detail.validate().map_err(DomainError::InvalidConfig)?;
    if detail.subject != subject {
        return Err(DomainError::InvalidConfig(
            "Schema Registry 返回的 Subject 与请求不一致".into(),
        ));
    }
    if detail.version != version {
        return Err(DomainError::InvalidConfig(
            "Schema Registry 返回的 Version 与请求不一致".into(),
        ));
    }
    Ok(detail)
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

    #[test]
    fn validates_versions_and_detail_context() {
        assert_eq!(
            validate_schema_versions("orders-value", vec![1, 3]).unwrap(),
            vec![1, 3]
        );
        assert!(validate_schema_versions("orders-value", vec![1, 1]).is_err());
        assert!(validate_schema_versions("orders-value", vec![-1]).is_err());
        let oversized = (0..=ramag_domain::entities::MAX_KAFKA_SCHEMA_VERSIONS as i32).collect();
        assert!(validate_schema_versions("orders-value", oversized).is_err());

        let detail = KafkaSchemaRegistryVersion {
            subject: "orders-value".into(),
            version: 3,
            id: 7,
            schema_type: Some("AVRO".into()),
            schema: "{}".into(),
        };
        assert!(validate_schema_version_detail("orders-value", 3, detail).is_ok());
        assert!(
            validate_schema_version_detail(
                "other-value",
                3,
                KafkaSchemaRegistryVersion {
                    subject: "orders-value".into(),
                    version: 3,
                    id: 7,
                    schema_type: None,
                    schema: "{}".into(),
                }
            )
            .is_err()
        );
    }
}
