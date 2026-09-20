//! API 请求执行驱动的领域接口。

use std::collections::BTreeMap;

use async_trait::async_trait;

use crate::entities::{
    ApiCancellation, ApiGrpcDiscoverySpec, ApiGrpcServiceSummary, ApiProtocol, ApiRequestSpec,
    ApiResponseSnapshot,
};
use crate::error::{DomainError, Result};

#[async_trait]
pub trait ApiDriver: Send + Sync {
    /// 返回该实现可以执行的协议，应用层据此选择 HTTP 或 gRPC 驱动。
    fn protocol(&self) -> ApiProtocol;

    /// 执行一条已保存的请求；变量由应用层解析后传入，响应必须受领域层大小限制。
    async fn execute(
        &self,
        request: &ApiRequestSpec,
        variables: &BTreeMap<String, String>,
        cancelled: ApiCancellation,
    ) -> Result<ApiResponseSnapshot>;

    /// 发现 gRPC Service/Method；不支持发现的协议驱动返回明确的配置错误。
    async fn discover_grpc_services(
        &self,
        request: &ApiGrpcDiscoverySpec,
        variables: &BTreeMap<String, String>,
        cancelled: ApiCancellation,
    ) -> Result<Vec<ApiGrpcServiceSummary>> {
        let _ = (request, variables, cancelled);
        Err(DomainError::InvalidConfig(
            "当前驱动不支持 gRPC Service 发现".into(),
        ))
    }
}
