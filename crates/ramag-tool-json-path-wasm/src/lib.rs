//! JSON Path 纯计算核心的 WASM 边界；不暴露 GPUI、凭据、网络或文件系统能力。

#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

use ramag_domain::extract_json_path;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

const MAX_ADAPTER_ERROR_BYTES: usize = 512;

#[derive(Debug, Deserialize)]
struct JsonPathRequest {
    raw_json: String,
    path: String,
}

#[derive(Debug, Serialize)]
struct JsonPathResponse {
    ok: bool,
    output: Option<String>,
    error: Option<String>,
}

/// 接收 JSON 编码请求并返回 JSON 编码结果，保持 WASM 与桌面核心使用同一输入协议。
#[wasm_bindgen]
pub fn extract_json_path_json(request: &str) -> String {
    let response = match serde_json::from_str::<JsonPathRequest>(request) {
        Ok(request) => match extract_json_path(&request.raw_json, &request.path) {
            Ok(output) => JsonPathResponse {
                ok: true,
                output: Some(output),
                error: None,
            },
            Err(error) => JsonPathResponse {
                ok: false,
                output: None,
                error: Some(bound_error(error.to_string())),
            },
        },
        Err(error) => JsonPathResponse {
            ok: false,
            output: None,
            error: Some(bound_error(format!("请求格式无效：{error}"))),
        },
    };
    serde_json::to_string(&response).unwrap_or_else(|_| {
        // Response fields are fixed and bounded; this branch is a defensive fallback for a
        // serializer failure and never includes the original input.
        r#"{"ok":false,"output":null,"error":"WASM 结果序列化失败"}"#.to_owned()
    })
}

fn bound_error(message: String) -> String {
    if message.len() <= MAX_ADAPTER_ERROR_BYTES {
        return message;
    }
    let mut end = MAX_ADAPTER_ERROR_BYTES - 3;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &message[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::extract_json_path;

    #[test]
    fn wasm_boundary_matches_native_core_result() {
        let request = serde_json::json!({
            "raw_json": "{ users: [{ name: 'Alice' }, { name: 'Bob' }] }",
            "path": "$.users[*].name"
        });
        let result: serde_json::Value =
            serde_json::from_str(&extract_json_path_json(&request.to_string())).expect("response");
        assert_eq!(result["ok"], true);
        assert_eq!(
            result["output"],
            extract_json_path(
                request["raw_json"].as_str().expect("raw json"),
                request["path"].as_str().expect("path")
            )
            .expect("native extraction")
        );
    }

    #[test]
    fn wasm_boundary_returns_bounded_structured_error() {
        let result: serde_json::Value = serde_json::from_str(&extract_json_path_json(
            r#"{"raw_json":"{}","path":"$.missing["}"#,
        ))
        .expect("response");
        assert_eq!(result["ok"], false);
        assert!(
            result["error"]
                .as_str()
                .is_some_and(|error| !error.is_empty())
        );
        assert!(result["output"].is_null());
    }
}
