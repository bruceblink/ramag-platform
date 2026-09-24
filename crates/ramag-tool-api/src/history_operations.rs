use super::*;

impl ApiView {
    pub(crate) fn confirm_clear_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.service.is_none()
            || self.history.is_empty()
            || self.clearing_history
            || self.workspace_load_state.save_block_message().is_some()
        {
            return;
        }
        let view = cx.entity();
        ramag_ui::open_confirm(
            "清空执行历史？",
            "当前 API 工作区的执行历史将被删除，已保存请求不会改变。",
            "清空",
            true,
            move |_, app| {
                view.update(app, |view, cx| view.clear_history(cx));
            },
            window,
            cx,
        );
    }

    pub(crate) fn clear_history(&mut self, cx: &mut Context<Self>) {
        if self.clearing_history {
            return;
        }
        let Some(service) = self.service.clone() else {
            self.notice = Some(("API 服务尚未接入".into(), true));
            cx.notify();
            return;
        };
        if self.history.is_empty() {
            return;
        }
        let workspace_id = self.workspace.id.clone();
        self.clearing_history = true;
        self.notice = Some(("正在清空执行历史…".into(), false));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.clear_history(&workspace_id).await;
            let _ = this.update(cx, |view, cx| {
                view.clearing_history = false;
                match result {
                    Ok(()) => {
                        view.history.clear();
                        view.notice = Some(("执行历史已清空".into(), false));
                    }
                    Err(error) => {
                        view.notice = Some((format!("清空执行历史失败：{error}"), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
