//! API 请求编辑器状态；具体布局和异步操作拆分到同目录文件。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, Render, ScrollHandle,
    Subscription, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Sizable as _, h_flex,
    input::{Input, InputState},
    v_flex,
};
use ramag_app::{ApiService, new_api_cancellation};
use ramag_domain::entities::{
    ApiAssertionResult, ApiAuth, ApiBody, ApiBodyMode, ApiCancellation, ApiCollection,
    ApiCollectionRunResult, ApiEnvironment, ApiExtractedVariable, ApiGrpcDiscoverySpec,
    ApiGrpcServiceSummary, ApiHistoryRecord, ApiMultipartPart, ApiMultipartValue, ApiParameter,
    ApiProtocol, ApiRequestSpec, ApiResponseSnapshot, ApiResponseStatus, ApiWorkspace,
    GrpcRequestSpec, HttpRequestSpec,
};
use ramag_domain::error::{DomainError, Result};

#[path = "context.rs"]
mod context;
#[path = "lifecycle.rs"]
mod lifecycle;
#[path = "operations.rs"]
mod operations;
#[path = "render.rs"]
mod render;
#[path = "render_body.rs"]
mod render_body;
#[path = "render_grpc.rs"]
mod render_grpc;
#[path = "render_helpers.rs"]
mod render_helpers;

const FIELD_BYTES: usize = 64 * 1024;
const API_SIDEBAR_WIDTH: f32 = 220.0;
const API_STACK_BREAKPOINT: f32 = 720.0;
const API_RESPONSE_PREVIEW_BYTES: usize = 16 * 1024;

/// API 工作台编辑状态；输入值先保留在 GPUI 状态中，点击发送时才构造领域请求。
pub struct ApiView {
    pub(crate) service: Option<Arc<ApiService>>,
    pub(crate) protocol: ApiProtocol,
    pub(crate) request_name: Entity<InputState>,
    pub(crate) http_method: Entity<InputState>,
    pub(crate) http_url: Entity<InputState>,
    pub(crate) http_query: Entity<InputState>,
    pub(crate) http_headers: Entity<InputState>,
    pub(crate) http_body: Entity<InputState>,
    pub(crate) http_auth: ApiAuth,
    pub(crate) http_body_mode: ApiBodyMode,
    pub(crate) http_body_content_type: String,
    pub(crate) environment_variables: Entity<InputState>,
    pub(crate) environment_sensitive: Entity<InputState>,
    pub(crate) runtime_environment: ApiEnvironment,
    pub(crate) assertions: Entity<InputState>,
    pub(crate) response_variables: Entity<InputState>,
    pub(crate) grpc_endpoint: Entity<InputState>,
    pub(crate) grpc_service: Entity<InputState>,
    pub(crate) grpc_method: Entity<InputState>,
    pub(crate) grpc_metadata_name: Entity<InputState>,
    pub(crate) grpc_metadata_value: Entity<InputState>,
    pub(crate) grpc_message: Entity<InputState>,
    pub(crate) grpc_services: Vec<ApiGrpcServiceSummary>,
    pub(crate) grpc_discovering: bool,
    pub(crate) grpc_discovery_generation: u64,
    pub(crate) grpc_discovery_cancelled: Option<ApiCancellation>,
    pub(crate) response: Option<ApiResponseSnapshot>,
    pub(crate) assertion_results: Vec<ApiAssertionResult>,
    pub(crate) extracted_variables: Vec<ApiExtractedVariable>,
    pub(crate) history: Vec<ApiHistoryRecord>,
    pub(crate) last_collection_run: Option<ApiCollectionRunResult>,
    pub(crate) loading: bool,
    pub(crate) saving: bool,
    pub(crate) importing: bool,
    pub(crate) notice: Option<(String, bool)>,
    pub(crate) workspace: ApiWorkspace,
    pub(crate) request_generation: u64,
    pub(crate) cancelled: Option<Arc<AtomicBool>>,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) response_scroll: ScrollHandle,
    pub(crate) _subscriptions: Vec<Subscription>,
}

impl ApiView {
    /// 创建没有运行时服务的视图，供 headless 布局和交互测试使用。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::without_service(window, cx)
    }

    /// 创建接入应用服务的 API 工作台。
    pub fn with_service(
        service: Arc<ApiService>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self::without_service(window, cx);
        view.service = Some(service);
        view.load_saved_workspace(cx);
        view
    }

    pub(crate) fn without_service(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            service: None,
            protocol: ApiProtocol::Http,
            request_name: api_input(window, cx, "请求名称", "新请求"),
            http_method: api_input(window, cx, "GET / POST", "GET"),
            http_url: api_input(window, cx, "https://example.com", "{{base_url}}/json"),
            http_query: api_multiline_input(
                window,
                cx,
                "每行一个查询参数，例如 q=hello",
                "",
                None,
                3,
            ),
            http_headers: api_multiline_input(
                window,
                cx,
                "每行一个请求头，例如 Content-Type: application/json",
                "",
                None,
                5,
            ),
            http_body: api_multiline_input(
                window,
                cx,
                "JSON 请求正文（可选）",
                "",
                Some("json"),
                8,
            ),
            http_auth: ApiAuth::None,
            http_body_mode: ApiBodyMode::Text,
            http_body_content_type: "application/json".into(),
            environment_variables: api_multiline_input(
                window,
                cx,
                "每行一个变量，例如 base_url=http://127.0.0.1:18089",
                "base_url=http://127.0.0.1:18089",
                None,
                3,
            ),
            environment_sensitive: api_multiline_input(
                window,
                cx,
                "每行一个敏感变量名（可选）",
                "",
                None,
                2,
            ),
            runtime_environment: ApiEnvironment::new("local"),
            assertions: api_multiline_input(
                window,
                cx,
                "status=200、body=ok 或 json=$.ok:true",
                "",
                None,
                3,
            ),
            response_variables: api_multiline_input(
                window,
                cx,
                "name=json:$.token、name=header:X-Request-Id 或 secret name=...",
                "",
                None,
                4,
            ),
            grpc_endpoint: api_input(
                window,
                cx,
                "http://127.0.0.1:18090",
                "http://127.0.0.1:18090",
            ),
            grpc_service: api_input(window, cx, "package.Service", "api.docker.Echo"),
            grpc_method: api_input(window, cx, "Unary", "Unary"),
            grpc_metadata_name: api_input(window, cx, "Metadata name", "x-request"),
            grpc_metadata_value: api_input(window, cx, "Metadata value", "docker"),
            grpc_message: api_multiline_input(
                window,
                cx,
                "Protobuf JSON；流式请求每行一个对象",
                r#"{"message":"hello"}"#,
                Some("json"),
                6,
            ),
            grpc_services: Vec::new(),
            grpc_discovering: false,
            grpc_discovery_generation: 0,
            grpc_discovery_cancelled: None,
            response: None,
            assertion_results: Vec::new(),
            extracted_variables: Vec::new(),
            history: Vec::new(),
            last_collection_run: None,
            loading: false,
            saving: false,
            importing: false,
            notice: None,
            workspace: ApiWorkspace::new("API Workspace"),
            request_generation: 0,
            cancelled: None,
            focus_handle: cx.focus_handle(),
            response_scroll: ScrollHandle::new(),
            _subscriptions: Vec::new(),
        }
    }

    pub(crate) fn main_content_width(window: &Window) -> f32 {
        f32::from(window.viewport_size().width)
    }

    pub(crate) fn is_stacked(window: &Window) -> bool {
        Self::main_content_width(window) < API_STACK_BREAKPOINT
    }

    /// 异步读取首个本地工作区；读取失败保留当前空白工作区，避免启动时阻塞工具页面。
    fn load_saved_workspace(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = service.list_workspaces().await;
            let Some(workspace) = result
                .ok()
                .and_then(|workspaces| workspaces.into_iter().next())
            else {
                return;
            };
            let history = service
                .list_history(&workspace.id, 20)
                .await
                .unwrap_or_default();
            let _ = this.update(cx, move |view, cx| {
                view.workspace = workspace;
                view.history = history;
                cx.notify();
            });
        })
        .detach();
    }
}

impl Focusable for ApiView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ApiView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render::render(self, window, cx)
    }
}

fn api_input(
    window: &mut Window,
    cx: &mut Context<ApiView>,
    placeholder: &'static str,
    default: &str,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .validate(|value, _| value.len() <= FIELD_BYTES)
            .placeholder(placeholder)
            .default_value(default.to_string())
    })
}

/// 创建请求报文编辑器；代码编辑器保持行号和缩进，多行文本则保持轻量输入体验。
fn api_multiline_input(
    window: &mut Window,
    cx: &mut Context<ApiView>,
    placeholder: &'static str,
    default: &str,
    language: Option<&'static str>,
    rows: usize,
) -> Entity<InputState> {
    cx.new(|cx| {
        let state = InputState::new(window, cx)
            .validate(|value, _| value.len() <= FIELD_BYTES)
            .placeholder(placeholder)
            .default_value(default.to_string());
        let state = match language {
            Some(language) => state.code_editor(language),
            None => state.multi_line(true),
        };
        state.rows(rows)
    })
}

pub(crate) fn input_value(field: &Entity<InputState>, cx: &App) -> String {
    field.read(cx).value().trim().to_string()
}

pub(crate) fn field<E: IntoElement>(label: &'static str, input: E) -> gpui::Div {
    v_flex()
        .flex_1()
        .min_w(px(180.0))
        .gap(px(5.0))
        .child(
            div()
                .text_xs()
                .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                .child(label),
        )
        .child(div().w_full().min_w_0().child(input))
}

pub(crate) fn row() -> gpui::Div {
    h_flex()
        .w_full()
        .min_w_0()
        .flex_wrap()
        .items_end()
        .gap(px(10.0))
}

pub(crate) fn status_text(snapshot: &ApiResponseSnapshot) -> String {
    let status = match &snapshot.status {
        ApiResponseStatus::Http { code } => format!("HTTP {code}"),
        ApiResponseStatus::Grpc { code } => format!("gRPC {code}"),
        ApiResponseStatus::TransportError => "传输错误".into(),
    };
    format!(
        "{status} · {} ms · {} bytes",
        snapshot.elapsed_millis, snapshot.size_bytes
    )
}

pub(crate) fn body_preview(snapshot: &ApiResponseSnapshot) -> String {
    let (text, _) = response_body_text(snapshot);
    let mut preview = text
        .chars()
        .take(API_RESPONSE_PREVIEW_BYTES)
        .collect::<String>();
    if text.chars().count() > API_RESPONSE_PREVIEW_BYTES || snapshot.truncated {
        preview.push_str("\n…响应正文已限制显示");
    }
    preview
}

/// 将 JSON 正文格式化为可读文本；解析失败时返回原文，避免隐藏服务端实际响应。
fn response_body_text(snapshot: &ApiResponseSnapshot) -> (String, &'static str) {
    let raw = String::from_utf8_lossy(&snapshot.body);
    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(value) => serde_json::to_string_pretty(&value)
            .map(|text| (text, "JSON"))
            .unwrap_or_else(|_| (raw.into_owned(), "原文")),
        Err(_) => (raw.into_owned(), "原文"),
    }
}

pub(crate) fn body_format_label(snapshot: &ApiResponseSnapshot) -> &'static str {
    response_body_text(snapshot).1
}

/// 把用户输入的逐行 `Name: Value` 文本转换成领域层请求头。
pub(crate) fn parse_http_headers(value: &str) -> Result<Vec<ApiParameter>> {
    let mut headers = Vec::new();
    for (index, line) in value.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, header_value)) = line.split_once(':') else {
            return Err(DomainError::InvalidConfig(format!(
                "请求头第 {} 行必须使用 Name: Value 格式",
                index + 1
            )));
        };
        let name = name.trim();
        let header_value = header_value.trim();
        if name.is_empty() || header_value.is_empty() {
            return Err(DomainError::InvalidConfig(format!(
                "请求头第 {} 行的名称和值都不能为空",
                index + 1
            )));
        }
        headers.push(ApiParameter::new(name, header_value, false));
    }
    Ok(headers)
}

/// 把用户输入的逐行 `name=value` 文本转换成领域层查询参数。
pub(crate) fn parse_http_query(value: &str) -> Result<Vec<ApiParameter>> {
    let mut query = Vec::new();
    for (index, line) in value.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, query_value)) = line.split_once('=') else {
            return Err(DomainError::InvalidConfig(format!(
                "查询参数第 {} 行必须使用 name=value 格式",
                index + 1
            )));
        };
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::InvalidConfig(format!(
                "查询参数第 {} 行的名称不能为空",
                index + 1
            )));
        }
        query.push(ApiParameter::new(name, query_value.trim(), false));
    }
    Ok(query)
}

/// 解析 Multipart 编辑器的逐行格式：`text|字段|值[|secret]` 或
/// `file|字段|路径[|文件名|Content-Type]`。
pub(crate) fn parse_multipart_body(value: &str) -> Result<Vec<ApiMultipartPart>> {
    let mut parts = Vec::new();
    for (index, line) in value.lines().enumerate() {
        let line_number = index + 1;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields = line.splitn(5, '|').collect::<Vec<_>>();
        let kind = fields.first().copied().unwrap_or_default().trim();
        let name = fields.get(1).copied().unwrap_or_default().trim();
        let value = fields.get(2).copied().unwrap_or_default().trim();
        if name.is_empty() || value.is_empty() {
            return Err(DomainError::InvalidConfig(format!(
                "Multipart 第 {line_number} 行必须包含类型、字段名称和值"
            )));
        }
        let part = match kind.to_ascii_lowercase().as_str() {
            "text" => {
                let sensitive = fields
                    .get(3)
                    .is_some_and(|flag| flag.trim().eq_ignore_ascii_case("secret"));
                ApiMultipartPart::text(name, value, sensitive)
            }
            "file" => {
                let file_name = fields
                    .get(3)
                    .map(|file_name| file_name.trim().to_string())
                    .filter(|file_name| !file_name.is_empty());
                let content_type = fields
                    .get(4)
                    .map(|content_type| content_type.trim().to_string())
                    .filter(|content_type| !content_type.is_empty());
                ApiMultipartPart::file(name, value, file_name, content_type)
            }
            _ => {
                return Err(DomainError::InvalidConfig(format!(
                    "Multipart 第 {line_number} 行类型必须是 text 或 file"
                )));
            }
        };
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(DomainError::InvalidConfig(
            "Multipart 至少需要一个字段".into(),
        ));
    }
    Ok(parts)
}

pub(crate) fn format_multipart_body(parts: &[ApiMultipartPart]) -> String {
    parts
        .iter()
        .map(|part| match &part.value {
            ApiMultipartValue::Text { value } => format!(
                "text|{}|{}{}",
                part.name,
                value,
                if part.sensitive { "|secret" } else { "" }
            ),
            ApiMultipartValue::File { path, file_name } => format!(
                "file|{}|{}|{}|{}",
                part.name,
                path,
                file_name.as_deref().unwrap_or_default(),
                part.content_type.as_deref().unwrap_or_default()
            ),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn request_from_view(view: &ApiView, cx: &App) -> Result<ApiRequestSpec> {
    match view.protocol {
        ApiProtocol::Http => {
            let method = input_value(&view.http_method, cx).to_ascii_uppercase();
            let mut spec = HttpRequestSpec::new(method, input_value(&view.http_url, cx));
            spec.query = parse_http_query(&input_value(&view.http_query, cx))?;
            spec.headers = parse_http_headers(&input_value(&view.http_headers, cx))?;
            spec.auth = view.http_auth.clone();
            let body = input_value(&view.http_body, cx);
            if !body.trim().is_empty() {
                spec.body = Some(match view.http_body_mode {
                    ApiBodyMode::Text => {
                        let content_type = spec
                            .headers
                            .iter()
                            .find(|header| header.name.eq_ignore_ascii_case("content-type"))
                            .map(|header| header.value.clone())
                            .or_else(|| {
                                let value = view.http_body_content_type.trim();
                                (!value.is_empty()).then(|| value.to_string())
                            });
                        ApiBody::text(body, content_type)
                    }
                    ApiBodyMode::Multipart => ApiBody::multipart(parse_multipart_body(&body)?),
                });
            }
            Ok(ApiRequestSpec::Http(spec))
        }
        ApiProtocol::Grpc => {
            let mut spec = GrpcRequestSpec::new(
                input_value(&view.grpc_endpoint, cx),
                input_value(&view.grpc_service, cx),
                input_value(&view.grpc_method, cx),
            )
            .with_message(input_value(&view.grpc_message, cx));
            let metadata_name = input_value(&view.grpc_metadata_name, cx);
            let metadata_value = input_value(&view.grpc_metadata_value, cx);
            if !metadata_name.is_empty() || !metadata_value.is_empty() {
                spec.metadata
                    .push(ApiParameter::new(metadata_name, metadata_value, false));
            }
            Ok(ApiRequestSpec::Grpc(spec))
        }
    }
}

trait GrpcRequestMessage {
    fn with_message(self, message: String) -> Self;
}

impl GrpcRequestMessage for GrpcRequestSpec {
    fn with_message(mut self, message: String) -> Self {
        self.message = message;
        self
    }
}

#[cfg(test)]
#[path = "body_tests.rs"]
mod body_tests;
#[cfg(test)]
#[path = "view_docker_tests.rs"]
mod docker_tests;
#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
