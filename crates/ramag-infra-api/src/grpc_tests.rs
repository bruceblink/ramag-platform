use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use prost_reflect::{DescriptorPool, DynamicMessage, Value};
use ramag_domain::entities::{
    ApiCancellation, ApiGrpcDescriptor, ApiParameter, ApiRequestSpec, ApiResponseStatus,
    GrpcRequestSpec,
};
use ramag_domain::traits::ApiDriver;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::metadata::MetadataValue;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use crate::grpc::{GrpcApiDriver, parse_method_path};

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
        if request
            .metadata()
            .get("x-request")
            .and_then(|value| value.to_str().ok())
            != Some("sent")
        {
            return Err(Status::invalid_argument("missing request metadata"));
        }
        if request.get_ref().message == "error" {
            let mut status = Status::failed_precondition("requested failure");
            status
                .metadata_mut()
                .insert("x-error", MetadataValue::from_static("present"));
            return Err(status);
        }
        let mut response = Response::new(proto::EchoResponse {
            message: format!("echo: {}", request.get_ref().message),
        });
        response
            .metadata_mut()
            .insert("x-response", MetadataValue::from_static("present"));
        Ok(response)
    }
}

async fn start_server(with_reflection: bool) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local gRPC test server");
    let address = listener.local_addr().expect("read local gRPC address");
    let mut builder = Server::builder();
    let echo = proto::echo_server::EchoServer::new(EchoService);
    let server_task = if with_reflection {
        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(DESCRIPTOR_SET)
            .build_v1()
            .expect("build reflection service");
        tokio::spawn(async move {
            builder
                .add_service(echo)
                .add_service(reflection)
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
                .expect("serve reflection test server");
        })
    } else {
        tokio::spawn(async move {
            builder
                .add_service(echo)
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
                .expect("serve gRPC test server");
        })
    };
    (format!("http://{address}"), server_task)
}

fn cancellation() -> ApiCancellation {
    Arc::new(AtomicBool::new(false))
}

fn request(descriptor: ApiGrpcDescriptor, message: &str) -> ApiRequestSpec {
    let mut spec = GrpcRequestSpec::new("http://127.0.0.1:1", "api.spike.Echo", "Unary");
    spec.descriptor = descriptor;
    spec.message = message.into();
    ApiRequestSpec::Grpc(spec)
}

#[tokio::test]
async fn dynamic_descriptor_driver_invokes_unary_and_keeps_metadata() {
    let pool = DescriptorPool::decode(DESCRIPTOR_SET).expect("descriptor set is valid");
    let request_descriptor = pool
        .get_message_by_name("api.spike.EchoRequest")
        .expect("request descriptor exists");
    let mut request_message = DynamicMessage::new(request_descriptor);
    request_message.set_field_by_name("message", Value::String("hello".into()));

    let (endpoint, server_task) = start_server(false).await;
    let driver = GrpcApiDriver::new().expect("gRPC driver initializes");
    let mut spec = match request(
        ApiGrpcDescriptor::FileDescriptorSet {
            bytes: DESCRIPTOR_SET.to_vec(),
        },
        "{\"message\":\"hello\"}",
    ) {
        ApiRequestSpec::Grpc(spec) => spec,
        ApiRequestSpec::Http(_) => unreachable!(),
    };
    spec.endpoint_template = endpoint;
    spec.metadata = vec![ApiParameter::new("x-request", "sent", false)];

    let snapshot = driver
        .execute(
            &ApiRequestSpec::Grpc(spec),
            &BTreeMap::new(),
            cancellation(),
        )
        .await
        .expect("dynamic unary call succeeds");
    assert_eq!(
        snapshot.status,
        ApiResponseStatus::Grpc { code: "ok".into() }
    );
    assert!(
        snapshot
            .metadata
            .iter()
            .any(|parameter| { parameter.name == "x-response" && parameter.value == "present" })
    );
    assert_eq!(snapshot.body, br#"{"message":"echo: hello"}"#);

    server_task.abort();
    let _ = server_task.await;
}

#[tokio::test]
async fn reflection_driver_discovers_services_and_returns_grpc_status() {
    let (endpoint, server_task) = start_server(true).await;
    let driver = GrpcApiDriver::new().expect("gRPC driver initializes");
    let services = driver
        .discover_services(
            &endpoint,
            &ApiGrpcDescriptor::Reflection,
            &Default::default(),
            5_000,
            &BTreeMap::new(),
            cancellation(),
        )
        .await
        .expect("reflection service discovery succeeds");
    assert!(services.iter().any(|service| {
        service.name == "api.spike.Echo"
            && service.methods.iter().any(|method| method.name == "Unary")
    }));

    let mut error_request = match request(ApiGrpcDescriptor::Reflection, "{\"message\":\"error\"}")
    {
        ApiRequestSpec::Grpc(spec) => spec,
        ApiRequestSpec::Http(_) => unreachable!(),
    };
    error_request.endpoint_template = endpoint;
    error_request
        .metadata
        .push(ApiParameter::new("x-request", "sent", false));
    let snapshot = driver
        .execute(
            &ApiRequestSpec::Grpc(error_request),
            &BTreeMap::new(),
            cancellation(),
        )
        .await
        .expect("gRPC status is represented as a response snapshot");
    assert_eq!(
        snapshot.status,
        ApiResponseStatus::Grpc {
            code: "FailedPrecondition".into()
        }
    );
    assert!(
        snapshot
            .metadata
            .iter()
            .any(|parameter| parameter.name == "x-error" && parameter.value == "present")
    );
    assert_eq!(snapshot.body, b"requested failure");

    server_task.abort();
    let _ = server_task.await;
}

#[test]
fn method_path_requires_leading_slash() {
    let error = parse_method_path("api.spike.Echo/Unary").expect_err("path must be absolute");
    assert_eq!(error.code(), tonic::Code::InvalidArgument);
}
