//! API 测试工具的 HTTP 和 gRPC 基础设施驱动。
//!
//! 领域模型和应用编排仍在 `ramag-domain`/`ramag-app`，本 crate 不保存 UI 状态或执行历史。

#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

#[path = "grpc_core.rs"]
mod grpc;
mod http;

pub use grpc::{DynamicGrpcClient, GrpcApiDriver, GrpcServiceMethod, GrpcServiceSummary};
pub use http::HttpApiDriver;

#[cfg(test)]
#[path = "grpc_tests.rs"]
mod tests;
