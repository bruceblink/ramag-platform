//! Docker Registry v2 的查询接口；客户端和 HTTP 细节留在基础设施层。

use async_trait::async_trait;

use crate::entities::{
    ContainerRegistryCredential, ContainerRegistryInfo, ContainerRegistryProfile,
    ContainerRegistryRepository, ContainerRegistryTag,
};
use crate::error::Result;

#[async_trait]
pub trait ContainerRegistryDriver: Send + Sync {
    async fn test_connection(
        &self,
        _profile: &ContainerRegistryProfile,
        _credential: Option<&ContainerRegistryCredential>,
    ) -> Result<ContainerRegistryInfo> {
        Err(crate::error::DomainError::NotImplemented(
            "container_registry_test_connection".into(),
        ))
    }

    async fn list_repositories(
        &self,
        _profile: &ContainerRegistryProfile,
        _credential: Option<&ContainerRegistryCredential>,
    ) -> Result<Vec<ContainerRegistryRepository>> {
        Err(crate::error::DomainError::NotImplemented(
            "container_registry_list_repositories".into(),
        ))
    }

    async fn list_tags(
        &self,
        _profile: &ContainerRegistryProfile,
        _credential: Option<&ContainerRegistryCredential>,
        _repository: &str,
    ) -> Result<Vec<ContainerRegistryTag>> {
        Err(crate::error::DomainError::NotImplemented(
            "container_registry_list_tags".into(),
        ))
    }
}
