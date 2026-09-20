use super::*;

impl ApiView {
    pub(crate) fn set_protocol(&mut self, protocol: ApiProtocol, cx: &mut Context<Self>) {
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

    pub(crate) fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(cancelled) = &self.cancelled {
            cancelled.store(true, Ordering::Relaxed);
        }
        if let Some(cancelled) = &self.grpc_discovery_cancelled {
            cancelled.store(true, Ordering::Relaxed);
        }
        let was_loading = self.loading;
        let was_discovering = self.grpc_discovering;
        if was_loading {
            self.request_generation = self.request_generation.wrapping_add(1);
            self.loading = false;
        }
        if was_discovering {
            self.grpc_discovery_generation = self.grpc_discovery_generation.wrapping_add(1);
            self.grpc_discovering = false;
            self.grpc_discovery_cancelled = None;
        }
        if was_loading || was_discovering {
            self.notice = Some((
                if was_loading {
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
