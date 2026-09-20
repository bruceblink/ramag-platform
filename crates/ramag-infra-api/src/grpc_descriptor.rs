//! gRPC Descriptor、Reflection 和动态消息适配。

use std::collections::BTreeMap;
use std::time::Duration;

use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor, MethodDescriptor};
use ramag_domain::entities::{
    ApiCancellation, MAX_API_GRPC_STREAM_MESSAGES, MAX_API_TIMEOUT_MILLIS,
};
use ramag_domain::error::{DomainError, Result as DomainResult};
use tonic::{Request, Status};
use tonic_reflection::pb::v1::{
    FileDescriptorResponse, ServerReflectionRequest, ServerReflectionResponse,
    server_reflection_client::ServerReflectionClient, server_reflection_request,
    server_reflection_response,
};

use super::{GrpcServiceMethod, GrpcServiceSummary, MAX_DISCOVERED_SERVICES};

pub(super) fn descriptor_pool(bytes: &[u8]) -> DomainResult<DescriptorPool> {
    DescriptorPool::decode(bytes)
        .map_err(|_| DomainError::InvalidConfig("gRPC FileDescriptorSet 无效".into()))
}

pub(super) async fn reflection_symbol_pool(
    channel: tonic::transport::Channel,
    service: &str,
    timeout: Duration,
    cancelled: ApiCancellation,
) -> DomainResult<DescriptorPool> {
    let request = ServerReflectionRequest {
        host: String::new(),
        message_request: Some(
            server_reflection_request::MessageRequest::FileContainingSymbol(service.to_owned()),
        ),
    };
    let responses = reflection_messages(channel, request, timeout, cancelled).await?;
    descriptor_pool_from_reflection(responses)
}

pub(super) async fn reflection_catalog_pool(
    channel: tonic::transport::Channel,
    timeout: Duration,
    cancelled: ApiCancellation,
) -> DomainResult<DescriptorPool> {
    let list_request = ServerReflectionRequest {
        host: String::new(),
        message_request: Some(server_reflection_request::MessageRequest::ListServices(
            String::new(),
        )),
    };
    let list =
        reflection_messages(channel.clone(), list_request, timeout, cancelled.clone()).await?;
    let service_names = list
        .into_iter()
        .find_map(|response| match response.message_response {
            Some(server_reflection_response::MessageResponse::ListServicesResponse(list)) => Some(
                list.service
                    .into_iter()
                    .map(|service| service.name)
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .ok_or_else(|| {
            DomainError::ConnectionFailed("gRPC Reflection 未返回 Service 目录".into())
        })?;
    if service_names.len() > MAX_DISCOVERED_SERVICES {
        return Err(DomainError::InvalidConfig(
            "gRPC Reflection Service 数量超过限制".into(),
        ));
    }

    let mut descriptor_bytes = Vec::new();
    for service in service_names {
        let request = ServerReflectionRequest {
            host: String::new(),
            message_request: Some(
                server_reflection_request::MessageRequest::FileContainingSymbol(service),
            ),
        };
        descriptor_bytes.extend(
            reflection_messages(channel.clone(), request, timeout, cancelled.clone()).await?,
        );
    }
    descriptor_pool_from_reflection(descriptor_bytes)
}

async fn reflection_messages(
    channel: tonic::transport::Channel,
    request: ServerReflectionRequest,
    timeout: Duration,
    cancelled: ApiCancellation,
) -> DomainResult<Vec<ServerReflectionResponse>> {
    let mut client = ServerReflectionClient::new(channel);
    let mut request = Request::new(tokio_stream::iter([request]));
    request.set_timeout(timeout);
    let response = tokio::select! {
        result = client.server_reflection_info(request) => result.map_err(map_reflection_status)?,
        _ = crate::http::wait_until_cancelled(cancelled.clone()) => {
            return Err(DomainError::Cancelled("gRPC Reflection 请求已取消".into()));
        }
    };
    let mut stream = response.into_inner();
    let mut messages = Vec::new();
    loop {
        let next = tokio::select! {
            result = stream.message() => result.map_err(map_reflection_status)?,
            _ = crate::http::wait_until_cancelled(cancelled.clone()) => {
                return Err(DomainError::Cancelled("gRPC Reflection 请求已取消".into()));
            }
        };
        let Some(message) = next else { break };
        messages.push(message);
    }
    Ok(messages)
}

fn descriptor_pool_from_reflection(
    responses: Vec<ServerReflectionResponse>,
) -> DomainResult<DescriptorPool> {
    let mut files = BTreeMap::new();
    for response in responses {
        match response.message_response {
            Some(server_reflection_response::MessageResponse::FileDescriptorResponse(
                FileDescriptorResponse {
                    file_descriptor_proto,
                },
            )) => {
                for bytes in file_descriptor_proto {
                    let file = prost_types::FileDescriptorProto::decode(bytes.as_slice()).map_err(
                        |_| {
                            DomainError::ConnectionFailed(
                                "gRPC Reflection 返回的 Descriptor 无效".into(),
                            )
                        },
                    )?;
                    let name = file.name.clone().ok_or_else(|| {
                        DomainError::ConnectionFailed(
                            "gRPC Reflection 返回的 Descriptor 缺少文件名".into(),
                        )
                    })?;
                    files.entry(name).or_insert(file);
                }
            }
            Some(server_reflection_response::MessageResponse::ErrorResponse(error)) => {
                return Err(DomainError::ConnectionFailed(format!(
                    "gRPC Reflection 返回错误码 {}",
                    error.error_code
                )));
            }
            Some(server_reflection_response::MessageResponse::ListServicesResponse(_)) => {}
            Some(_) | None => {}
        }
    }
    if files.is_empty() {
        return Err(DomainError::ConnectionFailed(
            "gRPC Reflection 未返回 Descriptor".into(),
        ));
    }
    DescriptorPool::from_file_descriptor_set(prost_types::FileDescriptorSet {
        file: files.into_values().collect(),
    })
    .map_err(|_| DomainError::ConnectionFailed("gRPC Descriptor 合并失败".into()))
}

pub(super) fn find_method(
    pool: &DescriptorPool,
    service_name: &str,
    method_name: &str,
) -> DomainResult<MethodDescriptor> {
    let service = pool.get_service_by_name(service_name).ok_or_else(|| {
        DomainError::InvalidConfig(format!("未找到 gRPC Service：{service_name}"))
    })?;
    service
        .methods()
        .find(|method| method.name() == method_name)
        .ok_or_else(|| DomainError::InvalidConfig(format!("未找到 gRPC Method：{method_name}")))
}

pub(super) fn service_summaries(pool: &DescriptorPool) -> DomainResult<Vec<GrpcServiceSummary>> {
    let services = pool
        .services()
        .map(|service| GrpcServiceSummary {
            name: service.full_name().to_owned(),
            methods: service
                .methods()
                .map(|method| GrpcServiceMethod {
                    name: method.name().to_owned(),
                    client_streaming: method.is_client_streaming(),
                    server_streaming: method.is_server_streaming(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    if services.len() > MAX_DISCOVERED_SERVICES {
        return Err(DomainError::InvalidConfig(
            "gRPC Service 数量超过限制".into(),
        ));
    }
    Ok(services)
}

pub(super) fn parse_request_message(
    message: &str,
    descriptor: MessageDescriptor,
) -> DomainResult<DynamicMessage> {
    let mut deserializer = serde_json::Deserializer::from_str(message);
    let parsed = DynamicMessage::deserialize(descriptor, &mut deserializer)
        .map_err(|_| DomainError::InvalidConfig("gRPC 请求消息 JSON 无效".into()))?;
    deserializer
        .end()
        .map_err(|_| DomainError::InvalidConfig("gRPC 请求消息 JSON 包含多余内容".into()))?;
    Ok(parsed)
}

pub(crate) fn parse_request_messages(
    message: &str,
    descriptor: MessageDescriptor,
    client_streaming: bool,
) -> DomainResult<Vec<DynamicMessage>> {
    if !client_streaming {
        return Ok(vec![parse_request_message(message, descriptor)?]);
    }

    let lines = message
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return Err(DomainError::InvalidConfig(
            "gRPC 流式请求至少需要一条 JSON 消息".into(),
        ));
    }
    if lines.len() > MAX_API_GRPC_STREAM_MESSAGES {
        return Err(DomainError::InvalidConfig(format!(
            "gRPC 流式请求消息数量超过 {MAX_API_GRPC_STREAM_MESSAGES} 条上限"
        )));
    }
    lines
        .into_iter()
        .map(|line| parse_request_message(line, descriptor.clone()))
        .collect()
}

fn map_reflection_status(status: Status) -> DomainError {
    DomainError::ConnectionFailed(format!("gRPC Reflection 请求失败：{}", status.code()))
}

pub(super) fn validate_discovery_timeout(timeout_millis: u64) -> DomainResult<()> {
    if timeout_millis == 0 || timeout_millis > MAX_API_TIMEOUT_MILLIS {
        return Err(DomainError::InvalidConfig(format!(
            "API 超时必须为 1 - {MAX_API_TIMEOUT_MILLIS} 毫秒"
        )));
    }
    Ok(())
}
