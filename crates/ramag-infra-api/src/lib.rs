//! API 测试工具的协议基础设施预研。
//!
//! 当前只提供 gRPC 动态 Unary 调用所需的低层能力。应用层和领域层的正式接口留在
//! `API-001`，避免在技术选型未冻结前把传输客户端类型扩散到其它 crate。

#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

use bytes::Buf;
use http::uri::PathAndQuery;
use prost::Message;
use prost_reflect::{DynamicMessage, MessageDescriptor};
use tonic::{
    Request, Response, Status,
    client::Grpc,
    codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder},
    metadata::MetadataMap,
    transport::{Channel, Endpoint},
};

/// 通过运行时 Descriptor 调用一个 Unary gRPC 方法。
///
/// 调用方负责从 `.proto` 文件或 Server Reflection 得到请求/响应消息描述，并将 JSON
/// 或其它编辑器数据转换成 `DynamicMessage`。本类型只负责路径、Metadata、Protobuf
/// 编解码和响应 Metadata 的保留，不保存请求历史或 UI 状态。
#[derive(Clone)]
pub struct DynamicGrpcClient {
    channel: Channel,
}

impl DynamicGrpcClient {
    /// 创建已连接或懒连接的动态 gRPC 客户端。
    pub fn new(channel: Channel) -> Self {
        Self { channel }
    }

    /// 按 Endpoint 建立连接；TLS、超时和证书策略由调用方在 Endpoint 上配置。
    pub async fn connect(endpoint: Endpoint) -> Result<Self, tonic::transport::Error> {
        Ok(Self::new(endpoint.connect().await?))
    }

    /// 调用指定的完整 gRPC 方法路径，并使用响应 Descriptor 解码动态消息。
    pub async fn unary(
        &self,
        method_path: &str,
        request_message: DynamicMessage,
        response_descriptor: MessageDescriptor,
        metadata: MetadataMap,
    ) -> Result<Response<DynamicMessage>, Status> {
        let path = parse_method_path(method_path)?;
        let mut request = Request::new(request_message);
        *request.metadata_mut() = metadata;

        let mut grpc = Grpc::new(self.channel.clone());
        grpc.ready()
            .await
            .map_err(|error| Status::unknown(format!("gRPC channel is not ready: {error}")))?;
        grpc.unary(request, path, DynamicCodec::new(response_descriptor))
            .await
    }
}

/// 校验 gRPC 方法路径，避免把不完整路径交给 HTTP/2 层后才产生难以定位的错误。
fn parse_method_path(value: &str) -> Result<PathAndQuery, Status> {
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

    fn encode(&mut self, item: Self::Item, dst: &mut EncodeBuf<'_>) -> Result<(), Self::Error> {
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

    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        let bytes = src.copy_to_bytes(src.remaining());
        DynamicMessage::decode(self.descriptor.clone(), bytes)
            .map(Some)
            .map_err(|error| Status::internal(format!("decode dynamic gRPC message: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost_reflect::{DescriptorPool, Value};
    use tokio::net::TcpListener;
    use tokio_stream::wrappers::TcpListenerStream;
    use tonic::transport::Server;
    use tonic::{Request, Response, Status};

    mod proto {
        tonic::include_proto!("api.spike");
    }

    const DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("api_spike");

    #[derive(Default)]
    struct EchoService;

    #[tonic::async_trait]
    impl proto::echo_server::Echo for EchoService {
        async fn unary(
            &self,
            request: Request<proto::EchoRequest>,
        ) -> Result<Response<proto::EchoResponse>, Status> {
            Ok(Response::new(proto::EchoResponse {
                message: format!("echo: {}", request.into_inner().message),
            }))
        }
    }

    #[tokio::test]
    async fn dynamic_descriptor_invokes_unary_grpc_method() {
        let pool = DescriptorPool::decode(DESCRIPTOR_SET).expect("descriptor set is valid");
        let request_descriptor = pool
            .get_message_by_name("api.spike.EchoRequest")
            .expect("request descriptor exists");
        let response_descriptor = pool
            .get_message_by_name("api.spike.EchoResponse")
            .expect("response descriptor exists");

        let mut request_message = DynamicMessage::new(request_descriptor);
        request_message.set_field_by_name("message", Value::String("hello".into()));

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local gRPC test server");
        let address = listener.local_addr().expect("read local gRPC address");
        let server = Server::builder()
            .add_service(proto::echo_server::EchoServer::new(EchoService))
            .serve_with_incoming(TcpListenerStream::new(listener));
        let server_task = tokio::spawn(server);

        let endpoint =
            Endpoint::from_shared(format!("http://{address}")).expect("build gRPC endpoint");
        let client = DynamicGrpcClient::connect(endpoint)
            .await
            .expect("connect local gRPC server");
        let response = client
            .unary(
                "/api.spike.Echo/Unary",
                request_message,
                response_descriptor,
                MetadataMap::new(),
            )
            .await
            .expect("dynamic unary call succeeds")
            .into_inner();

        let message = response
            .get_field_by_name("message")
            .expect("response message exists");
        assert_eq!(message.as_ref().as_str(), Some("echo: hello"));

        server_task.abort();
        let _ = server_task.await;
    }

    #[test]
    fn method_path_requires_leading_slash() {
        let error = parse_method_path("api.spike.Echo/Unary").expect_err("path must be absolute");
        assert_eq!(error.code(), tonic::Code::InvalidArgument);
    }
}
