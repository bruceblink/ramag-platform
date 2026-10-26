use super::*;

/// Stops the current tab's page and count requests, then reports server confirmations.
pub(super) fn handle_cancel(tab: &mut QueryTab, window: &mut Window, cx: &mut Context<QueryTab>) {
    let has_query_task = tab.current_task.take().is_some();
    let count_task = tab.count_task.take();
    let count_cancel_handle = tab.count_cancel_handle.take();
    let has_count_task = count_task.is_some() || count_cancel_handle.is_some();
    drop(count_task);
    if !has_query_task && !has_count_task {
        return;
    }
    if has_query_task {
        tab.run_seq = tab.run_seq.wrapping_add(1);
        tab.clear_cancelled_result(cx);
        tab.running = false;
        tab.query_start = None;
    }
    tab.count_seq = tab.count_seq.wrapping_add(1);
    let cancel_run_seq = tab.run_seq;
    let cancel_count_seq = tab.count_seq;
    let connection = tab.connection.clone();
    let mut cancel_requests: Vec<(&'static str, ramag_domain::entities::ConnectionConfig, u64)> =
        Vec::new();
    let mut missing_targets = Vec::new();

    if has_query_task {
        let backend_id = tab.cancel_handle.take().and_then(|handle| {
            let id = handle.load(std::sync::atomic::Ordering::SeqCst);
            (id > 0).then_some(id)
        });
        if let (Some(backend_id), Some(conn)) = (backend_id, connection.clone()) {
            cancel_requests.push(("查询", conn, backend_id));
        } else {
            missing_targets.push("查询".to_string());
        }
    }
    if let Some(handle) = count_cancel_handle {
        let backend_id = handle.load(std::sync::atomic::Ordering::SeqCst);
        if let (true, Some(conn)) = (backend_id > 0, connection) {
            cancel_requests.push(("分页统计", conn, backend_id));
        } else {
            missing_targets.push("分页统计".to_string());
        }
    }

    if !cancel_requests.is_empty() {
        ramag_ui::push_responsive_notification(
            window,
            Notification::info("已停止等待，正在请求服务器取消当前标签的查询…").autohide(true),
            cx,
        );
        let service = tab.service.clone();
        cx.spawn(async move |entity, cx| {
            let mut confirmed = Vec::new();
            let mut failures = missing_targets;
            for (label, conn, backend_id) in cancel_requests {
                match service.cancel_query(&conn, backend_id).await {
                    Ok(()) => confirmed.push(label),
                    Err(error) => failures.push(format!("{label}：{error}")),
                }
            }
            let notification = if failures.is_empty() {
                Notification::success(format!("服务器已确认取消{}", confirmed.join("和")))
                    .autohide(true)
            } else {
                Notification::warning(format!(
                    "客户端已停止等待；服务器未能确认取消：{}",
                    failures.join("；")
                ))
                .autohide(true)
            };
            let _ = entity.update(cx, |tab, cx| {
                if tab.run_seq != cancel_run_seq || tab.count_seq != cancel_count_seq {
                    return;
                }
                tab.pending_notification = Some(notification);
                cx.notify();
            });
            tracing::info!(
                operation = "sql_query_cancel",
                "server cancellation requests completed"
            );
        })
        .detach();
    } else {
        tab.pending_notification = Some(
            Notification::warning(format!(
                "客户端已停止等待；未获取到服务器线程：{}",
                missing_targets.join("、")
            ))
            .autohide(true),
        );
    }
    tracing::info!(
        operation = "sql_query_cancel",
        query_task = has_query_task,
        count_task = has_count_task,
        "client query wait cancelled"
    );
    cx.notify();
}
