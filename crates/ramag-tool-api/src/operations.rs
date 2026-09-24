use super::context::{apply_extracted_variables_to_view, environment_from_view, request_record};
use super::grpc_proto::{self, GrpcImportKind};
use super::*;

use std::io::Read;

impl ApiView {
    /// 打开本地 JSON 文件，交给应用服务导入并持久化；读取和解析都受大小限制且不执行脚本。
    pub(crate) fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.block_workspace_write_until_loaded(cx) {
            return;
        }
        if self.saving || self.importing || self.loading {
            return;
        }
        let Some(service) = self.service.clone() else {
            self.notice = Some(("API 服务尚未接入".into(), true));
            cx.notify();
            return;
        };
        self.importing = true;
        self.notice = None;
        let current_workspace = self.workspace.clone();
        cx.notify();
        cx.spawn_in(window, async move |this, async_cx| {
            let outcome: std::result::Result<_, String> = async {
                let Some(handle) = rfd::AsyncFileDialog::new()
                    .add_filter("Ramag JSON / Postman / OpenAPI 3", &["json"])
                    .pick_file()
                    .await
                else {
                    return Ok(None);
                };
                let path = handle.path().to_path_buf();
                let raw =
                    ramag_app::run_blocking(move || -> ramag_domain::error::Result<String> {
                        let file = std::fs::File::open(&path).map_err(|error| {
                            DomainError::Storage(format!("打开导入文件失败：{error}"))
                        })?;
                        let metadata = file.metadata().map_err(|error| {
                            DomainError::Storage(format!("读取导入文件信息失败：{error}"))
                        })?;
                        if !metadata.is_file() {
                            return Err(DomainError::InvalidConfig(
                                "导入目标必须是普通文件".into(),
                            ));
                        }
                        let max_bytes = ramag_domain::entities::MAX_API_IMPORT_BYTES as u64;
                        if metadata.len() > max_bytes {
                            return Err(DomainError::InvalidConfig(format!(
                                "导入文件超过 {max_bytes} bytes 上限"
                            )));
                        }
                        let mut bytes = Vec::new();
                        file.take(max_bytes + 1)
                            .read_to_end(&mut bytes)
                            .map_err(|error| {
                                DomainError::Storage(format!("读取导入文件失败：{error}"))
                            })?;
                        if bytes.len() as u64 > max_bytes {
                            return Err(DomainError::InvalidConfig(format!(
                                "导入文件读取后超过 {max_bytes} bytes 上限"
                            )));
                        }
                        String::from_utf8(bytes).map_err(|_| {
                            DomainError::InvalidConfig("导入文件必须使用 UTF-8 编码".into())
                        })
                    })
                    .await
                    .map_err(|error| format!("读取导入文件失败：{error}"))?;
                let imported = service
                    .import_workspace_json(&current_workspace, &raw)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(Some(imported))
            }
            .await;

            let _ = this.update_in(async_cx, |view, window, cx| {
                view.importing = false;
                match outcome {
                    Ok(None) => {}
                    Ok(Some((workspace, summary))) => {
                        let warning_count = summary.warnings.len();
                        view.workspace = workspace.clone();
                        context::apply_imported_workspace(view, &workspace, window, cx);
                        view.response = None;
                        view.response_tab = ApiResponseTab::Body;
                        view.assertion_results.clear();
                        view.extracted_variables.clear();
                        view.last_collection_run = None;
                        view.notice = Some((
                            format!(
                                "已导入 {}：{} 个 Collection · {} 个请求 · {} 个环境{}",
                                summary.format.label(),
                                summary.collection_count,
                                summary.request_count,
                                summary.environment_count,
                                if warning_count == 0 {
                                    String::new()
                                } else {
                                    format!(" · {} 条提示", warning_count)
                                }
                            ),
                            false,
                        ));
                    }
                    Err(error) => view.notice = Some((format!("导入失败：{error}"), true)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 读取有界的本地 FileDescriptorSet，供 gRPC 发现和请求执行共同使用。
    pub(crate) fn import_grpc_descriptor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.import_grpc_source(window, cx, GrpcImportKind::DescriptorSet);
    }

    /// 编译有界的本地 `.proto` 文件，供 gRPC 发现和请求执行共同使用。
    pub(crate) fn import_grpc_proto(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.import_grpc_source(window, cx, GrpcImportKind::ProtoSource);
    }

    fn import_grpc_source(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        kind: GrpcImportKind,
    ) {
        if self.protocol != ApiProtocol::Grpc
            || self.saving
            || self.importing
            || self.loading
            || self.grpc_discovering
        {
            return;
        }
        self.importing = true;
        self.notice = None;
        cx.notify();
        cx.spawn_in(window, async move |this, async_cx| {
            let outcome: std::result::Result<_, String> = async {
                let handle = match kind {
                    GrpcImportKind::DescriptorSet => {
                        rfd::AsyncFileDialog::new()
                            .set_title("选择 gRPC DescriptorSet 文件")
                            .add_filter("Protobuf FileDescriptorSet", &["bin", "fds", "desc"])
                            .add_filter("所有文件", &["*"])
                            .pick_file()
                            .await
                    }
                    GrpcImportKind::ProtoSource => {
                        rfd::AsyncFileDialog::new()
                            .set_title("选择 gRPC .proto 源文件")
                            .add_filter("Protobuf 源文件（.proto）", &["proto"])
                            .add_filter("所有文件", &["*"])
                            .pick_file()
                            .await
                    }
                };
                let Some(handle) = handle else {
                    return Ok(None);
                };
                let path = handle.path().to_path_buf();
                let descriptor =
                    ramag_app::run_blocking(move || grpc_proto::load_grpc_import(kind, &path))
                        .await
                        .map_err(|error| format!("读取 gRPC 描述文件失败：{error}"))?;
                Ok(Some(descriptor))
            }
            .await;

            let _ = this.update_in(async_cx, |view, _, cx| {
                view.importing = false;
                match outcome {
                    Ok(None) => {}
                    Ok(Some(descriptor)) => {
                        let bytes = match &descriptor {
                            ApiGrpcDescriptor::FileDescriptorSet { bytes } => bytes.len(),
                            ApiGrpcDescriptor::Reflection => 0,
                        };
                        view.grpc_descriptor = descriptor;
                        view.grpc_services.clear();
                        view.notice = Some((kind.success_message(bytes), false));
                    }
                    Err(error) => {
                        view.notice = Some((kind.failure_message(error), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 构造当前请求并交给 API 服务；结果包含响应、断言状态和脱敏历史摘要。
    pub(crate) fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            self.notice = Some(("API 服务尚未接入".into(), true));
            cx.notify();
            return;
        };
        let record = match request_record(self, cx) {
            Ok(record) => record,
            Err(error) => {
                self.notice = Some((error.to_string(), true));
                cx.notify();
                return;
            }
        };
        let mut environment = match environment_from_view(self, cx) {
            Ok(environment) => environment,
            Err(error) => {
                self.notice = Some((error.to_string(), true));
                cx.notify();
                return;
            }
        };
        let workspace_id = self.workspace.id.clone();
        self.request_generation = self.request_generation.wrapping_add(1);
        let generation = self.request_generation;
        if let Some(previous) = &self.cancelled {
            previous.store(true, Ordering::Relaxed);
        }
        let cancelled = new_api_cancellation();
        self.cancelled = Some(cancelled.clone());
        self.loading = true;
        self.response = None;
        self.response_tab = ApiResponseTab::Body;
        self.assertion_results.clear();
        self.extracted_variables.clear();
        self.last_collection_run = None;
        self.notice = None;
        cx.notify();
        cx.spawn_in(window, async move |this, async_cx| {
            let result = service
                .execute_record(&workspace_id, &record, &mut environment, cancelled)
                .await;
            let _ = this.update_in(async_cx, |view, window, cx| {
                if view.request_generation != generation {
                    return;
                }
                view.loading = false;
                view.cancelled = None;
                match result {
                    Ok(outcome) => {
                        view.history.insert(0, outcome.history);
                        view.history.truncate(20);
                        match outcome.result {
                            Some(result) => {
                                let extracted = result.extracted_variables.clone();
                                if let Err(error) =
                                    apply_extracted_variables_to_view(view, &extracted, window, cx)
                                {
                                    view.notice = Some((format!("变量回填失败：{error}"), true));
                                }
                                view.extracted_variables = extracted;
                                view.assertion_results = result.assertions;
                                view.response = Some(result.snapshot);
                                view.notice = if result.passed {
                                    Some(("请求完成，断言通过".into(), false))
                                } else {
                                    Some(("请求完成，但断言失败".into(), true))
                                };
                            }
                            None => {
                                view.notice = Some((
                                    outcome.error.unwrap_or_else(|| "请求失败".into()),
                                    true,
                                ));
                            }
                        }
                    }
                    Err(error) => view.notice = Some((error.to_string(), true)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 把当前请求并入首个 Collection 后串行执行，完成后在响应区域展示汇总。
    pub(crate) fn run_collection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            self.notice = Some(("API 服务尚未接入".into(), true));
            cx.notify();
            return;
        };
        let record = match request_record(self, cx) {
            Ok(record) => record,
            Err(error) => {
                self.notice = Some((error.to_string(), true));
                cx.notify();
                return;
            }
        };
        let mut environment = match environment_from_view(self, cx) {
            Ok(environment) => environment,
            Err(error) => {
                self.notice = Some((error.to_string(), true));
                cx.notify();
                return;
            }
        };
        let mut collection = self
            .workspace
            .collections
            .first()
            .cloned()
            .unwrap_or_else(|| ApiCollection::new("默认请求"));
        if let Some(existing) = collection
            .requests
            .iter_mut()
            .find(|existing| existing.name == record.name)
        {
            *existing = record;
        } else {
            collection.requests.push(record);
        }
        let workspace_id = self.workspace.id.clone();
        self.request_generation = self.request_generation.wrapping_add(1);
        let generation = self.request_generation;
        if let Some(previous) = &self.cancelled {
            previous.store(true, Ordering::Relaxed);
        }
        let cancelled = new_api_cancellation();
        self.cancelled = Some(cancelled.clone());
        self.loading = true;
        self.response = None;
        self.response_tab = ApiResponseTab::Body;
        self.assertion_results.clear();
        self.extracted_variables.clear();
        self.last_collection_run = None;
        self.notice = None;
        cx.notify();
        cx.spawn_in(window, async move |this, async_cx| {
            let result = service
                .run_collection(&workspace_id, &collection, &mut environment, cancelled)
                .await;
            let _ = this.update_in(async_cx, |view, window, cx| {
                if view.request_generation != generation {
                    return;
                }
                view.loading = false;
                view.cancelled = None;
                match result {
                    Ok(summary) => {
                        for outcome in &summary.outcomes {
                            view.history.insert(0, outcome.history.clone());
                        }
                        view.history.truncate(20);
                        let extracted = summary
                            .outcomes
                            .iter()
                            .filter_map(|outcome| outcome.result.as_ref())
                            .flat_map(|result| result.extracted_variables.clone())
                            .collect::<Vec<_>>();
                        if let Err(error) =
                            apply_extracted_variables_to_view(view, &extracted, window, cx)
                        {
                            view.notice = Some((format!("变量回填失败：{error}"), true));
                        }
                        if let Some(result) = summary
                            .outcomes
                            .last()
                            .and_then(|outcome| outcome.result.as_ref().cloned())
                        {
                            view.extracted_variables = result.extracted_variables;
                            view.assertion_results = result.assertions;
                            view.response = Some(result.snapshot);
                        }
                        let stopped = if summary.stopped { "，已停止" } else { "" };
                        let failed = summary.failed > 0 || summary.cancelled > 0;
                        view.notice = Some((
                            format!(
                                "Collection {}：{} 通过 · {} 失败 · {} 取消{}",
                                summary.collection_name,
                                summary.passed,
                                summary.failed,
                                summary.cancelled,
                                stopped
                            ),
                            failed,
                        ));
                        view.last_collection_run = Some(summary);
                    }
                    Err(error) => view.notice = Some((error.to_string(), true)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 使用当前 Environment 展开 Endpoint，并读取 gRPC Service/Method 目录。
    pub(crate) fn discover_grpc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.protocol != ApiProtocol::Grpc {
            return;
        }
        let Some(service) = self.service.clone() else {
            self.notice = Some(("API 服务尚未接入".into(), true));
            cx.notify();
            return;
        };
        let environment = match environment_from_view(self, cx) {
            Ok(environment) => environment,
            Err(error) => {
                self.notice = Some((error.to_string(), true));
                cx.notify();
                return;
            }
        };
        let mut request = ApiGrpcDiscoverySpec::new(input_value(&self.grpc_endpoint, cx));
        request.descriptor = self.grpc_descriptor.clone();
        request.auth = match self.auth_editor.to_auth(cx) {
            Ok(auth) => auth,
            Err(error) => {
                self.notice = Some((error.to_string(), true));
                cx.notify();
                return;
            }
        };
        request.tls = match context::tls_from_view(self, cx) {
            Ok(tls) => tls,
            Err(error) => {
                self.notice = Some((error.to_string(), true));
                cx.notify();
                return;
            }
        };
        let proxy = match context::proxy_from_view(self, cx) {
            Ok(proxy) => proxy,
            Err(error) => {
                self.notice = Some((error.to_string(), true));
                cx.notify();
                return;
            }
        };
        request.proxy = proxy;
        let variables = environment.execution_variables();
        self.grpc_discovery_generation = self.grpc_discovery_generation.wrapping_add(1);
        let generation = self.grpc_discovery_generation;
        if let Some(previous) = &self.grpc_discovery_cancelled {
            previous.store(true, Ordering::Relaxed);
        }
        let cancelled = new_api_cancellation();
        self.grpc_discovery_cancelled = Some(cancelled.clone());
        self.grpc_discovering = true;
        self.grpc_services.clear();
        self.notice = None;
        cx.notify();
        cx.spawn_in(window, async move |this, async_cx| {
            let result = service
                .discover_grpc_services(&request, &variables, cancelled)
                .await;
            let _ = this.update_in(async_cx, |view, window, cx| {
                if view.grpc_discovery_generation != generation {
                    return;
                }
                view.grpc_discovering = false;
                view.grpc_discovery_cancelled = None;
                match result {
                    Ok(services) => {
                        let services = services
                            .into_iter()
                            .filter(|service| {
                                !matches!(
                                    service.name.as_str(),
                                    "grpc.reflection.v1.ServerReflection"
                                        | "grpc.reflection.v1alpha.ServerReflection"
                                )
                            })
                            .collect::<Vec<_>>();
                        let method_count = services
                            .iter()
                            .map(|service| service.methods.len())
                            .sum::<usize>();
                        if let Some(service_summary) = services.first()
                            && let Some(method) = service_summary.methods.first()
                        {
                            view.grpc_service.update(cx, |input, cx| {
                                input.set_value(service_summary.name.clone(), window, cx)
                            });
                            view.grpc_method.update(cx, |input, cx| {
                                input.set_value(method.name.clone(), window, cx)
                            });
                        }
                        view.grpc_services = services;
                        view.notice = Some((
                            format!(
                                "已发现 {} 个 Service、{} 个 Method",
                                view.grpc_services.len(),
                                method_count
                            ),
                            false,
                        ));
                    }
                    Err(error) => {
                        view.grpc_services.clear();
                        view.notice = Some((format!("gRPC Service 发现失败：{error}"), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
