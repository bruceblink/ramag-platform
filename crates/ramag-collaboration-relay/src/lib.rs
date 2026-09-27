//! 本机协作 Relay 开发验证服务。
//!
//! 该服务只保存通过领域导出校验的非敏感共享包，默认内存存储并绑定回环地址。
//! 它没有账号、权限、持久化或自动同步能力，不能直接作为生产服务部署。

#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use ramag_domain::entities::{
    CollaborationShare, MAX_COLLABORATION_EXPORT_BYTES, MAX_COLLABORATION_REMOTE_ID_BYTES,
    MAX_COLLABORATION_SHARES,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use uuid::Uuid;

const FORMAT: &str = "ramag.collaboration.v1";
const MAX_REQUEST_BYTES: usize = MAX_COLLABORATION_EXPORT_BYTES + 1024;

#[derive(Clone, Default)]
pub struct RelayState {
    records: Arc<RwLock<HashMap<String, StoredShare>>>,
}

#[derive(Clone)]
struct StoredShare {
    revision: u64,
    payload: String,
}

#[derive(Debug, Deserialize)]
struct PublishRequest {
    format: String,
    payload: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RelayResponse {
    format: String,
    remote_id: String,
    revision: u64,
    expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<String>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: &'static str,
}

pub fn router(state: RelayState) -> Router {
    Router::new()
        .route("/v1/collaboration/shares", post(publish))
        .route("/v1/collaboration/shares/{remote_id}", get(fetch))
        .route("/health", get(health))
        .with_state(state)
        .layer(axum::extract::DefaultBodyLimit::max(MAX_REQUEST_BYTES))
}

pub async fn serve(listener: TcpListener, state: RelayState) -> std::io::Result<()> {
    axum::serve(listener, router(state)).await
}

pub async fn run(bind: SocketAddr) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind).await?;
    eprintln!("Ramag collaboration Relay listening on {listener:?}");
    serve(listener, RelayState::default()).await
}

async fn publish(
    State(state): State<RelayState>,
    Json(request): Json<PublishRequest>,
) -> Response<Body> {
    if request.format != FORMAT {
        return error(StatusCode::BAD_REQUEST, "协作包格式不支持");
    }
    if request.payload.len() > MAX_COLLABORATION_EXPORT_BYTES {
        return error(StatusCode::PAYLOAD_TOO_LARGE, "协作包超过大小上限");
    }
    let share = match serde_json::from_str::<CollaborationShare>(&request.payload) {
        Ok(share) => share,
        Err(_) => return error(StatusCode::BAD_REQUEST, "协作包格式无效"),
    };
    if share.manual_export_json().is_err() {
        return error(StatusCode::BAD_REQUEST, "协作包未通过安全校验");
    }
    let remote_id = Uuid::new_v4().to_string();
    let mut records = state.records.write().await;
    if records.len() >= MAX_COLLABORATION_SHARES {
        return error(StatusCode::SERVICE_UNAVAILABLE, "Relay 容量已满");
    }
    records.insert(
        remote_id.clone(),
        StoredShare {
            revision: 1,
            payload: request.payload,
        },
    );
    Json(RelayResponse {
        format: FORMAT.into(),
        remote_id,
        revision: 1,
        expires_at: None,
        payload: None,
    })
    .into_response()
}

async fn fetch(State(state): State<RelayState>, Path(remote_id): Path<String>) -> Response<Body> {
    if remote_id.is_empty()
        || remote_id.len() > MAX_COLLABORATION_REMOTE_ID_BYTES
        || remote_id
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | '?' | '#'))
    {
        return error(StatusCode::BAD_REQUEST, "远端共享包 ID 无效");
    }
    let records = state.records.read().await;
    let Some(stored) = records.get(&remote_id) else {
        return error(StatusCode::NOT_FOUND, "远端共享包不存在");
    };
    Json(RelayResponse {
        format: FORMAT.into(),
        remote_id,
        revision: stored.revision,
        expires_at: None,
        payload: Some(stored.payload.clone()),
    })
    .into_response()
}

async fn health() -> &'static str {
    "ok"
}

fn error(status: StatusCode, _message: &'static str) -> Response<Body> {
    // Keep response text generic so rejected content never appears in a client-visible error.
    (
        status,
        Json(ErrorResponse {
            error: "协作 Relay 请求被拒绝",
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::entities::CollaborationArtifact;

    async fn test_server() -> (String, tokio::task::JoinHandle<()>) {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind test listener");
        let address = listener.local_addr().expect("listener address");
        let handle = tokio::spawn(async move {
            axum::serve(listener, router(RelayState::default()))
                .await
                .expect("test server serves");
        });
        (format!("http://{address}"), handle)
    }

    fn safe_export() -> String {
        let mut share = CollaborationShare::new_local(
            "Relay 测试",
            vec![CollaborationArtifact::document("说明", "safe")],
        )
        .expect("local share");
        share.prepare_manual_export("test").expect("prepare export");
        share.manual_export_json().expect("export json")
    }

    #[tokio::test]
    async fn publish_and_fetch_only_accept_safe_exports() {
        let (endpoint, server) = test_server().await;
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{endpoint}/v1/collaboration/shares"))
            .json(&serde_json::json!({ "format": FORMAT, "payload": safe_export() }))
            .send()
            .await
            .expect("publish request");
        assert_eq!(response.status(), StatusCode::OK);
        let receipt: RelayResponse = response.json().await.expect("receipt");
        let fetched = client
            .get(format!(
                "{endpoint}/v1/collaboration/shares/{}",
                receipt.remote_id
            ))
            .send()
            .await
            .expect("fetch request");
        assert_eq!(fetched.status(), StatusCode::OK);
        let body: RelayResponse = fetched.json().await.expect("package");
        assert_eq!(body.format, FORMAT);
        assert!(body.payload.is_some());
        server.abort();
    }

    #[tokio::test]
    async fn rejects_wrong_format_and_sensitive_export() {
        let (endpoint, server) = test_server().await;
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{endpoint}/v1/collaboration/shares"))
            .json(&serde_json::json!({ "format": "other", "payload": safe_export() }))
            .send()
            .await
            .expect("wrong format request");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let mut share = CollaborationShare::new_local(
            "敏感",
            vec![CollaborationArtifact {
                data_class: ramag_domain::CollaborationDataClass::Sensitive,
                ..CollaborationArtifact::document("内部", "secret")
            }],
        )
        .expect("local sensitive draft");
        assert!(share.prepare_manual_export("test").is_err());
        server.abort();
    }

    #[tokio::test]
    async fn rejects_unknown_ids_oversized_exports_and_full_capacity() {
        let unknown = fetch(State(RelayState::default()), Path("missing".to_owned())).await;
        assert_eq!(unknown.status(), StatusCode::NOT_FOUND);

        let oversized = publish(
            State(RelayState::default()),
            Json(PublishRequest {
                format: FORMAT.to_owned(),
                payload: "x".repeat(MAX_COLLABORATION_EXPORT_BYTES + 1),
            }),
        )
        .await;
        assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let state = RelayState::default();
        {
            let mut records = state.records.write().await;
            for index in 0..MAX_COLLABORATION_SHARES {
                records.insert(
                    format!("filled-{index}"),
                    StoredShare {
                        revision: 1,
                        payload: safe_export(),
                    },
                );
            }
        }
        let full = publish(
            State(state),
            Json(PublishRequest {
                format: FORMAT.to_owned(),
                payload: safe_export(),
            }),
        )
        .await;
        assert_eq!(full.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
