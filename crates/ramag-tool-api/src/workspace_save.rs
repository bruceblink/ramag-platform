use super::context::{environment_from_view, request_record, upsert_environment};
use super::*;

impl ApiView {
    /// 把当前编辑器的请求、断言和环境异步写入工作区；只有草稿和读取代次未变化时才回写界面。
    pub(crate) fn save(&mut self, cx: &mut Context<Self>) {
        if self.saving || self.importing || self.loading {
            return;
        }
        if self.block_workspace_write_until_loaded(cx) {
            return;
        }
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
        let saved_environment = environment.clone();
        upsert_environment(&mut workspace, environment);
        if workspace.collections.is_empty() {
            workspace.collections.push(ApiCollection::new("默认请求"));
        }
        let active_request_id = self.active_request_id.clone();
        let existing_location =
            active_request_id.as_ref().and_then(|request_id| {
                workspace.collections.iter().enumerate().find_map(
                    |(collection_index, collection)| {
                        collection
                            .requests
                            .iter()
                            .position(|existing| &existing.id == request_id)
                            .map(|request_index| (collection_index, request_index))
                    },
                )
            });
        let existing_location = existing_location.or_else(|| {
            workspace.collections[0]
                .requests
                .iter()
                .position(|existing| existing.name == record.name)
                .map(|request_index| (0, request_index))
        });
        let saved_request_id = if let Some((collection_index, request_index)) = existing_location {
            let request_id = workspace.collections[collection_index].requests[request_index]
                .id
                .clone();
            let mut record = record;
            record.id = request_id.clone();
            workspace.collections[collection_index].requests[request_index] = record;
            request_id
        } else {
            let request_id = record.id.clone();
            workspace.collections[0].requests.push(record);
            request_id
        };
        let Some(saved_record) = workspace
            .collections
            .iter()
            .flat_map(|collection| collection.requests.iter())
            .find(|request| request.id == saved_request_id)
            .cloned()
        else {
            self.notice = Some(("保存请求失败：无法定位待保存请求".into(), true));
            cx.notify();
            return;
        };
        let load_generation = self.workspace_load_generation;
        self.saving = true;
        self.notice = None;
        let workspace_for_save = workspace.clone();
        cx.spawn(async move |this, cx| {
            let result = service.save_workspace(&workspace_for_save).await;
            let _ = this.update(cx, |view, cx| {
                view.saving = false;
                if view.workspace_load_generation != load_generation {
                    cx.notify();
                    return;
                }
                let draft_is_unchanged = view.active_request_id == active_request_id
                    && request_record(view, cx).is_ok_and(|mut current| {
                        current.id = saved_request_id.clone();
                        current == saved_record
                    })
                    && environment_from_view(view, cx)
                        .is_ok_and(|current| current == saved_environment);
                match result {
                    Ok(()) => {
                        view.workspace_load_state = ApiWorkspaceLoadState::Loaded;
                        if draft_is_unchanged {
                            view.workspace = workspace;
                            view.active_request_id = Some(saved_request_id);
                            view.notice = Some(("请求、环境和断言已保存".into(), false));
                        }
                    }
                    Err(error) if draft_is_unchanged => {
                        view.notice = Some((error.to_string(), true));
                    }
                    Err(_) => {}
                }
                cx.notify();
            });
        })
        .detach();
    }
}
