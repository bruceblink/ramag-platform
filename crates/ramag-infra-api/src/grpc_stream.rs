//! gRPC 流式响应收集和有界快照。

use std::time::Instant;

use prost_reflect::DynamicMessage;
use ramag_domain::entities::{
    ApiCancellation, ApiProtocol, ApiResponseSnapshot, ApiResponseSnapshotParts, ApiResponseStatus,
    MAX_API_GRPC_STREAM_MESSAGES, MAX_API_RESPONSE_BODY_BYTES,
};
use ramag_domain::error::{DomainError, Result as DomainResult};
use tonic::Response;
use tonic::codec::Streaming;
use tonic::metadata::MetadataMap;

use super::transport::{response_metadata, status_snapshot};

pub(super) async fn collect_response_stream(
    response: Response<Streaming<DynamicMessage>>,
    started: Instant,
    cancelled: ApiCancellation,
) -> DomainResult<ApiResponseSnapshot> {
    let (mut metadata, mut stream, _) = response.into_parts();
    let mut body = vec![b'['];
    let mut response_count = 0usize;
    let mut size_bytes = 1u64;
    let mut truncated = false;

    loop {
        let next = tokio::select! {
            result = stream.message() => result,
            _ = crate::http::wait_until_cancelled(cancelled.clone()) => {
                return Err(DomainError::Cancelled("gRPC 流式请求已取消".into()));
            }
        };
        let Some(message) = (match next {
            Ok(message) => message,
            Err(status) => return status_snapshot(status, started),
        }) else {
            break;
        };

        let encoded = serde_json::to_vec(&message)
            .map_err(|_| DomainError::QueryFailed("gRPC 流式响应消息序列化失败".into()))?;
        let separator = usize::from(response_count > 0);
        let next_size = size_bytes
            .checked_add(separator as u64)
            .and_then(|size| size.checked_add(encoded.len() as u64))
            .and_then(|size| size.checked_add(1))
            .ok_or_else(|| DomainError::QueryFailed("gRPC 流式响应大小计算溢出".into()))?;
        if response_count >= MAX_API_GRPC_STREAM_MESSAGES
            || next_size > MAX_API_RESPONSE_BODY_BYTES as u64
        {
            truncated = true;
            size_bytes = next_size;
            break;
        }

        if separator != 0 {
            body.push(b',');
        }
        body.extend_from_slice(&encoded);
        response_count += 1;
        size_bytes = next_size;
    }

    if !truncated
        && let Some(trailers) = stream.trailers().await.map_err(|status| {
            DomainError::QueryFailed(format!("gRPC 流式响应读取失败：{status}"))
        })?
    {
        merge_metadata(&mut metadata, trailers);
    }
    body.push(b']');
    size_bytes = size_bytes.max(body.len() as u64);
    ApiResponseSnapshot::new(ApiResponseSnapshotParts {
        protocol: ApiProtocol::Grpc,
        status: ApiResponseStatus::Grpc { code: "ok".into() },
        headers: Vec::new(),
        metadata: response_metadata(&metadata)?,
        body,
        elapsed_millis: elapsed_millis(started),
        size_bytes,
        truncated,
        error: None,
    })
    .map_err(DomainError::InvalidConfig)
}

fn merge_metadata(target: &mut MetadataMap, trailers: MetadataMap) {
    let mut headers = target.clone().into_headers();
    headers.extend(trailers.into_headers());
    *target = MetadataMap::from_headers(headers);
}

fn elapsed_millis(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}
