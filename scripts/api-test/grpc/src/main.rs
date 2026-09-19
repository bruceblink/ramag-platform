use std::net::SocketAddr;
use std::time::Duration;

use tonic::metadata::{BinaryMetadataValue, MetadataValue};
use tonic::transport::Server;
use tonic::{Request, Response, Status};

mod proto {
    tonic::include_proto!("api.docker");
}

const DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("api_docker");

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
            != Some("docker")
        {
            return Err(Status::invalid_argument("missing x-request metadata"));
        }

        let message = request.into_inner().message;
        if message == "error" {
            let mut status = Status::failed_precondition("docker requested failure");
            status
                .metadata_mut()
                .insert("x-error", MetadataValue::from_static("docker"));
            return Err(status);
        }
        if let Some(milliseconds) = message.strip_prefix("delay:") {
            let milliseconds = milliseconds
                .parse::<u64>()
                .map_err(|_| Status::invalid_argument("invalid delay"))?
                .min(5_000);
            tokio::time::sleep(Duration::from_millis(milliseconds)).await;
        }

        let mut response = Response::new(proto::EchoResponse {
            message: format!("docker echo: {message}"),
        });
        response
            .metadata_mut()
            .insert("x-response", MetadataValue::from_static("docker-grpc"));
        response.metadata_mut().insert_bin(
            "x-response-bin",
            BinaryMetadataValue::from_bytes(b"docker-binary"),
        );
        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address: SocketAddr = "0.0.0.0:50051".parse()?;
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(DESCRIPTOR_SET)
        .build_v1()?;
    Server::builder()
        .add_service(proto::echo_server::EchoServer::new(EchoService))
        .add_service(reflection)
        .serve(address)
        .await?;
    Ok(())
}
