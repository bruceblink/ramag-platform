use serde::{Deserialize, Serialize};

/// gRPC Service 中一个可调用方法的发现结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiGrpcMethodSummary {
    pub name: String,
    pub client_streaming: bool,
    pub server_streaming: bool,
}

/// Server Reflection 返回的 gRPC Service 目录项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiGrpcServiceSummary {
    pub name: String,
    pub methods: Vec<ApiGrpcMethodSummary>,
}
