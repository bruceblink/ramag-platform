//! Docker Registry v2 查询用例；凭据只通过调用边界传给基础设施适配器。

use std::sync::Arc;

use ramag_domain::entities::{
    ContainerRegistryCredential, ContainerRegistryInfo, ContainerRegistryManifest,
    ContainerRegistryProfile, ContainerRegistryRepository, ContainerRegistryTag,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::ContainerRegistryDriver;

pub struct ContainerRegistryService {
    driver: Arc<dyn ContainerRegistryDriver>,
}

impl ContainerRegistryService {
    pub fn new(driver: Arc<dyn ContainerRegistryDriver>) -> Self {
        Self { driver }
    }

    pub async fn test_connection(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
    ) -> Result<ContainerRegistryInfo> {
        self.validate_inputs(profile, credential)?;
        self.driver.test_connection(profile, credential).await
    }

    pub async fn list_repositories(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
    ) -> Result<Vec<ContainerRegistryRepository>> {
        self.validate_inputs(profile, credential)?;
        self.driver.list_repositories(profile, credential).await
    }

    pub async fn list_tags(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
        repository: &str,
    ) -> Result<Vec<ContainerRegistryTag>> {
        self.validate_inputs(profile, credential)?;
        self.driver.list_tags(profile, credential, repository).await
    }

    pub async fn get_manifest(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
        repository: &str,
        reference: &str,
    ) -> Result<ContainerRegistryManifest> {
        self.validate_inputs(profile, credential)?;
        self.driver
            .get_manifest(profile, credential, repository, reference)
            .await
    }

    fn validate_inputs(
        &self,
        profile: &ContainerRegistryProfile,
        credential: Option<&ContainerRegistryCredential>,
    ) -> Result<()> {
        profile.validate().map_err(DomainError::InvalidConfig)?;
        if let Some(credential) = credential {
            credential.validate().map_err(DomainError::InvalidConfig)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use ramag_domain::entities::ContainerRegistryId;

    struct MockRegistryDriver;

    #[async_trait]
    impl ContainerRegistryDriver for MockRegistryDriver {
        async fn test_connection(
            &self,
            profile: &ContainerRegistryProfile,
            _credential: Option<&ContainerRegistryCredential>,
        ) -> Result<ContainerRegistryInfo> {
            Ok(ContainerRegistryInfo {
                registry_id: profile.id.clone(),
                api_version: Some("v2".into()),
                server_version: None,
                authenticated: false,
            })
        }

        async fn list_repositories(
            &self,
            profile: &ContainerRegistryProfile,
            _credential: Option<&ContainerRegistryCredential>,
        ) -> Result<Vec<ContainerRegistryRepository>> {
            assert_eq!(profile.endpoint, "https://registry.example.test");
            Ok(vec![ContainerRegistryRepository {
                name: "library/app".into(),
            }])
        }

        async fn list_tags(
            &self,
            _profile: &ContainerRegistryProfile,
            _credential: Option<&ContainerRegistryCredential>,
            repository: &str,
        ) -> Result<Vec<ContainerRegistryTag>> {
            Ok(vec![ContainerRegistryTag {
                repository: repository.into(),
                name: "latest".into(),
            }])
        }

        async fn get_manifest(
            &self,
            _profile: &ContainerRegistryProfile,
            _credential: Option<&ContainerRegistryCredential>,
            repository: &str,
            reference: &str,
        ) -> Result<ContainerRegistryManifest> {
            Ok(ContainerRegistryManifest {
                repository: repository.into(),
                reference: reference.into(),
                digest: "sha256:test".into(),
                media_type: None,
                size_bytes: None,
            })
        }
    }

    #[test]
    fn validates_profile_before_delegating_queries() {
        let service = ContainerRegistryService::new(Arc::new(MockRegistryDriver));
        let profile = ContainerRegistryProfile::new("test", "http://registry.example.test");
        let result = smol::block_on(service.list_repositories(&profile, None));
        assert!(matches!(result, Err(DomainError::InvalidConfig(_))));
    }

    #[test]
    fn delegates_registry_queries_without_exposing_driver_types() {
        let service = ContainerRegistryService::new(Arc::new(MockRegistryDriver));
        let profile = ContainerRegistryProfile {
            id: ContainerRegistryId::default(),
            name: "test".into(),
            endpoint: "https://registry.example.test".into(),
            credential_ref: None,
            allow_insecure_http: false,
        };
        let repositories =
            smol::block_on(service.list_repositories(&profile, None)).expect("repositories");
        assert_eq!(repositories[0].name, "library/app");
        let tags = smol::block_on(service.list_tags(&profile, None, "library/app")).expect("tags");
        assert_eq!(tags[0].name, "latest");
        let manifest =
            smol::block_on(service.get_manifest(&profile, None, "library/app", "latest"))
                .expect("manifest");
        assert_eq!(manifest.digest, "sha256:test");
    }
}
