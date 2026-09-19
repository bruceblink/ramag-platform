//! API 请求编辑器状态；具体布局和异步操作拆分到同目录文件。

use std::collections::BTreeMap;
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
    ApiBody, ApiCollection, ApiParameter, ApiProtocol, ApiRequestRecord, ApiRequestSpec,
    ApiResponseSnapshot, ApiResponseStatus, ApiWorkspace, GrpcRequestSpec, HttpRequestSpec,
};
use ramag_domain::error::{DomainError, Result};

#[path = "operations.rs"]
mod operations;
#[path = "render.rs"]
mod render;

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
    pub(crate) http_headers: Entity<InputState>,
    pub(crate) http_body: Entity<InputState>,
    pub(crate) grpc_endpoint: Entity<InputState>,
    pub(crate) grpc_service: Entity<InputState>,
    pub(crate) grpc_method: Entity<InputState>,
    pub(crate) grpc_metadata_name: Entity<InputState>,
    pub(crate) grpc_metadata_value: Entity<InputState>,
    pub(crate) grpc_message: Entity<InputState>,
    pub(crate) response: Option<ApiResponseSnapshot>,
    pub(crate) loading: bool,
    pub(crate) saving: bool,
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
            http_url: api_input(
                window,
                cx,
                "https://example.com",
                "http://127.0.0.1:18089/json",
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
            grpc_message: api_input(window, cx, "Protobuf JSON", r#"{"message":"hello"}"#),
            response: None,
            loading: false,
            saving: false,
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

    pub(crate) fn set_protocol(&mut self, protocol: ApiProtocol, cx: &mut Context<Self>) {
        self.protocol = protocol;
        self.response = None;
        self.notice = None;
        cx.notify();
    }

    pub(crate) fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(cancelled) = &self.cancelled {
            cancelled.store(true, Ordering::Relaxed);
        }
        if self.loading {
            self.request_generation = self.request_generation.wrapping_add(1);
            self.loading = false;
            self.notice = Some(("请求已取消".into(), false));
            cx.notify();
        }
    }

    /// 异步读取首个本地工作区；读取失败保留当前空白工作区，避免启动时阻塞工具页面。
    fn load_saved_workspace(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = service.list_workspaces().await;
            let _ = this.update(cx, |view, cx| {
                if let Ok(workspaces) = result
                    && let Some(workspace) = workspaces.into_iter().next()
                {
                    view.workspace = workspace;
                    cx.notify();
                }
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

pub(crate) fn request_from_view(view: &ApiView, cx: &App) -> Result<ApiRequestSpec> {
    match view.protocol {
        ApiProtocol::Http => {
            let method = input_value(&view.http_method, cx).to_ascii_uppercase();
            let mut spec = HttpRequestSpec::new(method, input_value(&view.http_url, cx));
            spec.headers = parse_http_headers(&input_value(&view.http_headers, cx))?;
            let body = input_value(&view.http_body, cx);
            if !body.is_empty() {
                spec.body = Some(ApiBody::text(body, Some("application/json".into())));
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

fn request_record(view: &ApiView, cx: &App) -> Result<ApiRequestRecord> {
    let name = input_value(&view.request_name, cx);
    let request = request_from_view(view, cx)?;
    Ok(match request {
        ApiRequestSpec::Http(spec) => ApiRequestRecord::new_http(name, spec),
        ApiRequestSpec::Grpc(spec) => ApiRequestRecord::new_grpc(name, spec),
    })
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
