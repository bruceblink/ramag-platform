//! 动态 gRPC 驱动核心。

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytes::Buf;
use http::uri::PathAndQuery;
use prost::Message;
use prost_reflect::{DynamicMessage, MessageDescriptor};
use ramag_domain::entities::{
    ApiCancellation, ApiGrpcDescriptor, ApiProtocol, ApiRequestSpec, ApiResponseSnapshot,
    ApiTlsConfig,
};
use ramag_domain::error::{DomainError, Result as DomainResult};
use ramag_domain::traits::ApiDriver;
use tonic::{Request, Response, Status, client::Grpc, codec::Codec};
use tonic::{codec::DecodeBuf, codec::Decoder, codec::EncodeBuf, codec::Encoder};

#[path = "grpc_descriptor.rs"]
mod descriptor;
#[path = "grpc_stream.rs"]
mod stream;
#[path = "grpc_transport.rs"]
mod transport;

pub(crate) use descriptor::parse_request_messages;
use descriptor::{descriptor_pool, find_method, reflection_symbol_pool};
use transport::{
    build_endpoint, connect_endpoint, request_metadata, status_snapshot, success_snapshot,
};

const MAX_DISCOVERED_SERVICES: usize = 256;

/// 供 API 工作台展示的 gRPC 方法摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcServiceMethod {
    pub name: String,
    pub client_streaming: bool,
    pub server_streaming: bool,
}

/// 供 API 工作台展示的 gRPC Service 目录项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcServiceSummary {
    pub name: String,
    pub methods: Vec<GrpcServiceMethod>,
}

/// 无状态的动态 gRPC 驱动，支持四种方法类型。
#[derive(Clone, Debug, Default)]
pub struct GrpcApiDriver;

impl GrpcApiDriver {
    /// 创建驱动并确保 rustls 已安装可用的加密 Provider。
    pub fn new() -> DomainResult<Self> {
        transport::ensure_tls_provider()?;
        Ok(Self)
    }

    /// 从本地 Descriptor 或 Server Reflection 读取 Service/Method 目录。
    pub async fn discover_services(
        &self,
        endpoint_template: &str,
        descriptor: &ApiGrpcDescriptor,
        tls: &ApiTlsConfig,
        timeout_millis: u64,
        variables: &BTreeMap<String, String>,
        cancelled: ApiCancellation,
    ) -> DomainResult<Vec<GrpcServiceSummary>> {
        descriptor::validate_discovery_timeout(timeout_millis)?;
        transport::ensure_not_cancelled(&cancelled)?;
        let endpoint_text = crate::http::expand_template(
            endpoint_template,
            variables,
            "gRPC Endpoint 模板",
            ramag_domain::entities::MAX_API_GRPC_ENDPOINT_BYTES,
        )?;
        let timeout = Duration::from_millis(timeout_millis);
        let pool = match descriptor {
            ApiGrpcDescriptor::FileDescriptorSet { bytes } => descriptor_pool(bytes)?,
            ApiGrpcDescriptor::Reflection => {
                let endpoint = build_endpoint(endpoint_text, tls, timeout)?;
                let channel = connect_endpoint(endpoint, cancelled.clone()).await?;
                descriptor::reflection_catalog_pool(channel, timeout, cancelled.clone()).await?
            }
        };
        let services = descriptor::service_summaries(&pool)?;
        if services.len() > MAX_DISCOVERED_SERVICES {
            return Err(DomainError::InvalidConfig(
                "gRPC Service 数量超过限制".into(),
            ));
        }
        Ok(services)
    }
}

#[async_trait]
impl ApiDriver for GrpcApiDriver {
    fn protocol(&self) -> ApiProtocol {
        ApiProtocol::Grpc
    }

    /// 校验、发现 Descriptor、构造动态消息并执行一次有界 gRPC 调用。
    async fn execute(
        &self,
        request: &ApiRequestSpec,
        variables: &BTreeMap<String, String>,
        cancelled: ApiCancellation,
    ) -> DomainResult<ApiResponseSnapshot> {
        let spec = match request {
            ApiRequestSpec::Grpc(spec) => spec,
            ApiRequestSpec::Http(_) => {
                return Err(DomainError::InvalidConfig(
                    "gRPC 驱动不能执行 HTTP 请求".into(),
                ));
            }
        };
        spec.validate().map_err(DomainError::InvalidConfig)?;
        transport::ensure_not_cancelled(&cancelled)?;

        let endpoint_text = crate::http::expand_template(
            &spec.endpoint_template,
            variables,
            "gRPC Endpoint 模板",
            ramag_domain::entities::MAX_API_GRPC_ENDPOINT_BYTES,
        )?;
        let timeout = Duration::from_millis(spec.timeout_millis);
        let endpoint = build_endpoint(endpoint_text, &spec.tls, timeout)?;
        let channel = connect_endpoint(endpoint, cancelled.clone()).await?;
        let pool = match &spec.descriptor {
            ApiGrpcDescriptor::Reflection => {
                reflection_symbol_pool(channel.clone(), &spec.service, timeout, cancelled.clone())
                    .await?
            }
            ApiGrpcDescriptor::FileDescriptorSet { bytes } => descriptor_pool(bytes)?,
        };
        let method = find_method(&pool, &spec.service, &spec.method)?;
        let message = crate::http::expand_template(
            &spec.message,
            variables,
            "gRPC 请求消息",
            ramag_domain::entities::MAX_API_REQUEST_BODY_BYTES,
        )?;
        let request_messages =
            parse_request_messages(&message, method.input(), method.is_client_streaming())?;
        let metadata = request_metadata(&spec.metadata, variables)?;
        let path = format!("/{}/{}", spec.service, spec.method);
        let started = Instant::now();
        let client = DynamicGrpcClient::new(channel);
        if method.is_client_streaming() && method.is_server_streaming() {
            let call = client.streaming_with_timeout(
                &path,
                request_messages,
                method.output(),
                metadata,
                timeout,
            );
            return match tokio::select! {
                result = call => result,
                _ = crate::http::wait_until_cancelled(cancelled.clone()) => {
                    return Err(DomainError::Cancelled("gRPC 请求已取消".into()));
                }
            } {
                Ok(response) => stream::collect_response_stream(response, started, cancelled).await,
                Err(status) => status_snapshot(status, started),
            };
        }

        if method.is_server_streaming() {
            let request_message = request_messages
                .into_iter()
                .next()
                .ok_or_else(|| DomainError::InvalidConfig("gRPC 请求消息不能为空".into()))?;
            let call = client.server_streaming_with_timeout(
                &path,
                request_message,
                method.output(),
                metadata,
                timeout,
            );
            return match tokio::select! {
                result = call => result,
                _ = crate::http::wait_until_cancelled(cancelled.clone()) => {
                    return Err(DomainError::Cancelled("gRPC 请求已取消".into()));
                }
            } {
                Ok(response) => stream::collect_response_stream(response, started, cancelled).await,
                Err(status) => status_snapshot(status, started),
            };
        }

        if method.is_client_streaming() {
            let call = client.client_streaming_with_timeout(
                &path,
                request_messages,
                method.output(),
                metadata,
                timeout,
            );
            return match tokio::select! {
                result = call => result,
                _ = crate::http::wait_until_cancelled(cancelled) => {
                    return Err(DomainError::Cancelled("gRPC 请求已取消".into()));
                }
            } {
                Ok(response) => success_snapshot(response, started),
                Err(status) => status_snapshot(status, started),
            };
        }

        let request_message = request_messages
            .into_iter()
            .next()
            .ok_or_else(|| DomainError::InvalidConfig("gRPC 请求消息不能为空".into()))?;
        let call =
            client.unary_with_timeout(&path, request_message, method.output(), metadata, timeout);
        match tokio::select! {
            result = call => result,
            _ = crate::http::wait_until_cancelled(cancelled) => {
                return Err(DomainError::Cancelled("gRPC 请求已取消".into()));
            }
        } {
            Ok(response) => success_snapshot(response, started),
            Err(status) => status_snapshot(status, started),
        }
    }
}

/// 通过运行时 Descriptor 调用 gRPC 方法。
#[derive(Clone)]
pub struct DynamicGrpcClient {
    channel: tonic::transport::Channel,
}

impl DynamicGrpcClient {
    /// 创建已连接或懒连接的动态 gRPC 客户端。
    pub fn new(channel: tonic::transport::Channel) -> Self {
        Self { channel }
    }

    /// 按 Endpoint 建立连接；TLS、超时和证书策略由调用方在 Endpoint 上配置。
    pub async fn connect(
        endpoint: tonic::transport::Endpoint,
    ) -> std::result::Result<Self, tonic::transport::Error> {
        Ok(Self::new(endpoint.connect().await?))
    }

    /// 调用指定的完整 gRPC 方法路径，并使用响应 Descriptor 解码动态消息。
    pub async fn unary(
        &self,
        method_path: &str,
        request_message: DynamicMessage,
        response_descriptor: MessageDescriptor,
        metadata: tonic::metadata::MetadataMap,
    ) -> std::result::Result<Response<DynamicMessage>, Status> {
        self.unary_with_timeout(
            method_path,
            request_message,
            response_descriptor,
            metadata,
            Duration::from_secs(300),
        )
        .await
    }

    async fn unary_with_timeout(
        &self,
        method_path: &str,
        request_message: DynamicMessage,
        response_descriptor: MessageDescriptor,
        metadata: tonic::metadata::MetadataMap,
        timeout: Duration,
    ) -> std::result::Result<Response<DynamicMessage>, Status> {
        let path = parse_method_path(method_path)?;
        let mut request = Request::new(request_message);
        *request.metadata_mut() = metadata;
        request.set_timeout(timeout);

        let mut grpc = self.configured_grpc();
        grpc.ready()
            .await
            .map_err(|error| Status::unknown(format!("gRPC channel is not ready: {error}")))?;
        grpc.unary(request, path, DynamicCodec::new(response_descriptor))
            .await
    }

    async fn client_streaming_with_timeout(
        &self,
        method_path: &str,
        request_messages: Vec<DynamicMessage>,
        response_descriptor: MessageDescriptor,
        metadata: tonic::metadata::MetadataMap,
        timeout: Duration,
    ) -> std::result::Result<Response<DynamicMessage>, Status> {
        let path = parse_method_path(method_path)?;
        let mut request = Request::new(tokio_stream::iter(request_messages));
        *request.metadata_mut() = metadata;
        request.set_timeout(timeout);
        let mut grpc = self.configured_grpc();
        grpc.ready()
            .await
            .map_err(|error| Status::unknown(format!("gRPC channel is not ready: {error}")))?;
        grpc.client_streaming(request, path, DynamicCodec::new(response_descriptor))
            .await
    }

    async fn server_streaming_with_timeout(
        &self,
        method_path: &str,
        request_message: DynamicMessage,
        response_descriptor: MessageDescriptor,
        metadata: tonic::metadata::MetadataMap,
        timeout: Duration,
    ) -> std::result::Result<Response<tonic::codec::Streaming<DynamicMessage>>, Status> {
        let path = parse_method_path(method_path)?;
        let mut request = Request::new(request_message);
        *request.metadata_mut() = metadata;
        request.set_timeout(timeout);
        let mut grpc = self.configured_grpc();
        grpc.ready()
            .await
            .map_err(|error| Status::unknown(format!("gRPC channel is not ready: {error}")))?;
        grpc.server_streaming(request, path, DynamicCodec::new(response_descriptor))
            .await
    }

    async fn streaming_with_timeout(
        &self,
        method_path: &str,
        request_messages: Vec<DynamicMessage>,
        response_descriptor: MessageDescriptor,
        metadata: tonic::metadata::MetadataMap,
        timeout: Duration,
    ) -> std::result::Result<Response<tonic::codec::Streaming<DynamicMessage>>, Status> {
        let path = parse_method_path(method_path)?;
        let mut request = Request::new(tokio_stream::iter(request_messages));
        *request.metadata_mut() = metadata;
        request.set_timeout(timeout);
        let mut grpc = self.configured_grpc();
        grpc.ready()
            .await
            .map_err(|error| Status::unknown(format!("gRPC channel is not ready: {error}")))?;
        grpc.streaming(request, path, DynamicCodec::new(response_descriptor))
            .await
    }

    fn configured_grpc(&self) -> Grpc<tonic::transport::Channel> {
        Grpc::new(self.channel.clone())
            .max_encoding_message_size(ramag_domain::entities::MAX_API_REQUEST_BODY_BYTES)
            .max_decoding_message_size(ramag_domain::entities::MAX_API_RESPONSE_BODY_BYTES)
    }
}

/// 校验 gRPC 方法路径，避免把不完整路径交给 HTTP/2 层后才产生难以定位的错误。
pub(crate) fn parse_method_path(value: &str) -> std::result::Result<PathAndQuery, Status> {
    if !value.starts_with('/') {
        return Err(Status::invalid_argument(
            "gRPC method path must start with '/'",
        ));
    }

    value
        .parse()
        .map_err(|error| Status::invalid_argument(format!("invalid gRPC method path: {error}")))
}

#[derive(Clone, Debug)]
struct DynamicCodec {
    response_descriptor: MessageDescriptor,
}

impl DynamicCodec {
    fn new(response_descriptor: MessageDescriptor) -> Self {
        Self {
            response_descriptor,
        }
    }
}

impl Codec for DynamicCodec {
    type Encode = DynamicMessage;
    type Decode = DynamicMessage;
    type Encoder = DynamicEncoder;
    type Decoder = DynamicDecoder;

    fn encoder(&mut self) -> Self::Encoder {
        DynamicEncoder
    }

    fn decoder(&mut self) -> Self::Decoder {
        DynamicDecoder {
            descriptor: self.response_descriptor.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct DynamicEncoder;

impl Encoder for DynamicEncoder {
    type Item = DynamicMessage;
    type Error = Status;

    fn encode(
        &mut self,
        item: Self::Item,
        dst: &mut EncodeBuf<'_>,
    ) -> std::result::Result<(), Self::Error> {
        item.encode(dst)
            .map_err(|error| Status::internal(format!("encode dynamic gRPC message: {error}")))
    }
}

#[derive(Clone, Debug)]
struct DynamicDecoder {
    descriptor: MessageDescriptor,
}

impl Decoder for DynamicDecoder {
    type Item = DynamicMessage;
    type Error = Status;

    fn decode(
        &mut self,
        src: &mut DecodeBuf<'_>,
    ) -> std::result::Result<Option<Self::Item>, Self::Error> {
        let bytes = src.copy_to_bytes(src.remaining());
        DynamicMessage::decode(self.descriptor.clone(), bytes)
            .map(Some)
            .map_err(|error| Status::internal(format!("decode dynamic gRPC message: {error}")))
    }
}
