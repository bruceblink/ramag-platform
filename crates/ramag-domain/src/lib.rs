//! 领域实体与接口，不依赖 UI 和基础设施实现。

pub mod entities;
pub mod error;
pub mod traits;

pub use entities::{
    ApiAssertion, ApiAuth, ApiBody, ApiCancellation, ApiCollection, ApiCollectionId,
    ApiEnvironment, ApiEnvironmentId, ApiGrpcDescriptor, ApiKeyLocation, ApiParameter, ApiProtocol,
    ApiRequestId, ApiRequestRecord, ApiRequestSpec, ApiResponseSnapshot, ApiResponseSnapshotParts,
    ApiResponseStatus, ApiTlsConfig, ApiTlsVerify, ApiWorkspace, ApiWorkspaceId,
    ContainerImageOperationKind, ContainerImageOperationPreview, ContainerImageOperationRequest,
    ContainerImageOperationResult, ContainerRegistryCredential, ContainerRegistryId,
    ContainerRegistryInfo, ContainerRegistryManifest, ContainerRegistryProfile,
    ContainerRegistryRepository, ContainerRegistryTag, GrpcRequestSpec, HttpRequestSpec,
    MAX_CONTAINER_IMAGE_REFERENCE_BYTES, MAX_CONTAINER_REGISTRY_DIGEST_BYTES,
    MAX_CONTAINER_REGISTRY_REPOSITORIES, MAX_CONTAINER_REGISTRY_REPOSITORY_BYTES,
    MAX_CONTAINER_REGISTRY_TAG_BYTES, MAX_CONTAINER_REGISTRY_TAGS, bound_response_body,
};
pub use error::{
    ContainerError, ContainerErrorCategory, DomainError, KafkaError, KafkaErrorCategory, MqttError,
    MqttErrorCategory, Result,
};
pub use traits::{
    ApiDriver, ContainerDriver, ContainerOperationCancellation, ContainerRegistryDriver, Driver,
    KafkaAdminDriver, KafkaBrokerMetricsDriver, KafkaMessageTailSink, KafkaMessageTailSinkResult,
    KafkaProducerDriver, KafkaTransport, KvDriver, MosquittoDynamicSecurityDriver,
    MosquittoStaticConfigDriver, MqttDriver, MqttTransport, PluginApiVersion, PluginCapability,
    PluginDescriptor, PluginId, PluginRegistrationError, PluginSettingDefinition,
    PluginSettingKind, PluginSettingValue, SshDriver, Storage, Tool, ToolMeta,
};
