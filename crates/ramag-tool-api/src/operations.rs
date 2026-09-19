use super::context::{environment_from_view, request_record, upsert_environment};
use super::*;

use std::io::Read;

impl ApiView {
    /// 打开本地 JSON 文件，交给应用服务导入并持久化；读取和解析都受大小限制且不执行脚本。
    pub(crate) fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                        view.assertion_results.clear();
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

    /// 构造当前请求并交给 API 服务；结果包含响应、断言状态和脱敏历史摘要。
    pub(crate) fn send(&mut self, cx: &mut Context<Self>) {
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
        let environment = match environment_from_view(self, cx) {
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
        self.assertion_results.clear();
        self.last_collection_run = None;
        self.notice = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service
                .execute_record(&workspace_id, &record, &environment, cancelled)
                .await;
            let _ = this.update(cx, |view, cx| {
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
    pub(crate) fn run_collection(&mut self, cx: &mut Context<Self>) {
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
        let environment = match environment_from_view(self, cx) {
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
        self.assertion_results.clear();
        self.last_collection_run = None;
        self.notice = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service
                .run_collection(&workspace_id, &collection, &environment, cancelled)
                .await;
            let _ = this.update(cx, |view, cx| {
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
                        if let Some(result) = summary
                            .outcomes
                            .last()
                            .and_then(|outcome| outcome.result.as_ref().cloned())
                        {
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

    /// 把请求、断言和当前环境写入默认 Collection，Storage 负责加密。
    pub(crate) fn save(&mut self, cx: &mut Context<Self>) {
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
        let environment = match environment_from_view(self, cx) {
            Ok(environment) => environment,
            Err(error) => {
                self.notice = Some((error.to_string(), true));
                cx.notify();
                return;
            }
        };
        let mut workspace = self.workspace.clone();
        upsert_environment(&mut workspace, environment);
        if workspace.collections.is_empty() {
            workspace.collections.push(ApiCollection::new("默认请求"));
        }
        let collection = &mut workspace.collections[0];
        if let Some(existing) = collection
            .requests
            .iter_mut()
            .find(|existing| existing.name == record.name)
        {
            *existing = record;
        } else {
            collection.requests.push(record);
        }
        self.saving = true;
        self.notice = None;
        let workspace_for_save = workspace.clone();
        cx.spawn(async move |this, cx| {
            let result = service.save_workspace(&workspace_for_save).await;
            let _ = this.update(cx, |view, cx| {
                view.saving = false;
                match result {
                    Ok(()) => {
                        view.workspace = workspace;
                        view.notice = Some(("请求、环境和断言已保存".into(), false));
                    }
                    Err(error) => view.notice = Some((error.to_string(), true)),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
