use super::*;

impl ApiView {
    /// 构造当前编辑器内容并交给应用服务；每次发送递增代次，迟到结果不能覆盖新请求。
    pub(crate) fn send(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            self.notice = Some(("API 服务尚未接入".into(), true));
            cx.notify();
            return;
        };
        let request = match request_from_view(self, cx) {
            Ok(request) => request,
            Err(error) => {
                self.notice = Some((error.to_string(), true));
                cx.notify();
                return;
            }
        };
        self.request_generation = self.request_generation.wrapping_add(1);
        let generation = self.request_generation;
        if let Some(previous) = &self.cancelled {
            previous.store(true, Ordering::Relaxed);
        }
        let cancelled = new_api_cancellation();
        self.cancelled = Some(cancelled.clone());
        self.loading = true;
        self.response = None;
        self.notice = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.execute(&request, &BTreeMap::new(), cancelled).await;
            let _ = this.update(cx, |view, cx| {
                if view.request_generation != generation {
                    return;
                }
                view.loading = false;
                view.cancelled = None;
                match result {
                    Ok(snapshot) => {
                        view.notice = Some(("请求完成".into(), false));
                        view.response = Some(snapshot);
                    }
                    Err(error) => view.notice = Some((error.to_string(), true)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 把当前请求写入默认 Collection，Storage 负责加密工作区中的敏感字段。
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
        let mut workspace = self.workspace.clone();
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
                        view.notice = Some(("请求已保存到本地工作区".into(), false));
                    }
                    Err(error) => view.notice = Some((error.to_string(), true)),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
