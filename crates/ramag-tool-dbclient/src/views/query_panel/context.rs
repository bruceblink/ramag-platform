use gpui::{Context, Window};
use ramag_domain::entities::ConnectionConfig;

use super::QueryPanel;

impl QueryPanel {
    /// Counts local result changes and open transactions before a context switch.
    /// The caller uses this snapshot to avoid silently discarding state from any tab.
    fn pending_context_change_message(&self, cx: &gpui::App) -> Option<String> {
        let mut pending_result_changes = 0usize;
        let mut open_transactions = 0usize;
        for tab in &self.tabs {
            let tab = tab.read(cx);
            pending_result_changes += tab.pending_result_change_count(cx);
            open_transactions += usize::from(tab.has_open_transaction());
        }
        if pending_result_changes == 0 && open_transactions == 0 {
            return None;
        }

        let mut effects = Vec::with_capacity(2);
        if pending_result_changes > 0 {
            effects.push(format!("{pending_result_changes} 项未提交结果修改将被撤销"));
        }
        if open_transactions > 0 {
            effects.push(format!("{open_transactions} 个打开的事务将回滚"));
        }
        Some(format!(
            "{}。请先完成这些操作，或确认继续。",
            effects.join("；")
        ))
    }

    /// Applies a connection after the user has accepted the data-loss warning.
    fn set_connection_inner(
        &mut self,
        conn: Option<ConnectionConfig>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.connection.as_ref().map(|current| &current.id)
            != conn.as_ref().map(|current| &current.id)
        {
            self.closed_drafts.clear();
        }
        self.connection = conn.clone();
        self.active_schema = conn
            .as_ref()
            .and_then(|c| c.database.clone())
            .filter(|s| !s.is_empty());
        for tab in self.tabs.iter() {
            tab.update(cx, |t, cx| t.set_connection(conn.clone(), cx));
        }
        self.load_persisted_drafts(window, cx);
        cx.notify();
    }

    /// Changes the active connection only after all tabs accept the context reset.
    pub fn set_connection(
        &mut self,
        conn: Option<ConnectionConfig>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.connection.as_ref() == conn.as_ref() {
            return;
        }
        if let Some(message) = self.pending_context_change_message(cx) {
            let entity = cx.entity();
            ramag_ui::open_confirm(
                "切换数据库连接？",
                message,
                "继续切换",
                true,
                move |window, app| {
                    entity.update(app, |this, cx| {
                        this.set_connection_inner(conn, window, cx);
                    });
                },
                window,
                cx,
            );
            return;
        }
        self.set_connection_inner(conn, window, cx);
    }

    /// Applies a Schema change after the user has accepted the context reset.
    fn set_active_schema_inner(&mut self, schema: Option<String>, cx: &mut Context<Self>) {
        if self.active_schema == schema {
            return;
        }
        self.active_schema = schema.clone();
        for tab in self.tabs.iter() {
            tab.update(cx, |t, cx| t.set_active_schema(schema.clone(), cx));
        }
        self.schedule_draft_persist(cx);
        cx.notify();
    }

    /// Changes Schema after confirmation and optionally continues the original action.
    /// The callback also runs immediately when no confirmation is required, so callers do not
    /// lose actions such as opening the table that triggered the Schema change.
    pub fn set_active_schema_with_callback<F>(
        &mut self,
        schema: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_applied: F,
    ) -> bool
    where
        F: FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    {
        let normalized = schema.filter(|s| !s.is_empty());
        if self.active_schema == normalized {
            on_applied(self, window, cx);
            return true;
        }
        if let Some(message) = self.pending_context_change_message(cx) {
            let entity = cx.entity();
            ramag_ui::open_confirm(
                "切换 Schema？",
                message,
                "继续切换",
                true,
                move |window, app| {
                    entity.update(app, |this, cx| {
                        this.set_active_schema_inner(normalized, cx);
                        on_applied(this, window, cx);
                    });
                },
                window,
                cx,
            );
            return false;
        }
        self.set_active_schema_inner(normalized, cx);
        on_applied(self, window, cx);
        true
    }

    /// Changes Schema atomically across tabs and returns whether it was applied.
    /// `false` means a confirmation dialog is open and the caller must stop its action.
    pub fn set_active_schema(
        &mut self,
        schema: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.set_active_schema_with_callback(schema, window, cx, |_, _, _| {})
    }
}
