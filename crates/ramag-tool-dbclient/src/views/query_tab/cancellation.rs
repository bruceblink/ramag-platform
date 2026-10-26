use gpui_kit::{AppContext as _, Context};

use super::QueryTab;

impl QueryTab {
    /// Invalidates stale COUNT responses and requests cancellation on the database server.
    pub(super) fn cancel_total_count(&mut self, cx: &mut Context<Self>) {
        self.count_seq = self.count_seq.wrapping_add(1);
        let _ = self.count_task.take();
        let Some(handle) = self.count_cancel_handle.take() else {
            return;
        };
        let backend_id = handle.load(std::sync::atomic::Ordering::SeqCst);
        if backend_id == 0 {
            tracing::warn!(
                operation = "sql_count_cancel",
                connection_id = ?self.connection.as_ref().map(|connection| &connection.id),
                reason = "backend_id_unavailable",
                "count query cancellation skipped"
            );
            return;
        }
        let Some(conn) = self.connection.clone() else {
            return;
        };
        let service = self.service.clone();
        cx.background_spawn(async move {
            match service.cancel_query(&conn, backend_id).await {
                Ok(()) => tracing::info!(
                    operation = "sql_count_cancel",
                    connection_id = %conn.id,
                    driver = ?conn.driver,
                    thread_id = backend_id,
                    "count query cancellation confirmed"
                ),
                Err(error) => tracing::warn!(
                    operation = "sql_count_cancel",
                    connection_id = %conn.id,
                    driver = ?conn.driver,
                    thread_id = backend_id,
                    error = %error,
                    "count query cancellation failed"
                ),
            }
        })
        .detach();
    }
}
