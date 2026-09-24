use super::*;

impl ApiView {
    /// Blocks workspace writes until the initial Storage read has a trustworthy result.
    pub(crate) fn block_workspace_write_until_loaded(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(message) = self.workspace_load_state.save_block_message() else {
            return false;
        };
        self.notice = Some((message.into(), true));
        cx.notify();
        true
    }

    /// Loads the saved workspace and its history without exposing raw Storage errors.
    ///
    /// An empty result is distinct from a failed read: only a confirmed empty result
    /// allows saving a new workspace. The generation check prevents a late read from
    /// replacing state produced by a newer retry.
    pub(crate) fn load_saved_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || self.importing {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        self.workspace_load_generation = self.workspace_load_generation.wrapping_add(1);
        let generation = self.workspace_load_generation;
        self.workspace_load_state = ApiWorkspaceLoadState::Loading;
        self.notice = None;
        cx.notify();

        cx.spawn_in(window, async move |this, async_cx| {
            let result = match service.list_workspaces().await {
                Ok(workspaces) => match workspaces.into_iter().next() {
                    Some(workspace) => {
                        let history = service
                            .list_history(&workspace.id, 20)
                            .await
                            .unwrap_or_default();
                        Ok(Some((workspace, history)))
                    }
                    None => Ok(None),
                },
                Err(_) => Err(()),
            };

            let _ = this.update_in(async_cx, move |view, window, cx| {
                if view.workspace_load_generation != generation {
                    return;
                }
                match result {
                    Ok(Some((workspace, history))) => {
                        view.workspace = workspace;
                        view.history = history;
                        view.workspace_load_state = ApiWorkspaceLoadState::Loaded;
                        let workspace = view.workspace.clone();
                        context::apply_imported_workspace(view, &workspace, window, cx);
                    }
                    Ok(None) => {
                        view.history.clear();
                        view.workspace_load_state = ApiWorkspaceLoadState::Empty;
                    }
                    Err(()) => view.workspace_load_state = ApiWorkspaceLoadState::Failed,
                }
                cx.notify();
            });
        })
        .detach();
    }
}
