//! 领域实体与接口，不依赖 UI 和基础设施实现。

pub mod entities;
pub mod error;
pub mod json_path;
pub mod traits;

pub use entities::{
    ApiAssertion, ApiAuth, ApiBody, ApiCancellation, ApiCollection, ApiCollectionId,
    ApiEnvironment, ApiEnvironmentId, ApiGrpcDescriptor, ApiGrpcDiscoverySpec,
    ApiGrpcMethodSummary, ApiGrpcServiceSummary, ApiImportBundle, ApiImportFormat,
    ApiImportSummary, ApiKeyLocation, ApiOAuth2Config, ApiParameter, ApiProtocol, ApiProxyConfig,
    ApiRequestId, ApiRequestRecord, ApiRequestSpec, ApiResponseSnapshot, ApiResponseSnapshotParts,
    ApiResponseStatus, ApiTlsConfig, ApiTlsVerify, ApiWorkspace, ApiWorkspaceId,
    CollaborationArtifact, CollaborationArtifactKind, CollaborationAuditAction,
    CollaborationAuditEvent, CollaborationDataClass, CollaborationMutationError,
    CollaborationRemotePackage, CollaborationRemoteReceipt, CollaborationShare,
    CollaborationShareId, CollaborationShareState, CollaborationSyncPolicy,
    CollaborationValidationError, ContainerImageOperationKind, ContainerImageOperationPreview,
    ContainerImageOperationRequest, ContainerImageOperationResult, ContainerRegistryCredential,
    ContainerRegistryId, ContainerRegistryInfo, ContainerRegistryManifest,
    ContainerRegistryProfile, ContainerRegistryRepository, ContainerRegistryTag, GrpcRequestSpec,
    HttpRequestSpec, MAX_API_GRPC_STREAM_MESSAGES, MAX_API_IMPORT_BYTES, MAX_API_IMPORT_DEPTH,
    MAX_API_OAUTH2_SCOPE_BYTES, MAX_API_OAUTH2_URL_BYTES, MAX_COLLABORATION_ACTOR_BYTES,
    MAX_COLLABORATION_ARTIFACT_BYTES, MAX_COLLABORATION_ARTIFACT_TITLE_BYTES,
    MAX_COLLABORATION_ARTIFACTS, MAX_COLLABORATION_AUDIT_EVENTS, MAX_COLLABORATION_EXPORT_BYTES,
    MAX_COLLABORATION_PAYLOAD_BYTES, MAX_COLLABORATION_REMOTE_ID_BYTES, MAX_COLLABORATION_SHARES,
    MAX_COLLABORATION_TITLE_BYTES, MAX_CONTAINER_IMAGE_REFERENCE_BYTES,
    MAX_CONTAINER_REGISTRY_DIGEST_BYTES, MAX_CONTAINER_REGISTRY_REPOSITORIES,
    MAX_CONTAINER_REGISTRY_REPOSITORY_BYTES, MAX_CONTAINER_REGISTRY_TAG_BYTES,
    MAX_CONTAINER_REGISTRY_TAGS, bound_response_body, import_api_json,
};
pub use error::{
    ContainerError, ContainerErrorCategory, DomainError, KafkaError, KafkaErrorCategory, MqttError,
    MqttErrorCategory, Result,
};
pub use json_path::{
    JsonPathError, JsonPathSegment, MAX_JSON_INPUT_BYTES, MAX_JSON_PATH_BYTES,
    MAX_JSON_PATH_MATCHES, MAX_JSON_PATH_OUTPUT_BYTES, extract_json_path, query_json_path,
    tokenize_json_path,
};
pub use traits::{
    ApiDriver, ContainerDriver, ContainerLogSink, ContainerLogSinkResult,
    ContainerOperationCancellation, ContainerRegistryDriver, Driver, KafkaAdminDriver,
    KafkaBrokerMetricsDriver, KafkaMessageTailSink, KafkaMessageTailSinkResult,
    KafkaProducerDriver, KafkaTransport, KvDriver, MAX_PLUGIN_ENTRIES,
    MAX_PLUGIN_ENTRY_PAYLOAD_BYTES, MAX_PLUGIN_SETTING_LIST_ITEMS, MAX_PLUGIN_SETTING_VALUE_BYTES,
    MosquittoDynamicSecurityDriver, MosquittoStaticConfigDriver, MqttDriver, MqttTransport,
    PluginApiVersion, PluginCapability, PluginDescriptor, PluginEntryDataKind, PluginEntryDataSpec,
    PluginEntryDescriptor, PluginId, PluginRegistrationError, PluginSettingDefinition,
    PluginSettingKind, PluginSettingValue, SshDriver, Storage, Tool, ToolMeta,
};
