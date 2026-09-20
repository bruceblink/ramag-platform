use std::fs;
use std::net::SocketAddr;
use std::time::Duration;

use tonic::metadata::{BinaryMetadataValue, MetadataValue};
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};
use tonic::{Request, Response, Status};

mod proto {
    tonic::include_proto!("api.docker");
}

const DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("api_docker");

#[derive(Default)]
struct EchoService;

type EchoResponseStream =
    tokio_stream::Iter<std::vec::IntoIter<Result<proto::EchoResponse, Status>>>;

fn check_metadata<T>(request: &Request<T>) -> Result<(), Status> {
    if request
        .metadata()
        .get("x-request")
        .and_then(|value| value.to_str().ok())
        != Some("docker")
    {
        return Err(Status::invalid_argument("missing x-request metadata"));
    }
    Ok(())
}

fn response_stream(messages: Vec<String>) -> EchoResponseStream {
    tokio_stream::iter(
        messages
            .into_iter()
            .map(|message| Ok(proto::EchoResponse { message }))
            .collect::<Vec<_>>(),
    )
}

#[tonic::async_trait]
impl proto::echo_server::Echo for EchoService {
    async fn unary(
        &self,
        request: Request<proto::EchoRequest>,
    ) -> Result<Response<proto::EchoResponse>, Status> {
        check_metadata(&request)?;

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

    type ServerStreamStream = EchoResponseStream;

    async fn server_stream(
        &self,
        request: Request<proto::EchoRequest>,
    ) -> Result<Response<Self::ServerStreamStream>, Status> {
        check_metadata(&request)?;
        let message = request.into_inner().message;
        let mut response = Response::new(response_stream(vec![
            format!("docker server: {message}:one"),
            format!("docker server: {message}:two"),
        ]));
        response
            .metadata_mut()
            .insert("x-response", MetadataValue::from_static("docker-stream"));
        Ok(response)
    }

    async fn client_stream(
        &self,
        request: Request<tonic::Streaming<proto::EchoRequest>>,
    ) -> Result<Response<proto::EchoResponse>, Status> {
        check_metadata(&request)?;
        let mut stream = request.into_inner();
        let mut messages = Vec::new();
        while let Some(message) = stream.message().await? {
            messages.push(message.message);
        }
        let mut response = Response::new(proto::EchoResponse {
            message: format!("docker client: {}", messages.join(",")),
        });
        response
            .metadata_mut()
            .insert("x-response", MetadataValue::from_static("docker-stream"));
        Ok(response)
    }

    type BidiStreamStream = EchoResponseStream;

    async fn bidi_stream(
        &self,
        request: Request<tonic::Streaming<proto::EchoRequest>>,
    ) -> Result<Response<Self::BidiStreamStream>, Status> {
        check_metadata(&request)?;
        let mut stream = request.into_inner();
        let mut responses = Vec::new();
        while let Some(message) = stream.message().await? {
            responses.push(format!("docker bidi: {}", message.message));
        }
        Ok(Response::new(response_stream(responses)))
    }
}

async fn serve(
    address: SocketAddr,
    tls: Option<ServerTlsConfig>,
) -> Result<(), Box<dyn std::error::Error>> {
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(DESCRIPTOR_SET)
        .build_v1()?;
    let builder = Server::builder();
    let mut builder = if let Some(tls) = tls {
        builder.tls_config(tls)?
    } else {
        builder
    };
    builder
        .add_service(proto::echo_server::EchoServer::new(EchoService))
        .add_service(reflection)
        .serve(address)
        .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plain_address: SocketAddr = "0.0.0.0:50051".parse()?;
    let tls_address: SocketAddr = "0.0.0.0:50052".parse()?;
    let cert_path =
        std::env::var("API_TLS_CERT").unwrap_or_else(|_| "/app/tls/server.cert.pem".into());
    let key_path =
        std::env::var("API_TLS_KEY").unwrap_or_else(|_| "/app/tls/server.key.pem".into());
    let ca_path = std::env::var("API_TLS_CA").unwrap_or_else(|_| "/app/tls/ca.cert.pem".into());
    if [cert_path.as_str(), key_path.as_str(), ca_path.as_str()]
        .iter()
        .all(|path| std::path::Path::new(path).is_file())
    {
        let identity = Identity::from_pem(fs::read(cert_path)?, fs::read(key_path)?);
        let tls = ServerTlsConfig::new()
            .identity(identity)
            .client_ca_root(Certificate::from_pem(fs::read(ca_path)?));
        tokio::try_join!(serve(plain_address, None), serve(tls_address, Some(tls)))?;
    } else {
        serve(plain_address, None).await?;
    }
    Ok(())
}
