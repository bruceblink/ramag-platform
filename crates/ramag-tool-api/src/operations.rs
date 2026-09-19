use super::context::{environment_from_view, request_record, upsert_environment};
use super::*;

impl ApiView {
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
