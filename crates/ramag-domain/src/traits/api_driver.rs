//! API 请求执行驱动的领域接口。

use std::collections::BTreeMap;

use async_trait::async_trait;

use crate::entities::{ApiCancellation, ApiProtocol, ApiRequestSpec, ApiResponseSnapshot};
use crate::error::Result;

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
}
