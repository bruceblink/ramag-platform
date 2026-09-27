use super::*;

impl ApiView {
    /// 将当前已保留的响应正文复制到系统剪贴板；不会重新读取网络响应或扩大缓存边界。
    pub(crate) fn copy_response_body(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.response.as_ref() else {
            return;
        };
        let body = String::from_utf8_lossy(&snapshot.body).into_owned();
        ramag_ui::copy_text_with_notification(body, window, cx);
    }

    pub(crate) fn set_protocol(&mut self, protocol: ApiProtocol, cx: &mut Context<Self>) {
        // 切换协议会使当前请求上下文失效；先取消后台驱动并递增代际，避免旧响应回写新编辑器。
        let request_active = self.loading || self.cancelled.is_some();
        if let Some(cancelled) = &self.cancelled {
            cancelled.store(true, Ordering::Relaxed);
        }
        if request_active {
            self.request_generation = self.request_generation.wrapping_add(1);
            self.loading = false;
            self.cancelled = None;
        }
        if self.grpc_discovering {
            if let Some(cancelled) = &self.grpc_discovery_cancelled {
                cancelled.store(true, Ordering::Relaxed);
            }
            self.grpc_discovery_generation = self.grpc_discovery_generation.wrapping_add(1);
            self.grpc_discovering = false;
            self.grpc_discovery_cancelled = None;
        }
        self.protocol = protocol;
        self.grpc_services.clear();
        self.response = None;
        self.response_tab = ApiResponseTab::Body;
        self.assertion_results.clear();
        self.extracted_variables.clear();
        self.last_collection_run = None;
        self.notice = None;
        cx.notify();
    }

    pub(crate) fn set_http_body_mode(&mut self, mode: ApiBodyMode, cx: &mut Context<Self>) {
        self.http_body_mode = mode;
        self.http_body_content_type = match mode {
            ApiBodyMode::Text => "application/json".into(),
            ApiBodyMode::Multipart => String::new(),
        };
        self.notice = None;
        cx.notify();
    }

    /// 取消当前请求或 gRPC 发现，并清理代际与句柄，防止迟到回调继续回写视图。
    pub(crate) fn cancel(&mut self, cx: &mut Context<Self>) {
        let request_active = self.loading || self.cancelled.is_some();
        if let Some(cancelled) = &self.cancelled {
            cancelled.store(true, Ordering::Relaxed);
        }
        if let Some(cancelled) = &self.grpc_discovery_cancelled {
            cancelled.store(true, Ordering::Relaxed);
        }
        let was_discovering = self.grpc_discovering;
        if request_active {
            self.request_generation = self.request_generation.wrapping_add(1);
            self.loading = false;
            self.cancelled = None;
        }
        if was_discovering {
            self.grpc_discovery_generation = self.grpc_discovery_generation.wrapping_add(1);
            self.grpc_discovering = false;
            self.grpc_discovery_cancelled = None;
        }
        if request_active || was_discovering {
            self.notice = Some((
                if request_active {
                    "请求已取消"
                } else {
                    "gRPC Service 发现已取消"
                }
                .into(),
                false,
            ));
            cx.notify();
        }
    }
}
