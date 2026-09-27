//! 远程协作 Relay 接口；领域层不关心 HTTP、TLS 或服务端实现。

use async_trait::async_trait;

use crate::entities::{CollaborationRemotePackage, CollaborationRemoteReceipt};
use crate::error::Result;

/// 只承载用户已明确选择并通过导出校验的共享包；实现不得自动重试写操作。
#[async_trait]
pub trait CollaborationRelay: Send + Sync {
    async fn publish(&self, endpoint: &str, payload: &str) -> Result<CollaborationRemoteReceipt>;

    async fn fetch(&self, endpoint: &str, remote_id: &str) -> Result<CollaborationRemotePackage>;
}
