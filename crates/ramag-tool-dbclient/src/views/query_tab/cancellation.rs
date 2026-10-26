use gpui_kit::{AppContext as _, Context};

use super::QueryTab;

impl QueryTab {
    /// Cancels a query that no longer belongs to the active SQL context.
    pub(super) fn cancel_inflight_query(&mut self, cx: &mut Context<Self>) {
        if !self.running {
            return;
        }
        let query_task = self.current_task.take();
        let Some(cancel_handle) = self.cancel_handle.take() else {
            drop(query_task);
            return;
        };
        let thread_id = cancel_handle.load(std::sync::atomic::Ordering::SeqCst);
        let Some(conn) = self.connection.clone() else {
            drop(query_task);
            return;
        };
        if thread_id == 0 {
            drop(query_task);
            return;
        }
        if let Some(task) = query_task {
            // Preserve the cancellation result in history while the sequence rejects its UI update.
            task.detach();
        }
        let service = self.service.clone();
        cx.background_spawn(async move {
            match service.cancel_query(&conn, thread_id).await {
                Ok(()) => tracing::info!(
                    operation = "sql_query_context_cancel",
                    connection_id = %conn.id,
                    driver = ?conn.driver,
                    thread_id,
                    "invalidated query cancellation confirmed"
                ),
                Err(error) => tracing::warn!(
                    operation = "sql_query_context_cancel",
                    connection_id = %conn.id,
                    driver = ?conn.driver,
                    thread_id,
                    error = %error,
                    "invalidated query cancellation failed"
                ),
            }
        })
        .detach();
    }

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
